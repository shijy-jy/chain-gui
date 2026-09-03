//! Engram MCP 核心操作层（规划书 v1.1 阶段一 M2-M4）：
//! 不依赖 tauri / rmcp 的纯函数工具集，供 engram-mcp bin 做协议映射。
//! - 工具清单 v1：只读 6（get_overview/search/read_node/expand/read_path/get_guide）
//!   + 写入 3（create_node/update_node/link_nodes）
//! - D3 并发写保护：expected_updated 乐观锁（CONFLICT 不落盘）+ tmp/rename 原子写；
//!   跨请求串行化由 bin 侧 write_lock 保证
//! - D4：写入类工具返回体附 hint 规范提示；指南版本经 bin 握手 instructions 下发
//! - D1：rel 保持子节点 frontmatter 单值，边说明走可选 rel_desc（不引入边列表）
//! - D2：rel_type 仅放行 contains / solves / alternative（严格校验，非法值报错）

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use serde_json::{json, Value};

use crate::commands::{ai_guide, graph_edit, workspace};
use crate::model::{ScanMode, UpdateFields};
use crate::scanner::{frontmatter, walker};

/// 合法的递进关系类型（D2 词表）
const REL_TYPES: [&str; 3] = ["contains", "solves", "alternative"];

// ── server 上下文 ─────────────────────────────────────────

/// MCP server 上下文：--workspace 启动时建立，全程只读
pub struct McpContext {
    pub root: PathBuf,
    pub mode: ScanMode,
}

impl McpContext {
    /// 校验目标目录是已打标的 Engram 工作区（.chain/.mode = dev/analysis）。
    /// 无 .chain 或无标签 → 拒绝服务（不给来历不明的目录乱写）。
    pub fn open(root: PathBuf) -> Result<Self, String> {
        if !root.join(".chain").is_dir() {
            return Err(format!(
                "目录 {} 下不存在 .chain/，不是 Engram 工作区",
                root.display()
            ));
        }
        let mode = workspace::read_mode_tag(&root).ok_or_else(|| {
            format!(
                "工作区 {} 缺少 .chain/.mode 标签（dev/analysis）——请先在 Engram GUI 添加该工作区完成打标",
                root.display()
            )
        })?;
        Ok(Self { root, mode })
    }

    pub fn mode_str(&self) -> &'static str {
        workspace::mode_to_str(self.mode)
    }

    pub fn guide_version(&self) -> u32 {
        ai_guide::guide_version_for(Some(self.mode_str()))
    }

    fn nodes_dir(&self) -> PathBuf {
        self.root.join(".chain").join("nodes")
    }

    fn node_path(&self, id: &str) -> PathBuf {
        self.nodes_dir().join(format!("{id}.md"))
    }

    fn scan(&self) -> Result<crate::model::chain::ChainSnapshot, String> {
        walker::scan_chain_dir_mode(&self.root, self.mode).map_err(|e| format!("扫描失败：{e}"))
    }
}

// ── 写入保护原语（D3）──────────────────────────────────────

/// tmp/rename 原子写：先写同目录隐藏 .tmp 再 rename 替换，杜绝半截文件。
/// Windows 上 Rust rename 为替换语义（MOVEFILE_REPLACE_EXISTING），
/// 由测试 update_append_then_replace（连续两次写同一文件）覆盖验证。
fn atomic_write(path: &Path, content: &str) -> Result<(), String> {
    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| format!("非法文件路径：{}", path.display()))?;
    let tmp = path.with_file_name(format!(".{file_name}.tmp"));
    std::fs::write(&tmp, content).map_err(|e| format!("写临时文件失败：{e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("原子替换失败：{e}")
    })?;
    Ok(())
}

