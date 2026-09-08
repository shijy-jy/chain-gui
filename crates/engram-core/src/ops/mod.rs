//! Engram 核心操作层（规划书 v1.1 阶段一 M2-M4）：
//! 不依赖 tauri / rmcp 的纯函数工具集，供 engram-mcp / engram-gui 入口做适配。
//! - 工具清单 v1：只读 6（get_overview/search/read_node/expand/read_path/get_guide）
//!   + 写入 3（create_node/update_node/link_nodes）
//! - D3 并发写保护：expected_updated 乐观锁（CONFLICT 不落盘）+ tmp/rename 原子写；
//!   跨请求串行化由 MCP bin 侧 write_lock 保证
//! - D4：写入类工具返回体附 hint 规范提示；指南版本经 bin 握手 instructions 下发
//! - D1：rel 保持子节点 frontmatter 单值，边说明走可选 rel_desc（不引入边列表）
//! - D2：rel_type 仅放行 contains / solves / alternative（严格校验，非法值报错）

pub mod chain;
pub mod node_edit;

use serde_json::{json, Value};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use crate::guide::{guide_for, guide_version_for};
use crate::model::{ScanMode, UpdateFields};
use crate::ops::node_edit::{auto_id, is_safe_id};
use crate::profile::{mode_str, REL_TYPES};
use crate::scanner::{frontmatter, walker};
use crate::workspace::read_mode_tag;

// ── server 上下文 ─────────────────────────────────────────

/// 工作区上下文：入口（MCP --workspace / GUI 命令）打开时建立，结构只读；
/// stats/index 经 Mutex 内部可变（惰性派生物状态，框架 §5.10）
pub struct Workspace {
    pub root: PathBuf,
    pub mode: ScanMode,
    pub(crate) stats: std::sync::Mutex<crate::stats::StatsStore>,
    pub(crate) index: std::sync::Mutex<crate::index::IndexStore>,
}

impl Workspace {
    /// 校验目标目录是已打标的 Engram 工作区（.chain/.mode = dev/analysis）。
    /// 无 .chain 或无标签 → 拒绝服务（不给来历不明的目录乱写）。
    pub fn open(root: PathBuf) -> Result<Self, String> {
        if !root.join(".chain").is_dir() {
            return Err(format!(
                "目录 {} 下不存在 .chain/，不是 Engram 工作区",
                root.display()
            ));
        }
        let mode = read_mode_tag(&root).ok_or_else(|| {
            format!(
                "工作区 {} 缺少 .chain/.mode 标签（dev/analysis）——请先在 Engram GUI 添加该工作区完成打标",
                root.display()
            )
        })?;
        // 宪法第 9 条：旧软件遇更高 major 一律拒绝打开（MCP 只读 .schema，不 adoption 写）
        crate::schema::check_openable(&root)?;
        let stats = crate::stats::StatsStore::open(&root)?;
        let index = crate::index::IndexStore::open(&root)?;
        Ok(Self {
            root,
            mode,
            stats: std::sync::Mutex::new(stats),
            index: std::sync::Mutex::new(index),
        })
    }

    /// 全局记忆时钟：每次工具调用 +1（ADR 0008；由各工具入口调用）
    pub fn bump_clock(&self) -> Result<(), String> {
        self.stats
            .lock()
            .map_err(|e| format!("stats 锁失败：{e}"))?
            .bump_memory_clock()
    }

    /// 写入触达回写 + 落盘（写工具成功后调用）
    pub fn touch_write(&self, id: &str) -> Result<(), String> {
        let mut st = self
            .stats
            .lock()
            .map_err(|e| format!("stats 锁失败：{e}"))?;
        st.touch(id, crate::stats::TouchKind::Write)?;
        st.flush()
    }

    /// 读触达回写 + 落盘（读工具命中后调用；框架 T3：读命中是节点触达）
    pub fn touch_read(&self, id: &str) -> Result<(), String> {
        let mut st = self
            .stats
            .lock()
            .map_err(|e| format!("stats 锁失败：{e}"))?;
        st.touch(id, crate::stats::TouchKind::ReadHit)?;
        st.flush()
    }

    /// 写路径标 index 条目 stale + 落盘（框架 §5.10：写入成功后触发 index 条目标 stale；
    /// 缺失条目无副作用——recall 的「索引中无此 id → 按需重嵌」路径覆盖新建节点）
    pub fn mark_index_stale(&self, id: &str) -> Result<(), String> {
        let mut ix = self.index.lock().map_err(|e| format!("索引锁失败：{e}"))?;
        ix.mark_stale(id)?;
        ix.flush()
    }

    /// 审计追加（框架 §4/T15：append-only 派生物；失败不阻断主写入——打 stderr 继续）
    pub fn audit(&self, action: &str, node_id: &str, detail: &str) {
        if let Err(e) = crate::audit::append(&self.root, action, node_id, detail) {
            eprintln!("[engram] audit 写入失败：{e}");
        }
    }

    /// CONFLICT 计数（框架 §6 指标采集点，落 stats calibrate 区）
    pub fn record_conflict(&self) -> Result<(), String> {
        let mut st = self
            .stats
            .lock()
            .map_err(|e| format!("stats 锁失败：{e}"))?;
        st.record_conflict()?;
        st.flush()
    }

    pub fn mode_str(&self) -> &'static str {
        mode_str(self.mode)
    }

    pub fn guide_version(&self) -> u32 {
        guide_version_for(Some(self.mode_str()))
    }

    fn nodes_dir(&self) -> PathBuf {
        self.root.join(".chain").join("nodes")
    }

    fn node_path(&self, id: &str) -> PathBuf {
        self.nodes_dir().join(format!("{id}.md"))
    }

    fn archive_dir(&self) -> PathBuf {
        self.root.join(".chain").join(crate::ops::chain::ARCHIVE_DIR)
    }

    fn archive_path(&self, id: &str) -> PathBuf {
        self.archive_dir().join(format!("{id}.md"))
    }

    pub(crate) fn scan(&self) -> Result<crate::model::chain::ChainSnapshot, String> {
        walker::scan_chain_dir_mode(&self.root, self.mode).map_err(|e| format!("扫描失败：{e}"))
    }
}

// ── 写入保护原语（D3）──────────────────────────────────────