/// 开发模式宽松解析兜底：无 frontmatter 的 .md 补最小 frontmatter（语义同 GUI update_node）
fn parse_lenient(raw: &str, node_id: &str) -> Result<(serde_yaml::Mapping, String), String> {
    use serde_yaml::Value as YV;
    match frontmatter::parse(raw) {
        Ok(result) => Ok(result),
        Err(e) => Err(format!("解析 frontmatter 失败：{e}")),
    }
    .or_else(|_| {
        let now = frontmatter::now_iso8601();
        let mut m = serde_yaml::Mapping::new();
        m.insert(YV::String("id".into()), YV::String(node_id.into()));
        m.insert(YV::String("type".into()), YV::String("note".into()));
        m.insert(YV::String("status".into()), YV::String("none".into()));
        m.insert(YV::String("title".into()), YV::String(node_id.into()));
        m.insert(YV::String("created".into()), YV::String(now.clone()));
        m.insert(YV::String("updated".into()), YV::String(now));
        m.insert(YV::String("revision".into()), YV::Number(1u64.into()));
        m.insert(YV::String("parent".into()), YV::Null);
        Ok((m, raw.to_string()))
    })
}

fn fm_get_str(fm: &serde_yaml::Mapping, key: &str) -> Option<String> {
    fm.get(&serde_yaml::Value::String(key.into()))
        .and_then(|v| v.as_str().map(|s| s.to_string()))
}

// ── 只读工具 ──────────────────────────────────────────────

/// get_overview()：全局概览——规模 + 活跃链 + 健康度 + 模式与指南版本
pub fn get_overview(ctx: &McpContext) -> Result<Value, String> {
    let snap = ctx.scan()?;
    Ok(json!({
        "workspace": ctx.root.display().to_string(),
        "mode": ctx.mode_str(),
        "node_count": snap.manifest.node_count,
        "edge_count": snap.manifest.edge_count,
        "active_chain": snap.manifest.active_chain,
        "chain_health": snap.manifest.chain_health,
        "guide_version": ctx.guide_version(),
    }))
}

/// search(query, limit=10)：title/tags/body 大小写不敏感子串匹配
pub fn search(ctx: &McpContext, query: &str, limit: Option<usize>) -> Result<Value, String> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Err("query 不能为空".into());
    }
    let limit = limit.unwrap_or(10).clamp(1, 100);
    let snap = ctx.scan()?;
    let mut hits: Vec<(u8, Value)> = Vec::new();
    for n in &snap.nodes {
        let mut score = 0u8;
        let mut matched_on: Vec<&str> = Vec::new();
        if n.title.to_lowercase().contains(&q) {
            score = score.max(3);
            matched_on.push("title");
        }
        if n.tags.iter().any(|t| t.to_lowercase().contains(&q)) {
            score = score.max(2);
            matched_on.push("tags");
        }
        if n.body.to_lowercase().contains(&q) {
            score = score.max(1);
            matched_on.push("body");
        }
        if score > 0 {
            hits.push((score, json!({
                "id": n.id,
                "title": n.title,
                "type": n.node_type,
                "status": n.status,
                "matched_on": matched_on,
                "snippet": frontmatter::truncate_utf8(n.body.trim(), 160),
                "updated": n.updated,
            })));
        }
    }
    // 命中字段权重优先，同级按 updated 倒序
    hits.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| {
        let ua = a.1["updated"].as_str().unwrap_or("").to_string();
        let ub = b.1["updated"].as_str().unwrap_or("").to_string();
        ub.cmp(&ua)
    }));
    let total = hits.len();
    let results: Vec<Value> = hits.into_iter().take(limit).map(|(_, v)| v).collect();
    Ok(json!({ "query": query, "total": total, "returned": results.len(), "results": results }))
}

/// read_node(id, include_neighbors=false)：单节点全字段 + 正文；neighbors 附父与子
pub fn read_node(ctx: &McpContext, id: &str, include_neighbors: Option<bool>) -> Result<Value, String> {
    if !graph_edit::is_safe_id(id) {
        return Err("节点 id 非法（仅允许字母/数字/连字符/下划线）".into());
    }
    let snap = ctx.scan()?;
    let node = snap
        .nodes
        .iter()
        .find(|n| n.id == id)
        .ok_or_else(|| format!("节点 {id} 不存在"))?;
    let mut v = serde_json::to_value(node).map_err(|e| format!("序列化失败：{e}"))?;
    // MCP 输出规范化：serialize 落盘会在 body 尾加 \n、parse 原样读回——对 AI 隐藏文件格式噪音
    if let Some(b) = v.get("body").and_then(|b| b.as_str()) {
        let trimmed = b.trim_end().to_string();
        v["body"] = Value::String(trimmed);
    }
    if include_neighbors.unwrap_or(false) {
        let parent_node = node
            .parent
            .as_ref()
            .and_then(|p| snap.nodes.iter().find(|n| &n.id == p))
            .map(|n| json!({"id": n.id, "title": n.title}));
        let children: Vec<Value> = snap
            .nodes
            .iter()
            .filter(|n| n.parent.as_deref() == Some(id))
            .map(|n| json!({"id": n.id, "title": n.title, "rel": n.rel}))
            .collect();
        v["neighbors"] = json!({ "parent": parent_node, "children": children });
    }
    Ok(v)
}

/// expand(id, depth=1..2)：以 id 为中心无向 BFS 扩展，返回局部子图（摘要级）
pub fn expand(ctx: &McpContext, id: &str, depth: Option<u32>) -> Result<Value, String> {
    let depth = depth.unwrap_or(1);
    if !(1..=2).contains(&depth) {
        return Err(format!("depth 仅支持 1 或 2（防大图上响应膨胀），收到：{depth}"));
    }
    let snap = ctx.scan()?;
    if !snap.nodes.iter().any(|n| n.id == id) {
        return Err(format!("节点 {id} 不存在"));
    }
    // 无向邻接表
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    for e in &snap.edges {
        adj.entry(e.parent.as_str()).or_default().push(e.child.as_str());
        adj.entry(e.child.as_str()).or_default().push(e.parent.as_str());
    }
    let mut visited: HashSet<&str> = HashSet::from([id]);
    let mut frontier: Vec<&str> = vec![id];
    for _ in 0..depth {
        let mut next = Vec::new();
        for cur in &frontier {
            if let Some(neis) = adj.get(cur) {
                for nei in neis {
                    if visited.insert(*nei) {
                        next.push(*nei);
                    }
                }
            }
        }
        frontier = next;
    }
    let nodes: Vec<Value> = snap
        .nodes
        .iter()
        .filter(|n| visited.contains(n.id.as_str()))
        .map(|n| json!({
            "id": n.id, "title": n.title, "type": n.node_type,
            "status": n.status, "parent": n.parent, "rel": n.rel,
        }))
        .collect();
    let edges: Vec<Value> = snap
        .edges
        .iter()
        .filter(|e| visited.contains(e.parent.as_str()) && visited.contains(e.child.as_str()))
        .map(|e| json!({"parent": e.parent, "child": e.child, "rel": e.rel}))
        .collect();
    Ok(json!({ "center": id, "depth": depth, "node_count": nodes.len(), "nodes": nodes, "edges": edges }))
}