/// tmp/rename 原子写：先写同目录隐藏 .tmp 再 rename 替换，杜绝半截文件。
/// Windows 上 Rust rename 为替换语义（MOVEFILE_REPLACE_EXISTING），
/// 由测试 update_append_then_replace（连续两次写同一文件）覆盖验证。
/// 唯一写路径原语：GUI 侧写入也走这里。
pub fn atomic_write(path: &Path, content: &str) -> Result<(), String> {
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

/// 二进制原子写（同 atomic_write 的 tmp/rename 语义；索引向量等非 UTF-8 数据专用，
/// 严禁经 String 转换——from_utf8_lossy 会改写字节破坏数据）
pub fn atomic_write_bytes(path: &Path, content: &[u8]) -> Result<(), String> {
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
/// 核心唯一写路径共用原语（GUI set_parent / update_node / MCP 写入工具）
pub fn parse_lenient(raw: &str, node_id: &str) -> Result<(serde_yaml::Mapping, String), String> {
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
    fm.get(serde_yaml::Value::String(key.into()))
        .and_then(|v| v.as_str().map(|s| s.to_string()))
}

fn fm_get_bool(fm: &serde_yaml::Mapping, key: &str) -> bool {
    fm.get(serde_yaml::Value::String(key.into()))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// 标题归一化（T8 阶段一用：大小写/空白不敏感）
fn normalize_title_key(t: &str) -> String {
    t.chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_lowercase()
}

/// 冻结门禁（ADR 0003）：frozen 节点拒绝一切 MCP 写入（人工裁决后恢复）
fn ensure_not_frozen(fm: &serde_yaml::Mapping, id: &str) -> Result<(), String> {
    if fm_get_bool(fm, "frozen") {
        return Err(format!(
            "节点 {id} 已冻结 [待裁决]（并发写冲突待人工裁决，拒绝写入）。恢复方式：编辑节点文件去除 frozen 标记并修正内容，或在 GUI 修改标题/状态"
        ));
    }
    Ok(())
}

// ── 只读工具 ──────────────────────────────────────────────

/// get_overview()：全局概览——规模 + 活跃链 + 健康度 + 模式与指南版本
pub fn get_overview(ctx: &Workspace) -> Result<Value, String> {
    ctx.bump_clock()?;
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

/// search(query, limit=10)：title/tags/body 大小写不敏感子串匹配。
/// M7'：命中结果触达回写 stats（读触达全覆盖）；内部调用（recall 关键词降级）走
/// search_impl(touch=false) 避免与 recall 自身的触达回写重复计数。
pub fn search(ctx: &Workspace, query: &str, limit: Option<usize>) -> Result<Value, String> {
    search_impl(ctx, query, limit, true)
}

pub(crate) fn search_impl(
    ctx: &Workspace,
    query: &str,
    limit: Option<usize>,
    touch_stats: bool,
) -> Result<Value, String> {
    ctx.bump_clock()?;
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
            hits.push((
                score,
                json!({
                    "id": n.id,
                    "title": n.title,
                    "type": n.node_type,
                    "status": n.status,
                    "matched_on": matched_on,
                    "snippet": frontmatter::truncate_utf8(n.body.trim(), 160),
                    "updated": n.updated,
                }),
            ));
        }
    }
    // 命中字段权重优先，同级按 updated 倒序；同秒再按 id 升序（跨平台确定性，避免
    // 文件系统枚举顺序差异导致 golden 漂移——recall 关键词降级复用本排序）
    hits.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| {
                let ua = a.1["updated"].as_str().unwrap_or("").to_string();
                let ub = b.1["updated"].as_str().unwrap_or("").to_string();
                ub.cmp(&ua)
            })
            .then_with(|| {
                let ia = a.1["id"].as_str().unwrap_or("");
                let ib = b.1["id"].as_str().unwrap_or("");
                ia.cmp(ib)
            })
    });
    let total = hits.len();
    let results: Vec<Value> = hits.into_iter().take(limit).map(|(_, v)| v).collect();
    if touch_stats {
        // 读触达：每个返回命中回写（T3）；stats 为派生物，失败不吞检索结果
        for r in &results {
            if let Some(id) = r["id"].as_str() {
                let _ = ctx.touch_read(id);
            }
        }
    }
    Ok(json!({ "query": query, "total": total, "returned": results.len(), "results": results }))
}