/// read_path(from, to)：无向 BFS 最短路径，返回节点序列 + 关系叙述化 narrative
pub fn read_path(ctx: &McpContext, from: &str, to: &str) -> Result<Value, String> {
    let snap = ctx.scan()?;
    let by_id: HashMap<&str, &crate::model::node::Node> =
        snap.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    if !by_id.contains_key(from) {
        return Err(format!("节点 {from} 不存在"));
    }
    if !by_id.contains_key(to) {
        return Err(format!("节点 {to} 不存在"));
    }
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    for e in &snap.edges {
        adj.entry(e.parent.as_str()).or_default().push(e.child.as_str());
        adj.entry(e.child.as_str()).or_default().push(e.parent.as_str());
    }
    // BFS 记录前驱
    let mut prev: HashMap<&str, &str> = HashMap::new();
    let mut visited: HashSet<&str> = HashSet::from([from]);
    let mut queue: VecDeque<&str> = VecDeque::from([from]);
    while let Some(cur) = queue.pop_front() {
        if cur == to {
            break;
        }
        if let Some(neis) = adj.get(cur) {
            for nei in neis {
                if visited.insert(*nei) {
                    prev.insert(*nei, cur);
                    queue.push_back(*nei);
                }
            }
        }
    }
    if !visited.contains(to) {
        return Ok(json!({
            "found": false,
            "hint": format!("{from} 与 {to} 在当前图中不连通"),
        }));
    }
    let mut path_ids: Vec<&str> = vec![to];
    while *path_ids.last().unwrap() != from {
        path_ids.push(prev[path_ids.last().unwrap()]);
    }
    path_ids.reverse();

    let path: Vec<Value> = path_ids
        .iter()
        .map(|id| {
            let n = by_id[id];
            json!({"id": n.id, "title": n.title, "type": n.node_type, "status": n.status})
        })
        .collect();
    // 关系叙述化：v.parent==u → u --rel_v--> v；u.parent==v → u <--rel_u-- v
    let mut narrative = String::new();
    for (i, w) in path_ids.windows(2).enumerate() {
        let (u, v) = (w[0], w[1]);
        let (un, vn) = (by_id[u], by_id[v]);
        if i > 0 {
            narrative.push(' ');
        }
        narrative.push_str(&format!("{}", un.title));
        if vn.parent.as_deref() == Some(u) {
            narrative.push_str(&format!(" --{}--> ", vn.rel.as_deref().unwrap_or("contains")));
        } else {
            narrative.push_str(&format!(" <--{}-- ", un.rel.as_deref().unwrap_or("contains")));
        }
        if i == path_ids.len() - 2 {
            narrative.push_str(&vn.title);
        }
    }
    Ok(json!({
        "found": true,
        "hops": path_ids.len() - 1,
        "path": path,
        "narrative": narrative,
    }))
}

/// get_guide()：当前模式的 AI 使用指南全文 + 版本号
pub fn get_guide(ctx: &McpContext) -> Result<Value, String> {
    let mode = ctx.mode_str();
    Ok(json!({
        "mode": mode,
        "version": ctx.guide_version(),
        "content": ai_guide::guide_for(Some(mode)),
    }))
}

// ── 写入工具（D3 保护 + D4 提示）────────────────────────────

/// create_node(title, body?, tags?, force?)：开发模式新建知识节点（类型 note）
/// - title 重复（忽略大小写）时拒绝，force=true 放行；id 冲突永远拒绝
pub fn create_node(
    ctx: &McpContext,
    title: &str,
    body: Option<&str>,
    tags: Option<Vec<String>>,
    force: Option<bool>,
) -> Result<Value, String> {
    if !ctx.mode.is_dev() {
        return Err("仅开发模式工作区可自由新建节点；分析模式的链由 AI 按协议维护（本工作区可用 update_node）".into());
    }
    let title = title.trim();
    if title.is_empty() {
        return Err("title 不能为空".into());
    }
    if title.contains('\n') || title.contains('\r') {
        return Err("title 必须为单行文本".into());
    }
    // 重复标题检测（忽略大小写）——记忆层防呆，force 显式放行
    let snap = ctx.scan()?;
    let lower = title.to_lowercase();
    if let Some(dup) = snap.nodes.iter().find(|n| n.title.trim().to_lowercase() == lower) {
        if !force.unwrap_or(false) {
            return Err(format!(
                "DUPLICATE_TITLE: 已存在同名节点 {}「{}」。若确为同一记忆请 update_node 补充；确认要另建请传 force=true",
                dup.id, dup.title
            ));
        }
    }
    let id = graph_edit::auto_id(&ctx.nodes_dir());
    let path = ctx.node_path(&id);
    if path.exists() {
        return Err(format!("节点 {id} 已存在"));
    }

    use serde_yaml::Value as YV;
    let now = frontmatter::now_iso8601();
    let mut fm = serde_yaml::Mapping::new();
    fm.insert(YV::String("id".into()), YV::String(id.clone()));
    fm.insert(YV::String("type".into()), YV::String("note".into()));
    fm.insert(YV::String("title".into()), YV::String(title.to_string()));
    fm.insert(YV::String("parent".into()), YV::Null);
    fm.insert(YV::String("status".into()), YV::String("none".into()));
    fm.insert(YV::String("created".into()), YV::String(now.clone()));
    fm.insert(YV::String("updated".into()), YV::String(now));
    fm.insert(YV::String("revision".into()), YV::Number(1u64.into()));
    fm.insert(
        YV::String("tags".into()),
        YV::Sequence(tags.unwrap_or_default().into_iter().map(YV::String).collect()),
    );
    let body_text = match body {
        Some(b) if !b.trim().is_empty() => b.trim().to_string(),
        _ => format!("# {title}"),
    };
    let content = frontmatter::serialize(&fm, &body_text).map_err(|e| format!("序列化失败：{e}"))?;
    atomic_write(&path, &content)?;

    Ok(json!({
        "created": true,
        "id": id,
        "title": title,
        "file": format!(".chain/nodes/{id}.md"),
        "hint": format!("节点已创建（AI 指南 v{}）。建立链接用 link_nodes（rel_type 仅 contains/solves/alternative）；更新内容用 update_node，建议先 read_node 取 updated 并传 expected_updated 防并发覆盖。完整规范见 get_guide", ctx.guide_version()),
    }))
}

/// update_node(id, mode=append|replace_body, content, expected_updated?)
/// D3 乐观锁：expected_updated 与文件当前 updated 不符 → CONFLICT 不落盘
pub fn update_node(
    ctx: &McpContext,
    id: &str,
    mode: &str,
    content: &str,
    expected_updated: Option<&str>,
) -> Result<Value, String> {
    if !matches!(mode, "append" | "replace_body") {
        return Err(format!("mode 仅支持 append / replace_body，收到：{mode}"));
    }
    if !graph_edit::is_safe_id(id) {
        return Err("节点 id 非法".into());
    }
    let path = ctx.node_path(id);
    if !path.exists() {
        return Err(format!("节点 {id} 不存在"));
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| format!("读取失败：{e}"))?;
    let (mut fm, body) = if ctx.mode.is_dev() {
        parse_lenient(&raw, id)?
    } else {
        frontmatter::parse(&raw).map_err(|e| format!("解析 frontmatter 失败：{e}"))?
    };

    // D3 乐观锁：先比对再动手，CONFLICT 不落盘
    if let Some(expected) = expected_updated {
        let current = fm_get_str(&fm, "updated").unwrap_or_default();
        if current != expected {
            return Err(format!(
                "CONFLICT: 节点 {id} 的 updated 已变为 {current}（你期望 {expected}）——节点被其他方修改过，本次未落盘。请 read_node 获取最新内容后重试"
            ));
        }
    }

    let new_body = match mode {
        "append" => {
            let old = body.trim_end();
            if old.is_empty() {
                content.trim().to_string()
            } else {
                format!("{old}\n\n{}", content.trim())
            }
        }
        _ => content.trim().to_string(),
    };
    // 分析模式严格：body 不能为空（与 GUI update_node 一致）
    if !ctx.mode.is_dev() && new_body.trim().is_empty() {
        return Err("body 不能为空（分析模式）".into());
    }

    // 字段不动，仅借 apply_update 完成 revision+1 与 updated 刷新
    let fields = UpdateFields {
        title: None,
        status: None,
        body: None,
        tags: None,
        evidence: None,
        parent: None,
        rel: None,
    };
    crate::model::node::apply_update(&mut fm, &fields).map_err(|e| format!("应用更新失败：{e}"))?;
    let new_content = frontmatter::serialize(&fm, &new_body).map_err(|e| format!("序列化失败：{e}"))?;
    atomic_write(&path, &new_content)?;

    Ok(json!({
        "updated": true,
        "id": id,
        "mode": mode,
        "revision": fm_get_str(&fm, "revision"),
        "updated_at": fm_get_str(&fm, "updated"),
        "hint": format!("更新已原子落盘（AI 指南 v{}）。正文请保持结构化小节；并发场景务必先 read_node 并传 expected_updated", ctx.guide_version()),
    }))
}