/// read_node(id, include_neighbors=false)：单节点全字段 + 正文；neighbors 附父与子
pub fn read_node(
    ctx: &Workspace,
    id: &str,
    include_neighbors: Option<bool>,
) -> Result<Value, String> {
    ctx.bump_clock()?;
    if !is_safe_id(id) {
        return Err("节点 id 非法（仅允许字母/数字/连字符/下划线）".into());
    }
    let snap = ctx.scan()?;
    // M7'：活跃图优先，其次归档列表（归档节点全文仍可读，检索自 L4 起可见）
    let node = snap
        .nodes
        .iter()
        .find(|n| n.id == id)
        .or_else(|| snap.archived.iter().find(|n| n.id == id))
        .ok_or_else(|| format!("节点 {id} 不存在"))?;
    let _ = ctx.touch_read(&node.id); // 读触达（T3；失败不吞读取结果）
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
pub fn expand(ctx: &Workspace, id: &str, depth: Option<u32>) -> Result<Value, String> {
    ctx.bump_clock()?;
    let depth = depth.unwrap_or(1);
    if !(1..=2).contains(&depth) {
        return Err(format!(
            "depth 仅支持 1 或 2（防大图上响应膨胀），收到：{depth}"
        ));
    }
    let snap = ctx.scan()?;
    if !snap.nodes.iter().any(|n| n.id == id) {
        return Err(format!("节点 {id} 不存在"));
    }
    let _ = ctx.touch_read(id); // 读触达（T3）
    // 无向邻接表
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    for e in &snap.edges {
        adj.entry(e.parent.as_str())
            .or_default()
            .push(e.child.as_str());
        adj.entry(e.child.as_str())
            .or_default()
            .push(e.parent.as_str());
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
        .map(|n| {
            json!({
                "id": n.id, "title": n.title, "type": n.node_type,
                "status": n.status, "parent": n.parent, "rel": n.rel,
            })
        })
        .collect();
    let edges: Vec<Value> = snap
        .edges
        .iter()
        .filter(|e| visited.contains(e.parent.as_str()) && visited.contains(e.child.as_str()))
        .map(|e| json!({"parent": e.parent, "child": e.child, "rel": e.rel}))
        .collect();
    Ok(
        json!({ "center": id, "depth": depth, "node_count": nodes.len(), "nodes": nodes, "edges": edges }),
    )
}

/// read_path(from, to)：无向 BFS 最短路径，返回节点序列 + 关系叙述化 narrative
pub fn read_path(ctx: &Workspace, from: &str, to: &str) -> Result<Value, String> {
    ctx.bump_clock()?;
    let snap = ctx.scan()?;
    let by_id: HashMap<&str, &crate::model::node::Node> =
        snap.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    if !by_id.contains_key(from) {
        return Err(format!("节点 {from} 不存在"));
    }
    if !by_id.contains_key(to) {
        return Err(format!("节点 {to} 不存在"));
    }
    let _ = ctx.touch_read(from); // 读触达（T3）：路径两端点
    let _ = ctx.touch_read(to);
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    for e in &snap.edges {
        adj.entry(e.parent.as_str())
            .or_default()
            .push(e.child.as_str());
        adj.entry(e.child.as_str())
            .or_default()
            .push(e.parent.as_str());
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
        narrative.push_str(&un.title.to_string());
        if vn.parent.as_deref() == Some(u) {
            narrative.push_str(&format!(
                " --{}--> ",
                vn.rel.as_deref().unwrap_or("contains")
            ));
        } else {
            narrative.push_str(&format!(
                " <--{}-- ",
                un.rel.as_deref().unwrap_or("contains")
            ));
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
pub fn get_guide(ctx: &Workspace) -> Result<Value, String> {
    ctx.bump_clock()?;
    let mode = ctx.mode_str();
    Ok(json!({
        "mode": mode,
        "version": ctx.guide_version(),
        "content": guide_for(Some(mode)),
    }))
}

// ── 写入工具（D3 保护 + D4 提示）────────────────────────────

/// create_node(title, body?, tags?, force?)：开发模式新建知识节点（类型 note）
/// - title 重复（忽略大小写）时拒绝，force=true 放行；id 冲突永远拒绝
/// - v2.11 M8' T8 重复检测两阶段：①标题归一化包含关系（公共子串启发式保守形态，零依赖）
///   ②候选存在时嵌入余弦 > 0.9 → duplicate_hint + alternative 竞争边；force 跳过检测
pub fn create_node(
    ctx: &Workspace,
    title: &str,
    body: Option<&str>,
    tags: Option<Vec<String>>,
    force: Option<bool>,
) -> Result<Value, String> {
    create_node_impl(ctx, title, body, tags, force, None)
}

pub(crate) fn create_node_impl(
    ctx: &Workspace,
    title: &str,
    body: Option<&str>,
    tags: Option<Vec<String>>,
    force: Option<bool>,
    embedder_override: Option<&dyn crate::embed::Embedder>,
) -> Result<Value, String> {
    ctx.bump_clock()?;
    if !ctx.mode.is_dev() {
        return Err("WORKSPACE_MODE_MISMATCH: 仅开发模式工作区可自由新建节点；分析模式的链由 AI 按协议维护（本工作区可用 update_node）".into());
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
    if let Some(dup) = snap
        .nodes
        .iter()
        .find(|n| n.title.trim().to_lowercase() == lower)
    {
        if !force.unwrap_or(false) {
            return Err(format!(
                "DUPLICATE_TITLE: 已存在同名节点 {}「{}」。若确为同一记忆请 update_node 补充；确认要另建请传 force=true",
                dup.id, dup.title
            ));
        }
    }

    // ── T8 重复检测两阶段（force 跳过）──
    let force = force.unwrap_or(false);
    let mut suspected: Vec<(String, f32)> = Vec::new(); // (id, cosine)
    if !force {
        // 阶段一：归一化包含关系（公共子串启发式的保守形态；排除精确同名——已在上方拦截）
        let key = normalize_title_key(title);
        let candidates: Vec<&crate::model::node::Node> = snap
            .nodes
            .iter()
            .filter(|n| {
                let k = normalize_title_key(&n.title);
                k != key && (k.contains(&key) || key.contains(&k))
            })
            .collect();
        if !candidates.is_empty() {
            // 阶段二：嵌入余弦 > 0.9（框架 §9 拍板阈值；模型不可用 → 仅阶段一，不误报）
            let loaded;
            let embedder: Option<&dyn crate::embed::Embedder> = if let Some(e) = embedder_override {
                Some(e)
            } else {
                loaded = crate::embed::try_load_embedder();
                loaded.as_deref()
            };
            if let Some(e) = embedder {
                let new_text = format!("{title}\n{}", body.unwrap_or(title));
                let mut texts = vec![new_text];
                texts.extend(candidates.iter().map(|n| format!("{}\n{}", n.title, n.body)));
                if let Ok(vecs) = e.embed(&texts) {
                    if let Some((qv, rest)) = vecs.split_first() {
                        for (n, v) in candidates.iter().zip(rest) {
                            if v.len() == qv.len() {
                                let s: f32 = qv.iter().zip(v.iter()).map(|(a, b)| a * b).sum();
                                if s > 0.9 {
                                    suspected.push((n.id.clone(), s));
                                }
                            }
                        }
                    }
                }
                suspected.sort_by(|a, b| {
                    b.1.partial_cmp(&a.1)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| a.0.cmp(&b.0))
                });
            }
        }
    }

    let id = auto_id(&ctx.nodes_dir());
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
    // 疑似重复 → alternative 竞争边（复用 D1 边语义表达竞争，T8）
    let (parent, rel) = match suspected.first() {
        Some((dup_id, _)) => (YV::String(dup_id.clone()), Some("alternative")),
        None => (YV::Null, None),
    };
    fm.insert(YV::String("parent".into()), parent);
    if let Some(r) = rel {
        fm.insert(YV::String("rel".into()), YV::String(r.to_string()));
    }
    fm.insert(YV::String("status".into()), YV::String("none".into()));
    fm.insert(YV::String("created".into()), YV::String(now.clone()));
    fm.insert(YV::String("updated".into()), YV::String(now));
    fm.insert(YV::String("revision".into()), YV::Number(1u64.into()));
    fm.insert(
        YV::String("tags".into()),
        YV::Sequence(
            tags.unwrap_or_default()
                .into_iter()
                .map(YV::String)
                .collect(),
        ),
    );
    let body_text = match body {
        Some(b) if !b.trim().is_empty() => b.trim().to_string(),
        _ => format!("# {title}"),
    };
    let content =
        frontmatter::serialize(&fm, &body_text).map_err(|e| format!("序列化失败：{e}"))?;
    atomic_write(&path, &content)?;
    ctx.touch_write(&id)?;
    ctx.mark_index_stale(&id)?;
    ctx.audit("create", &id, &format!("title={title}"));

    let mut out = json!({
        "created": true,
        "id": id,
        "title": title,
        "file": format!(".chain/nodes/{id}.md"),
        "hint": format!("节点已创建（AI 指南 v{}）。建立链接用 link_nodes（rel_type 仅 contains/solves/alternative）；更新内容用 update_node，建议先 read_node 取 updated 并传 expected_updated 防并发覆盖。完整规范见 get_guide", ctx.guide_version()),
    });
    if !suspected.is_empty() {
        let top = &suspected[0];
        let top_title = snap
            .nodes
            .iter()
            .find(|n| n.id == top.0)
            .map(|n| n.title.as_str())
            .unwrap_or("");
        out["duplicate_hint"] = json!(format!(
            "疑似重复：与 {}\u{300c}{}\u{300d} 嵌入余弦 {:.2} > 0.9（共 {} 个疑似）——已建 alternative 竞争边指向「{}」；确属同一记忆请考虑 update_node 合并，确认另建传 force=true",
            top.0, top_title, top.1, suspected.len(), top_title
        ));
    }
    Ok(out)
}

/// update_node(id, mode=append|replace_body, content, expected_updated?)
/// D3 乐观锁：expected_updated 与文件当前 updated 不符 → CONFLICT 不落盘；
/// v2.11 M8' ADR 0003 冲突即冻结：CONFLICT 后节点进入 [待裁决]（status=blocked + frozen 标记，
/// 仅元数据写入、绝不触碰冲突内容），冻结期间拒绝一切 MCP 写入，人工裁决后恢复
pub fn update_node(
    ctx: &Workspace,
    id: &str,
    mode: &str,
    content: &str,
    expected_updated: Option<&str>,
) -> Result<Value, String> {
    ctx.bump_clock()?;
    if !matches!(mode, "append" | "replace_body") {
        return Err(format!("mode 仅支持 append / replace_body，收到：{mode}"));
    }
    if !is_safe_id(id) {
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
    // 冻结门禁：人工裁决前拒绝写入
    ensure_not_frozen(&fm, id)?;

    // D3 乐观锁：先比对再动手，CONFLICT 不落盘（内容）；冲突即冻结（元数据，ADR 0003）
    if let Some(expected) = expected_updated {
        let current = fm_get_str(&fm, "updated").unwrap_or_default();
        if current != expected {
            let old_title = fm_get_str(&fm, "title").unwrap_or_else(|| id.to_string());
            let new_title = if old_title.starts_with("[待裁决]") {
                old_title
            } else {
                format!("[待裁决]{old_title}")
            };
            let fields = UpdateFields {
                title: Some(new_title),
                status: Some(crate::model::node::NodeStatus::Blocked),
                body: None,
                tags: None,
                evidence: None,
                parent: None,
                rel: None,
            };
            crate::model::node::apply_update(&mut fm, &fields)
                .map_err(|e| format!("应用冻结失败：{e}"))?;
            use serde_yaml::Value as YV;
            fm.insert(YV::String("frozen".into()), YV::Bool(true));
            fm.insert(
                YV::String("freeze_reason".into()),
                YV::String(format!(
                    "并发写冲突（expected={expected}，current={current}），待人工裁决"
                )),
            );
            let new_content = frontmatter::serialize(&fm, &body)
                .map_err(|e| format!("序列化失败：{e}"))?;
            atomic_write(&path, &new_content)?;
            let _ = ctx.record_conflict();
            let _ = ctx.touch_write(id);
            ctx.mark_index_stale(id)?;
            ctx.audit("freeze", id, "并发写冲突 → [待裁决]（status=blocked）");
            return Err(format!(
                "CONFLICT: 节点 {id} 的 updated 已变为 {current}（你期望 {expected}）——检测到并发写冲突，节点已冻结 [待裁决]（status=blocked，frozen=true），本次未落盘。请人工裁决：read_node 核对双方内容，恢复时编辑节点文件去除 frozen 标记并修正标题/状态"
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
    let new_content =
        frontmatter::serialize(&fm, &new_body).map_err(|e| format!("序列化失败：{e}"))?;
    atomic_write(&path, &new_content)?;
    ctx.touch_write(id)?;
    ctx.mark_index_stale(id)?;
    ctx.audit("update", id, &format!("mode={mode}"));

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
    ctx: &Workspace,
    from: &str,
    to: &str,
    rel_type: &str,
    desc: Option<&str>,
) -> Result<Value, String> {
    ctx.bump_clock()?;
    if !ctx.mode.is_dev() {
        return Err(
            "WORKSPACE_MODE_MISMATCH: 仅开发模式工作区可自由建链；分析模式的链由 AI 按协议维护"
                .into(),
        );
    }
    if !REL_TYPES.contains(&rel_type) {
        return Err(format!(
            "INVALID_REL: rel_type 非法「{rel_type}」，仅支持：{}（语义见 get_guide）",
            REL_TYPES.join(" / ")
        ));
    }
    if from == to {
        return Err("不允许自环（from 与 to 相同）".into());
    }
    if !is_safe_id(from) || !is_safe_id(to) {
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
    ensure_not_frozen(&fm, to)?; // 冻结门禁（ADR 0003）
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
            fm.remove(YV::String("rel_desc".into()));
            Value::Null
        }
        Some(d) => {
            fm.insert(
                YV::String("rel_desc".into()),
                YV::String(d.trim().to_string()),
            );
            json!(d.trim())
        }
        None => fm_get_str(&fm, "rel_desc")
            .map(|s| json!(s))
            .unwrap_or(Value::Null),
    };
    let new_content = frontmatter::serialize(&fm, &body).map_err(|e| format!("序列化失败：{e}"))?;
    atomic_write(&to_path, &new_content)?;
    ctx.touch_write(to)?;
    ctx.mark_index_stale(to)?;
    ctx.audit("link", to, &format!("from={from} rel={rel_type}"));

    Ok(json!({
        "linked": true,
        "from": from,
        "to": to,
        "rel": rel_type,
        "rel_desc": rel_desc_out,
        "hint": format!("链接已建立（rel 写入子节点 frontmatter 单值，AI 指南 v{}）。rel_type 仅 contains/solves/alternative，语义详见 get_guide", ctx.guide_version()),
    }))
}

/// recall：语义召回（记忆层 L2，契约 v2 新工具；框架 §5.4）
pub fn recall(
    ctx: &Workspace,
    query: &str,
    k: Option<usize>,
    include_archived: bool,
) -> Result<Value, String> {
    ctx.bump_clock()?;
    crate::retrieval::recall(ctx, query, k, include_archived)
}

/// consolidate：蒸馏（框架 T9/§5.5/§5.6，契约 v4 第 13 工具；开发模式为主、分析模式共享）。
/// - dry_run 默认 true（先看计划再执行）；BFS 连通分量聚类（size ≥ 2），targets 过滤，k = 簇数上限
/// - 产物：骨架节点（derived:true + 标题 [蒸馏] + 正文逐条来源引用），检索默认降权 ×0.85
/// - 人审摘帽 = 删除 derived 标记后转普通节点；无可蒸馏簇 → CONSOLIDATE_EMPTY:
pub fn consolidate(
    ctx: &Workspace,
    targets: Option<Vec<String>>,
    dry_run: Option<bool>,
    k: Option<usize>,
) -> Result<Value, String> {
    ctx.bump_clock()?;
    let snap = ctx.scan()?;
    let k = k.unwrap_or(8).clamp(1, 100);
    let plan = crate::consolidate::build_plan(&snap, targets.as_deref(), k);
    if plan.clusters.is_empty() {
        return Err(
            "CONSOLIDATE_EMPTY: 无可蒸馏簇（需 ≥2 个活跃节点组成的连通分量；targets 过滤后成员 <2 也算空）——先用 link_nodes 建链，或检查 targets 是否命中同簇节点"
                .into(),
        );
    }
    let dry_run = dry_run.unwrap_or(true);
    let plan_json: Vec<Value> = plan
        .clusters
        .iter()
        .enumerate()
        .map(|(i, c)| {
            json!({
                "cluster_id": format!("cluster-{}", i + 1),
                "members": c.members,
                "sources": c.sources,
                "summary_preview": frontmatter::truncate_utf8(&c.summary, 200),
            })
        })
        .collect();

    let mut created: Vec<Value> = Vec::new();
    if !dry_run {
        for c in &plan.clusters {
            let id = auto_id(&ctx.nodes_dir());
            let path = ctx.node_path(&id);
            if path.exists() {
                return Err(format!("节点 {id} 已存在"));
            }
            let title = format!("[蒸馏]{}", c.title);
            use serde_yaml::Value as YV;
            let now = frontmatter::now_iso8601();
            let mut fm = serde_yaml::Mapping::new();
            fm.insert(YV::String("id".into()), YV::String(id.clone()));
            fm.insert(YV::String("type".into()), YV::String("note".into()));
            fm.insert(YV::String("title".into()), YV::String(title.clone()));
            fm.insert(YV::String("parent".into()), YV::Null);
            fm.insert(YV::String("status".into()), YV::String("none".into()));
            fm.insert(YV::String("created".into()), YV::String(now.clone()));
            fm.insert(YV::String("updated".into()), YV::String(now));
            fm.insert(YV::String("revision".into()), YV::Number(1u64.into()));
            fm.insert(YV::String("tags".into()), YV::Sequence(Vec::new()));
            fm.insert(YV::String("derived".into()), YV::Bool(true));
            let content = frontmatter::serialize(&fm, &c.summary)
                .map_err(|e| format!("序列化失败：{e}"))?;
            atomic_write(&path, &content)?;
            ctx.touch_write(&id)?;
            ctx.mark_index_stale(&id)?;
            ctx.audit("consolidate", &id, &format!("members={}", c.members.join(",")));
            created.push(json!({ "id": id, "title": title, "derived": true }));
        }
    }

    let hint = if dry_run {
        format!(
            "蒸馏计划（不落盘，AI 指南 v{}）：{} 个簇。确认后传 dry_run=false 执行——产物为 [蒸馏] 骨架节点（derived:true，逐条来源引用，检索默认降权 ×0.85）；人审摘帽 = 删除 derived 标记。",
            ctx.guide_version(),
            plan_json.len()
        )
    } else {
        format!(
            "已创建 {} 个 [蒸馏] 骨架节点（derived:true，默认降权）。原节点不删；人审摘帽 = 删除 derived 标记后转普通节点（AI 指南 v{}）。",
            created.len(),
            ctx.guide_version()
        )
    };
    let mut out = json!({
        "dry_run": dry_run,
        "plan": plan_json,
        "hint": hint,
    });
    if !dry_run {
        out["created"] = json!(created);
    }
    Ok(out)
}

/// archive_node：归档节点（框架 T6 / §5.5，契约 v3 新工具；开发模式为主）。
/// - `archived: true` + 标题前缀 `[归档]`（幂等：已带前缀不重复加）+ 可选 archived_reason
/// - 文件原子标记后移入 `.chain/archive/<id>.md`（同盘 rename；窗口期残留 flagged 文件
///   由扫描器按归档处理，绝无半截状态）
/// - 索引条目标 stale（下次 recall 按需重嵌并带上 archived 标记）；stats 记写触达
/// - 90 天未触达为建议阈值：仅提示，不自动执行
pub fn archive_node(
    ctx: &Workspace,
    id: &str,
    reason: Option<&str>,
) -> Result<Value, String> {
    ctx.bump_clock()?;
    if !ctx.mode.is_dev() {
        return Err(
            "WORKSPACE_MODE_MISMATCH: 仅开发模式工作区可归档节点（知识库维护工具）；分析模式的链由 AI 按协议维护"
                .into(),
        );
    }
    if !is_safe_id(id) {
        return Err("节点 id 非法".into());
    }
    let src = ctx.node_path(id);
    let dst = ctx.archive_path(id);
    if dst.exists() {
        return Err(format!(
            "节点 {id} 已归档（.chain/archive/{id}.md）——如需恢复请手动移回 nodes/ 并去掉 archived 标记"
        ));
    }
    if !src.exists() {
        return Err(format!("节点 {id} 不存在"));
    }

    let raw = std::fs::read_to_string(&src).map_err(|e| format!("读取失败：{e}"))?;
    let (mut fm, body) = parse_lenient(&raw, id)?;
    ensure_not_frozen(&fm, id)?; // 冻结门禁（ADR 0003）
    let old_title = fm_get_str(&fm, "title").unwrap_or_else(|| id.to_string());
    let new_title = if old_title.starts_with("[归档]") {
        old_title
    } else {
        format!("[归档]{old_title}")
    };
    // 仅改 title（revision+1、updated 刷新），archived 字段直接写入 fm
    let fields = UpdateFields {
        title: Some(new_title.clone()),
        status: None,
        body: None,
        tags: None,
        evidence: None,
        parent: None,
        rel: None,
    };
    crate::model::node::apply_update(&mut fm, &fields).map_err(|e| format!("应用更新失败：{e}"))?;
    use serde_yaml::Value as YV;
    fm.insert(YV::String("archived".into()), YV::Bool(true));
    if let Some(r) = reason.map(|s| s.trim()).filter(|s| !s.is_empty()) {
        fm.insert(
            YV::String("archived_reason".into()),
            YV::String(r.to_string()),
        );
    }
    let new_content =
        frontmatter::serialize(&fm, &body).map_err(|e| format!("序列化失败：{e}"))?;
    // 原子窗口：先原地原子写（文件已 flagged → 扫描器即按归档处理），再同盘 rename 进 archive/
    atomic_write(&src, &new_content)?;
    std::fs::create_dir_all(ctx.archive_dir()).map_err(|e| format!("创建归档目录失败：{e}"))?;
    std::fs::rename(&src, &dst).map_err(|e| {
        // rename 失败（如 archive 被占用）：文件仍处 nodes/ 且已 flagged，语义一致不丢数据
        format!("移动归档失败：{e}")
    })?;
    ctx.touch_write(id)?;
    ctx.mark_index_stale(id)?;
    ctx.audit("archive", id, "移入 .chain/archive/");

    Ok(json!({
        "archived": true,
        "id": id,
        "title": new_title,
        "archived_to": format!(".chain/archive/{id}.md"),
        "reason": reason.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
        "hint": format!(
            "节点已归档（AI 指南 v{}）。归档节点全文保留、默认不进图与检索，recall 传 include_archived=true 可自 L4 起找回；read_node 仍可直读。90 天未触达为建议归档阈值（仅提示，不自动执行）。",
            ctx.guide_version()
        ),
    }))
}

/// unlink_nodes：断开 from(父)→to(子) 链接（框架 T6 补断边 / §5.5，契约 v3 新工具；
/// 开发模式为主）。边 = 子节点 frontmatter 的 parent 字段：断边即 parent 置 null，
/// 并清理 rel / rel_desc（返回值 rel_removed 供回溯）。
pub fn unlink_nodes(ctx: &Workspace, from: &str, to: &str) -> Result<Value, String> {
    ctx.bump_clock()?;
    if !ctx.mode.is_dev() {
        return Err(
            "WORKSPACE_MODE_MISMATCH: 仅开发模式工作区可自由断边（知识库维护工具）；分析模式的链由 AI 按协议维护"
                .into(),
        );
    }
    if !is_safe_id(from) || !is_safe_id(to) {
        return Err("节点 id 非法".into());
    }
    if from == to {
        return Err("from 与 to 不能相同".into());
    }
    if !ctx.node_path(from).exists() {
        return Err(format!("父节点 {from} 不存在"));
    }
    if !ctx.node_path(to).exists() {
        if ctx.archive_path(to).exists() {
            return Err(format!("子节点 {to} 已归档，无需断边"));
        }
        return Err(format!("子节点 {to} 不存在"));
    }

    let to_path = ctx.node_path(to);
    let raw = std::fs::read_to_string(&to_path).map_err(|e| format!("读取失败：{e}"))?;
    let (mut fm, body) = parse_lenient(&raw, to)?;
    ensure_not_frozen(&fm, to)?; // 冻结门禁（ADR 0003）
    let cur_parent = fm_get_str(&fm, "parent");
    if cur_parent.as_deref() != Some(from) {
        return Err(format!(
            "边不存在：{to} 的父节点不是 {from}（当前：{}）",
            cur_parent.as_deref().unwrap_or("null")
        ));
    }
    let rel_removed = fm_get_str(&fm, "rel").unwrap_or_else(|| "contains".to_string());
    let fields = UpdateFields {
        title: None,
        status: None,
        body: None,
        tags: None,
        evidence: None,
        parent: Some(None),
        rel: None,
    };
    crate::model::node::apply_update(&mut fm, &fields).map_err(|e| format!("应用更新失败：{e}"))?;
    // 断边后关系字段一并清理（rel/rel_desc 无 parent 即无意义）
    use serde_yaml::Value as YV;
    fm.remove(YV::String("rel".into()));
    fm.remove(YV::String("rel_desc".into()));
    let new_content = frontmatter::serialize(&fm, &body).map_err(|e| format!("序列化失败：{e}"))?;
    atomic_write(&to_path, &new_content)?;
    ctx.touch_write(to)?;
    ctx.mark_index_stale(to)?;
    ctx.audit("unlink", to, &format!("from={from} rel_removed={rel_removed}"));

    Ok(json!({
        "unlinked": true,
        "from": from,
        "to": to,
        "rel_removed": rel_removed,
        "hint": format!("边已断开（{to} 的 parent 置 null，rel/rel_desc 已清理，AI 指南 v{}）。需要恢复时用 link_nodes 重建（rel_type 仅 contains/solves/alternative）。", ctx.guide_version()),
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

    fn ctx_of(tmp: &TempDir) -> Workspace {
        Workspace::open(tmp.path().to_path_buf()).unwrap()
    }

    fn write_node(tmp: &TempDir, id: &str, title: &str, parent: &str, rel: &str) {
        let content = format!(
            "---\nid: {id}\ntype: note\ntitle: {title}\nparent: {parent}\nrel: {rel}\nstatus: none\ncreated: 2026-09-01T10:00:00+08:00\nupdated: 2026-09-01T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# {title}\n"
        );
        fs::write(
            tmp.path()
                .join(".chain")
                .join("nodes")
                .join(format!("{id}.md")),
            content,
        )
        .unwrap();
    }

    #[test]
    fn ctx_open_rejects_untagged() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".chain").join("nodes")).unwrap();
        assert!(
            Workspace::open(tmp.path().to_path_buf()).is_err(),
            "无 .mode 标签应拒绝"
        );
    }

    #[test]
    fn create_and_read_node() {
        let tmp = setup("dev");
        let ctx = ctx_of(&tmp);
        let r = create_node(
            &ctx,
            "Rust 笔记",
            Some("所有权与借用"),
            Some(vec!["rust".into()]),
            None,
        )
        .unwrap();
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
        let err = update_node(
            &ctx,
            "node-1",
            "replace_body",
            "恶意覆盖",
            Some("2000-01-01T00:00:00+08:00"),
        )
        .unwrap_err();
        assert!(err.starts_with("CONFLICT"), "应报 CONFLICT：{err}");
        let v2 = read_node(&ctx, "node-1", None).unwrap();
        assert_eq!(
            v2["body"].as_str().unwrap(),
            "原文\n\n安全追加",
            "CONFLICT 不得落盘"
        );

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
        assert!(
            update_node(&ctx2, "g-001", "replace_body", "  ", None).is_err(),
            "分析模式空 body 应拒绝"
        );
    }

    #[test]
    fn link_nodes_full_flow() {
        let tmp = setup("dev");
        let ctx = ctx_of(&tmp);
        create_node(&ctx, "父", None, None, None).unwrap();
        create_node(&ctx, "子", None, None, None).unwrap();
        let r = link_nodes(
            &ctx,
            "node-1",
            "node-2",
            "solves",
            Some("子节点补父节点短板"),
        )
        .unwrap();
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
        assert!(
            link_nodes(&ctx, "node-1", "node-2", "depends_on", None).is_err(),
            "D2 词表外应拒绝"
        );
        assert!(link_nodes(&ctx, "node-1", "ghost", "contains", None).is_err());
        assert!(
            link_nodes(&ctx, "node-1", "node-1", "contains", None).is_err(),
            "自环应拒绝"
        );
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
        assert!(
            narrative.contains("--contains-->"),
            "叙述应含正向边：{narrative}"
        );
        assert!(
            narrative.contains("--solves-->"),
            "叙述应含 solves：{narrative}"
        );

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
        assert_eq!(o["guide_version"], crate::guide::AI_GUIDE_DEV_VERSION);

        let g = get_guide(&ctx).unwrap();
        assert_eq!(g["version"], crate::guide::AI_GUIDE_DEV_VERSION);
        assert!(g["content"].as_str().unwrap().contains("知识库搭建"));
    }

    // ── v2.10 M7'：归档与断边 ──

    #[test]
    fn archive_unlink_full_flow() {
        let tmp = setup("dev");
        let ctx = ctx_of(&tmp);
        create_node(&ctx, "父", None, None, None).unwrap();
        create_node(&ctx, "子", None, None, None).unwrap();
        link_nodes(&ctx, "node-1", "node-2", "solves", Some("补短板")).unwrap();

        // 断边：edge 消失、rel 清理、返回 rel_removed
        let u = unlink_nodes(&ctx, "node-1", "node-2").unwrap();
        assert_eq!(u["unlinked"], true);
        assert_eq!(u["rel_removed"], "solves");
        let snap = ctx.scan().unwrap();
        assert_eq!(snap.edges.len(), 0);
        let child = read_node(&ctx, "node-2", None).unwrap();
        assert!(child["parent"].is_null(), "断边后 parent 应为 null");
        assert!(
            child["rel"].is_null(),
            "断边后 rel 应为 null（Node 序列化对缺省 rel 输出 null）"
        );

        // 重建边再归档 node-2
        link_nodes(&ctx, "node-1", "node-2", "contains", None).unwrap();
        let a = archive_node(&ctx, "node-2", Some("内容过时")).unwrap();
        assert_eq!(a["archived"], true);
        assert_eq!(a["title"], "[归档]子");
        assert_eq!(a["archived_to"], ".chain/archive/node-2.md");
        assert_eq!(a["reason"], "内容过时");
        // 文件已移入 archive/
        assert!(!tmp.path().join(".chain/nodes/node-2.md").exists());
        let raw = fs::read_to_string(tmp.path().join(".chain/archive/node-2.md")).unwrap();
        assert!(raw.contains("archived: true"), "frontmatter 应落 archived: true");
        assert!(raw.contains("archived_reason: 内容过时"));
        assert!(raw.contains("title: '[归档]子'"));

        // 活跃图只余 node-1；归档列表含 node-2；边消失
        let snap = ctx.scan().unwrap();
        assert_eq!(snap.nodes.len(), 1);
        assert_eq!(snap.archived.len(), 1);
        assert_eq!(snap.archived[0].id, "node-2");
        assert_eq!(snap.edges.len(), 0, "归档后边自动退出活跃图");

        // read_node 仍可直读归档节点
        let v = read_node(&ctx, "node-2", None).unwrap();
        assert_eq!(v["archived"], true);
        assert_eq!(v["title"], "[归档]子");

        // search 默认不含归档节点
        let s = search(&ctx, "子", None).unwrap();
        assert_eq!(s["total"], 0, "search 只搜活跃节点（归档自 L4 起可见）");

        // 再次归档 → 报已归档
        let err = archive_node(&ctx, "node-2", None).unwrap_err();
        assert!(err.contains("已归档"), "{err}");
        // 断边：归档子节点 → 提示无需断边
        let err = unlink_nodes(&ctx, "node-1", "node-2").unwrap_err();
        assert!(err.contains("已归档"), "{err}");
    }

    #[test]
    fn archive_prefix_idempotent_and_stats_touched() {
        let tmp = setup("dev");
        let ctx = ctx_of(&tmp);
        create_node(&ctx, "旧记忆", None, None, None).unwrap();
        archive_node(&ctx, "node-1", None).unwrap();
        // 手工归档文件（带前缀）再走一次 archive_node 的标题逻辑应不重复加前缀——
        // 已归档节点直接报错，前缀幂等由单次调用的 starts_with 检查覆盖（旧标题含前缀不重复加）
        let raw = fs::read_to_string(tmp.path().join(".chain/archive/node-1.md")).unwrap();
        assert_eq!(raw.matches("[归档]").count(), 1, "标题前缀只加一次");
        // stats：归档是写触达（writes=1 且有触达时间戳）
        let raw_stats = fs::read_to_string(tmp.path().join(".chain/stats.json")).unwrap();
        let d: crate::stats::StatsData = serde_json::from_str(&raw_stats).unwrap();
        let e = d.per_id.get("node-1").expect("node-1 应有 per_id 记录");
        assert_eq!(e.writes, 2, "create + archive 两次写触达");
        assert_eq!(e.touches.len(), 2, "写触达计入强度窗口");
    }

    #[test]
    fn archive_unlink_errors() {
        let tmp = setup("dev");
        let ctx = ctx_of(&tmp);
        create_node(&ctx, "A", None, None, None).unwrap();
        create_node(&ctx, "B", None, None, None).unwrap();
        // 不存在的节点
        assert!(archive_node(&ctx, "ghost", None).is_err());
        // 边不存在
        let err = unlink_nodes(&ctx, "node-1", "node-2").unwrap_err();
        assert!(err.contains("边不存在"), "{err}");
        // 自环/同 id
        assert!(unlink_nodes(&ctx, "node-1", "node-1").is_err());
        // 断掉一条已断的边
        link_nodes(&ctx, "node-1", "node-2", "contains", None).unwrap();
        unlink_nodes(&ctx, "node-1", "node-2").unwrap();
        assert!(unlink_nodes(&ctx, "node-1", "node-2").is_err());

        // 分析模式：WORKSPACE_MODE_MISMATCH 前缀（错误码契约 §7）
        let tmp2 = setup("analysis");
        let ctx2 = ctx_of(&tmp2);
        let err = archive_node(&ctx2, "a", None).unwrap_err();
        assert!(err.starts_with("WORKSPACE_MODE_MISMATCH"), "{err}");
        let err = unlink_nodes(&ctx2, "a", "b").unwrap_err();
        assert!(err.starts_with("WORKSPACE_MODE_MISMATCH"), "{err}");
        // link_nodes 词表外 → INVALID_REL 前缀（错误码契约 §7）
        let err = link_nodes(&ctx, "node-1", "node-2", "bogus", None).unwrap_err();
        assert!(err.starts_with("INVALID_REL"), "{err}");
    }

    #[test]
    fn read_tools_touch_stats() {
        let tmp = setup("dev");
        write_node(&tmp, "a", "根节点", "null", "contains");
        write_node(&tmp, "b", "中间节点", "a", "contains");
        write_node(&tmp, "c", "叶子节点", "b", "solves");
        let ctx = ctx_of(&tmp);

        read_node(&ctx, "a", None).unwrap();
        search(&ctx, "叶", None).unwrap();
        expand(&ctx, "a", Some(1)).unwrap();
        read_path(&ctx, "a", "c").unwrap();
        // 四个读工具 → per_id 触达回写（a: read+search? search 命中 c；expand 中心 a；path 端点 a/c）
        let raw = fs::read_to_string(tmp.path().join(".chain/stats.json")).unwrap();
        let d: crate::stats::StatsData = serde_json::from_str(&raw).unwrap();
        assert!(d.per_id.get("a").map(|e| e.reads).unwrap_or(0) >= 3, "a 被 read/expand/path 触达");
        assert!(d.per_id.get("c").map(|e| e.reads).unwrap_or(0) >= 2, "c 被 search/path 触达");
    }

    // ── v2.11 M8'：冲突冻结 / 重复检测 / 蒸馏 / 审计 ──

    #[test]
    fn conflict_freezes_node_and_blocks_writes() {
        let tmp = setup("dev");
        let ctx = ctx_of(&tmp);
        create_node(&ctx, "并发目标", Some("原文"), None, None).unwrap();
        let v = read_node(&ctx, "node-1", None).unwrap();
        let updated = v["updated"].as_str().unwrap().to_string();

        // 正确 expected → 通过（不冻结）
        update_node(&ctx, "node-1", "append", "安全追加", Some(&updated)).unwrap();

        // 过期 expected → CONFLICT + 冲突即冻结（ADR 0003：绝不静默覆盖）
        let err = update_node(
            &ctx,
            "node-1",
            "replace_body",
            "恶意覆盖",
            Some("2000-01-01T00:00:00+08:00"),
        )
        .unwrap_err();
        assert!(err.starts_with("CONFLICT"), "{err}");
        assert!(err.contains("[待裁决]"), "错误应说明已冻结：{err}");

        // 冻结态：title 前缀 + status blocked + frozen 标记；正文未被触碰
        let v = read_node(&ctx, "node-1", None).unwrap();
        assert_eq!(v["title"], "[待裁决]并发目标");
        assert_eq!(v["status"], "blocked");
        assert_eq!(v["frozen"], true);
        assert_eq!(
            v["body"].as_str().unwrap(),
            "原文\n\n安全追加",
            "冲突内容绝不落盘"
        );
        let raw = fs::read_to_string(tmp.path().join(".chain/nodes/node-1.md")).unwrap();
        assert!(raw.contains("frozen: true"));
        assert!(raw.contains("freeze_reason:"));

        // 冻结期间拒绝一切写入
        let err = update_node(&ctx, "node-1", "append", "x", None).unwrap_err();
        assert!(err.contains("已冻结"), "{err}");
        create_node(&ctx, "另一个", None, None, None).unwrap(); // node-2
        let err = link_nodes(&ctx, "node-2", "node-1", "contains", None).unwrap_err();
        assert!(err.contains("已冻结"), "{err}");
        let err = archive_node(&ctx, "node-1", None).unwrap_err();
        assert!(err.contains("已冻结"), "{err}");
        let err = unlink_nodes(&ctx, "node-1", "node-1").unwrap_err(); // 自环错误优先，不掩盖
        assert!(err.contains("不能相同"), "{err}");

        // stats：CONFLICT 计数（框架 §6 指标采集点）
        let raw_stats = fs::read_to_string(tmp.path().join(".chain/stats.json")).unwrap();
        let d: crate::stats::StatsData = serde_json::from_str(&raw_stats).unwrap();
        assert_eq!(d.calibrate.conflicts, 1, "冲突计数应 +1");

        // 人工裁决：编辑文件去除 frozen 并修正标题/状态 → 恢复可写（模拟人工/GUI 侧修复）
        let raw = fs::read_to_string(tmp.path().join(".chain/nodes/node-1.md")).unwrap();
        let (mut fm, body) = parse_lenient(&raw, "node-1").unwrap();
        use serde_yaml::Value as YV;
        fm.insert(YV::String("frozen".into()), YV::Bool(false));
        fm.remove(YV::String("freeze_reason".into()));
        fm.insert(YV::String("title".into()), YV::String("并发目标".into()));
        fm.insert(YV::String("status".into()), YV::String("none".into()));
        let fixed = frontmatter::serialize(&fm, &body).unwrap();
        atomic_write(&tmp.path().join(".chain/nodes/node-1.md"), &fixed).unwrap();
        update_node(&ctx, "node-1", "append", "恢复后追加", None).unwrap();
        let v = read_node(&ctx, "node-1", None).unwrap();
        // frozen=false 时字段不出现在 JSON（skip_serializing_if 零破坏语义）
        assert!(v.get("frozen").is_none(), "解冻后 frozen 字段应消失：{v}");
    }

    #[test]
    fn duplicate_detection_stage2_stub() {
        let tmp = setup("dev");
        let ctx = ctx_of(&tmp);
        create_node(&ctx, "贝叶斯推理", Some("贝叶斯定理与后验更新"), None, None).unwrap();
        // 阶段一：标题包含关系命中候选；阶段二：stub 同向向量 → 余弦 1.0 > 0.9 → 疑似重复
        struct Stub;
        impl crate::embed::Embedder for Stub {
            fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, crate::embed::EmbedError> {
                Ok(texts.iter().map(|_| vec![1.0f32, 0.0]).collect())
            }
            fn dim(&self) -> usize {
                2
            }
        }
        let r = create_node_impl(
            &ctx,
            "贝叶斯推理笔记",
            Some("补充内容"),
            None,
            None,
            Some(&Stub),
        )
        .unwrap();
        assert_eq!(r["created"], true);
        let hint = r["duplicate_hint"].as_str().expect("应含疑似重复提示");
        assert!(hint.contains("疑似重复"), "{hint}");
        assert!(hint.contains("node-1"), "{hint}");
        // alternative 竞争边已建
        let snap = ctx.scan().unwrap();
        assert_eq!(snap.edges.len(), 1);
        assert_eq!(snap.edges[0].parent, "node-1");
        assert_eq!(snap.edges[0].rel, "alternative");
        let child = read_node(&ctx, "node-2", None).unwrap();
        assert_eq!(child["parent"], "node-1");
        assert_eq!(child["rel"], "alternative");

        // force=true → 跳过检测：无 hint、无边
        let r2 = create_node_impl(
            &ctx,
            "贝叶斯推理详解",
            None,
            None,
            Some(true),
            Some(&Stub),
        )
        .unwrap();
        assert!(r2.get("duplicate_hint").is_none(), "force 应跳过检测");
        let child = read_node(&ctx, "node-3", None).unwrap();
        assert!(child["parent"].is_null(), "force 不应建竞争边");
    }

    #[test]
    fn duplicate_detection_no_candidate_no_hint() {
        // 无包含关系候选 → 不加载模型、无 hint（golden 确定性锚点）
        let tmp = setup("dev");
        let ctx = ctx_of(&tmp);
        create_node(&ctx, "Golden A", None, None, None).unwrap();
        let r = create_node(&ctx, "Golden B", None, None, None).unwrap();
        assert!(
            r.get("duplicate_hint").is_none(),
            "无候选不得产生疑似提示：{r}"
        );
        assert!(r.get("parent").is_none(), "创建响应无 parent 字段");
        let child = read_node(&ctx, "node-2", None).unwrap();
        assert!(child["parent"].is_null());
    }

    #[test]
    fn consolidate_plan_and_run_flow() {
        let tmp = setup("dev");
        let ctx = ctx_of(&tmp);
        // 标题无包含关系 → 不触发重复检测阶段二（单测零模型依赖）
        create_node(&ctx, "力导向布局", Some("斥力引力模型"), None, None).unwrap();
        create_node(&ctx, "稳定化调优", Some("布局稳定化参数"), None, None).unwrap();
        link_nodes(&ctx, "node-1", "node-2", "solves", None).unwrap();

        // dry_run（默认 true）→ 计划不落盘
        let p = consolidate(&ctx, None, None, None).unwrap();
        assert_eq!(p["dry_run"], true);
        assert_eq!(p["plan"][0]["cluster_id"], "cluster-1");
        assert_eq!(p["plan"][0]["members"][0], "node-1");
        assert_eq!(p["plan"][0]["members"][1], "node-2");
        assert!(p["plan"][0]["summary_preview"]
            .as_str()
            .unwrap()
            .contains("来源：node-1"), "{}", p["plan"][0]["summary_preview"]);
        assert!(p.get("created").is_none(), "dry_run 不返回 created");
        assert_eq!(ctx.scan().unwrap().nodes.len(), 2, "dry_run 不落盘");

        // dry_run=false → 创建骨架节点
        let r = consolidate(&ctx, None, Some(false), None).unwrap();
        assert_eq!(r["dry_run"], false);
        assert_eq!(r["created"][0]["id"], "node-3");
        assert_eq!(r["created"][0]["derived"], true);
        let t = r["created"][0]["title"].as_str().unwrap();
        assert!(t.starts_with("[蒸馏]"), "骨架标题带前缀：{t}");

        let v = read_node(&ctx, "node-3", None).unwrap();
        assert_eq!(v["derived"], true);
        let body = v["body"].as_str().unwrap();
        assert!(body.contains("来源：node-1「力导向布局」"), "{body}");
        assert!(body.contains("斥力引力模型"), "骨架含成员一句话：{body}");
        let raw = fs::read_to_string(tmp.path().join(".chain/nodes/node-3.md")).unwrap();
        assert!(raw.contains("derived: true"), "frontmatter 落 derived 标记");

        // 原节点不删
        let snap = ctx.scan().unwrap();
        assert_eq!(snap.nodes.len(), 3);
    }

    #[test]
    fn consolidate_empty_and_targets_filter() {
        let tmp = setup("dev");
        let ctx = ctx_of(&tmp);
        create_node(&ctx, "孤岛A", None, None, None).unwrap();
        create_node(&ctx, "孤岛B", None, None, None).unwrap();
        // 无边 → 无连通分量 ≥2 → CONSOLIDATE_EMPTY
        let err = consolidate(&ctx, None, None, None).unwrap_err();
        assert!(err.starts_with("CONSOLIDATE_EMPTY"), "{err}");

        // 建链后 targets 过滤：只取 1 个 → 空；两个 → 1 簇
        link_nodes(&ctx, "node-1", "node-2", "contains", None).unwrap();
        let err = consolidate(&ctx, Some(vec!["node-1".into()]), None, None).unwrap_err();
        assert!(err.starts_with("CONSOLIDATE_EMPTY"), "{err}");
        let p = consolidate(&ctx, Some(vec!["node-1".into(), "node-2".into()]), None, None).unwrap();
        assert_eq!(p["plan"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn audit_entries_appended_for_write_actions() {
        let tmp = setup("dev");
        let ctx = ctx_of(&tmp);
        create_node(&ctx, "审计节点", None, None, None).unwrap();
        update_node(&ctx, "node-1", "append", "追加", None).unwrap();
        create_node(&ctx, "子节点", None, None, None).unwrap();
        link_nodes(&ctx, "node-1", "node-2", "contains", None).unwrap();
        archive_node(&ctx, "node-2", None).unwrap();
        unlink_nodes(&ctx, "node-1", "node-2").unwrap_err(); // 已归档，无 audit
        let rows = crate::audit::read_all(tmp.path()).unwrap();
        let actions: Vec<String> = rows.iter().map(|r| r["action"].as_str().unwrap().to_string()).collect();
        assert!(actions.contains(&"create".into()), "{actions:?}");
        assert!(actions.contains(&"update".into()));
        assert!(actions.contains(&"link".into()));
        assert!(actions.contains(&"archive".into()));
        assert_eq!(
            actions.iter().filter(|a| *a == "create").count(),
            2,
            "两次 create 两条审计"
        );
    }
}