/// link_nodes(from, to, rel_type, desc?)：建立 from(父) → to(子) 链接
/// D1：rel 写入子节点 frontmatter 单值；desc 写入可选 rel_desc（空串=清除，None=不动）
/// D2：rel_type 严格校验，仅 contains / solves / alternative
pub fn link_nodes(
    ctx: &McpContext,
    from: &str,
    to: &str,
    rel_type: &str,
    desc: Option<&str>,
) -> Result<Value, String> {
    if !ctx.mode.is_dev() {
        return Err("仅开发模式工作区可自由建链；分析模式的链由 AI 按协议维护".into());
    }
    if !REL_TYPES.contains(&rel_type) {
        return Err(format!(
            "rel_type 非法「{rel_type}」，仅支持：{}（语义见 get_guide）",
            REL_TYPES.join(" / ")
        ));
    }
    if from == to {
        return Err("不允许自环（from 与 to 相同）".into());
    }
    if !graph_edit::is_safe_id(from) || !graph_edit::is_safe_id(to) {
        return Err("节点 id 非法".into());
    }
    if !ctx.node_path(from).exists() {
        return Err(format!("父节点 {from} 不存在"));
    }
    let to_path = ctx.node_path(to);
    if !to_path.exists() {
        return Err(format!("子节点 {to} 不存在"));
    }

    let raw = std::fs::read_to_string(&to_path).map_err(|e| format!("读取失败：{e}"))?;
    let (mut fm, body) = parse_lenient(&raw, to)?;
    let fields = UpdateFields {
        title: None,
        status: None,
        body: None,
        tags: None,
        evidence: None,
        parent: Some(Some(from.to_string())),
        rel: Some(rel_type.to_string()),
    };
    crate::model::node::apply_update(&mut fm, &fields).map_err(|e| format!("应用更新失败：{e}"))?;
    // D1：可选边说明 rel_desc（Some(空)=清除；Some(值)=设置；None=不动）
    use serde_yaml::Value as YV;
    let rel_desc_out = match desc {
        Some(d) if d.trim().is_empty() => {
            fm.remove(&YV::String("rel_desc".into()));
            Value::Null
        }
        Some(d) => {
            fm.insert(YV::String("rel_desc".into()), YV::String(d.trim().to_string()));
            json!(d.trim())
        }
        None => fm_get_str(&fm, "rel_desc").map(|s| json!(s)).unwrap_or(Value::Null),
    };
    let new_content = frontmatter::serialize(&fm, &body).map_err(|e| format!("序列化失败：{e}"))?;
    atomic_write(&to_path, &new_content)?;

    Ok(json!({
        "linked": true,
        "from": from,
        "to": to,
        "rel": rel_type,
        "rel_desc": rel_desc_out,
        "hint": format!("链接已建立（rel 写入子节点 frontmatter 单值，AI 指南 v{}）。rel_type 仅 contains/solves/alternative，语义详见 get_guide", ctx.guide_version()),
    }))
}

// ── 测试 ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup(mode: &str) -> TempDir {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".chain").join("nodes")).unwrap();
        fs::write(tmp.path().join(".chain").join(".mode"), mode).unwrap();
        tmp
    }

    fn ctx_of(tmp: &TempDir) -> McpContext {
        McpContext::open(tmp.path().to_path_buf()).unwrap()
    }

    fn write_node(tmp: &TempDir, id: &str, title: &str, parent: &str, rel: &str) {
        let content = format!(
            "---\nid: {id}\ntype: note\ntitle: {title}\nparent: {parent}\nrel: {rel}\nstatus: none\ncreated: 2026-09-01T10:00:00+08:00\nupdated: 2026-09-01T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# {title}\n"
        );
        fs::write(
            tmp.path().join(".chain").join("nodes").join(format!("{id}.md")),
            content,
        )
        .unwrap();
    }

    #[test]
    fn ctx_open_rejects_untagged() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".chain").join("nodes")).unwrap();
        assert!(McpContext::open(tmp.path().to_path_buf()).is_err(), "无 .mode 标签应拒绝");
    }

    #[test]
    fn create_and_read_node() {
        let tmp = setup("dev");
        let ctx = ctx_of(&tmp);
        let r = create_node(&ctx, "Rust 笔记", Some("所有权与借用"), Some(vec!["rust".into()]), None).unwrap();
        assert_eq!(r["id"], "node-1");
        assert!(tmp.path().join(".chain/nodes/node-1.md").exists());

        let v = read_node(&ctx, "node-1", Some(true)).unwrap();
        assert_eq!(v["title"], "Rust 笔记");
        assert_eq!(v["body"].as_str().unwrap(), "所有权与借用");
        assert_eq!(v["tags"][0], "rust");
        assert!(v["neighbors"]["children"].is_array());
    }

    #[test]
    fn create_duplicate_title_requires_force() {
        let tmp = setup("dev");
        let ctx = ctx_of(&tmp);
        create_node(&ctx, "重复标题", None, None, None).unwrap();
        let err = create_node(&ctx, " 重复标题 ", None, None, None).unwrap_err();
        assert!(err.starts_with("DUPLICATE_TITLE"), "应拦截同名：{err}");
        // force 放行
        let r = create_node(&ctx, "重复标题", None, None, Some(true)).unwrap();
        assert_eq!(r["id"], "node-2");
    }

    #[test]
    fn create_rejects_analysis_mode() {
        let tmp = setup("analysis");
        let ctx = ctx_of(&tmp);
        assert!(create_node(&ctx, "x", None, None, None).is_err());
    }

    #[test]
    fn update_append_then_replace() {
        let tmp = setup("dev");
        let ctx = ctx_of(&tmp);
        create_node(&ctx, "正文测试", Some("第一段"), None, None).unwrap();

        let r1 = update_node(&ctx, "node-1", "append", "第二段", None).unwrap();
        assert_eq!(r1["updated"], true);
        let v = read_node(&ctx, "node-1", None).unwrap();
        assert_eq!(v["body"].as_str().unwrap(), "第一段\n\n第二段");
        assert_eq!(v["revision"], 2);

        update_node(&ctx, "node-1", "replace_body", "全新正文", None).unwrap();
        let v = read_node(&ctx, "node-1", None).unwrap();
        assert_eq!(v["body"].as_str().unwrap(), "全新正文");
        // 连续写同一文件成功 = Windows rename 替换语义验证
    }

    #[test]
    fn update_optimistic_lock_conflict_not_written() {
        let tmp = setup("dev");
        let ctx = ctx_of(&tmp);
        create_node(&ctx, "锁测试", Some("原文"), None, None).unwrap();
        let v = read_node(&ctx, "node-1", None).unwrap();
        let updated = v["updated"].as_str().unwrap().to_string();

        // 正确的 expected_updated → 通过
        update_node(&ctx, "node-1", "append", "安全追加", Some(&updated)).unwrap();

        // 过期的 expected_updated → CONFLICT 且不落盘
        // 注：updated 为秒级精度，同秒内多次写入值不变——过期值须用必然不同的固定旧时间戳
        let err = update_node(&ctx, "node-1", "replace_body", "恶意覆盖", Some("2000-01-01T00:00:00+08:00")).unwrap_err();
        assert!(err.starts_with("CONFLICT"), "应报 CONFLICT：{err}");
        let v2 = read_node(&ctx, "node-1", None).unwrap();
        assert_eq!(v2["body"].as_str().unwrap(), "原文\n\n安全追加", "CONFLICT 不得落盘");

        // 原子写无 .tmp 残留
        let residue: Vec<_> = fs::read_dir(tmp.path().join(".chain").join("nodes"))
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(residue.is_empty(), "不应残留 .tmp 文件");
    }

    #[test]
    fn update_rejects_bad_mode_and_empty_analysis_body() {
        let tmp = setup("dev");
        let ctx = ctx_of(&tmp);
        create_node(&ctx, "m", None, None, None).unwrap();
        assert!(update_node(&ctx, "node-1", "overwrite", "x", None).is_err());

        let tmp2 = setup("analysis");
        fs::write(
            tmp2.path().join(".chain/nodes/g-001.md"),
            "---\nid: g-001\ntype: goal\ntitle: 目标\nparent: null\nstatus: pending\ncreated: 2026-09-01T10:00:00+08:00\nupdated: 2026-09-01T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# 目标\n",
        ).unwrap();
        let ctx2 = ctx_of(&tmp2);
        assert!(update_node(&ctx2, "g-001", "replace_body", "  ", None).is_err(), "分析模式空 body 应拒绝");
    }

    #[test]
    fn link_nodes_full_flow() {
        let tmp = setup("dev");
        let ctx = ctx_of(&tmp);
        create_node(&ctx, "父", None, None, None).unwrap();
        create_node(&ctx, "子", None, None, None).unwrap();
        let r = link_nodes(&ctx, "node-1", "node-2", "solves", Some("子节点补父节点短板")).unwrap();
        assert_eq!(r["linked"], true);
        assert_eq!(r["rel"], "solves");

        let snap = ctx.scan().unwrap();
        assert_eq!(snap.edges.len(), 1);
        assert_eq!(snap.edges[0].rel, "solves");
        let child = snap.nodes.iter().find(|n| n.id == "node-2").unwrap();
        assert_eq!(child.rel_desc.as_deref(), Some("子节点补父节点短板"));

        // 空 desc 清除 rel_desc
        link_nodes(&ctx, "node-1", "node-2", "contains", Some("  ")).unwrap();
        let child = read_node(&ctx, "node-2", None).unwrap();
        assert!(child["rel_desc"].is_null(), "空 desc 应清除 rel_desc");
    }

    #[test]
    fn link_rejects_bad_rel_and_missing_node() {
        let tmp = setup("dev");
        let ctx = ctx_of(&tmp);
        create_node(&ctx, "A", None, None, None).unwrap();
        create_node(&ctx, "B", None, None, None).unwrap();
        assert!(link_nodes(&ctx, "node-1", "node-2", "depends_on", None).is_err(), "D2 词表外应拒绝");
        assert!(link_nodes(&ctx, "node-1", "ghost", "contains", None).is_err());
        assert!(link_nodes(&ctx, "node-1", "node-1", "contains", None).is_err(), "自环应拒绝");
    }

    #[test]
    fn link_rejects_analysis_mode() {
        let tmp = setup("analysis");
        let ctx = ctx_of(&tmp);
        assert!(link_nodes(&ctx, "a", "b", "contains", None).is_err());
    }

    #[test]
    fn search_and_expand_and_path() {
        let tmp = setup("dev");
        write_node(&tmp, "a", "根节点", "null", "contains");
        write_node(&tmp, "b", "中间节点", "a", "contains");
        write_node(&tmp, "c", "叶子节点", "b", "solves");
        let ctx = ctx_of(&tmp);

        // search
        let r = search(&ctx, "叶子", None).unwrap();
        assert_eq!(r["total"], 1);
        assert_eq!(r["results"][0]["id"], "c");
        assert_eq!(r["results"][0]["matched_on"][0], "title");

        // expand depth=1 从中间节点出发应见三个节点
        let e = expand(&ctx, "b", None).unwrap();
        assert_eq!(e["node_count"], 3);
        assert!(expand(&ctx, "b", Some(3)).is_err(), "depth>2 应拒绝");

        // read_path 根到叶
        let p = read_path(&ctx, "a", "c").unwrap();
        assert_eq!(p["found"], true);
        assert_eq!(p["hops"], 2);
        let narrative = p["narrative"].as_str().unwrap();
        assert!(narrative.contains("--contains-->"), "叙述应含正向边：{narrative}");
        assert!(narrative.contains("--solves-->"), "叙述应含 solves：{narrative}");

        // 不连通
        write_node(&tmp, "island", "孤岛", "null", "contains");
        let p2 = read_path(&ctx, "a", "island").unwrap();
        assert_eq!(p2["found"], false);
    }

    #[test]
    fn get_overview_and_guide() {
        let tmp = setup("dev");
        write_node(&tmp, "a", "根", "null", "contains");
        let ctx = ctx_of(&tmp);

        let o = get_overview(&ctx).unwrap();
        assert_eq!(o["mode"], "dev");
        assert_eq!(o["node_count"], 1);
        assert_eq!(o["guide_version"], ai_guide::AI_GUIDE_DEV_VERSION);

        let g = get_guide(&ctx).unwrap();
        assert_eq!(g["version"], ai_guide::AI_GUIDE_DEV_VERSION);
        assert!(g["content"].as_str().unwrap().contains("知识库搭建"));
    }
}
