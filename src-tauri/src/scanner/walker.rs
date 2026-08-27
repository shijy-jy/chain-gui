use std::path::Path;
use anyhow::Result;
use walkdir::WalkDir;
use crate::model::ScanMode;
use crate::scanner::frontmatter::{parse, now_iso8601};
use crate::scanner::validator;
use crate::model::node::Node;
use crate::model::chain::{ChainSnapshot, Manifest, Edge, ChainHealth, ProjectPersona};
use crate::model::validation::ValidationReport;

/// 分析模式扫描（严格 chain 协议）——兼容旧调用方的薄封装
pub fn scan_chain_dir(root: &Path) -> Result<ChainSnapshot> {
    scan_chain_dir_mode(root, ScanMode::Analysis)
}

/// 双模式扫描：Analysis 走严格协议校验；Dev 走自由知识图谱规则（v2.0）
pub fn scan_chain_dir_mode(root: &Path, mode: ScanMode) -> Result<ChainSnapshot> {
    let chain_dir = root.join(".chain");
    let nodes_dir = chain_dir.join("nodes");

    if !chain_dir.exists() {
        anyhow::bail!("目录 {:?} 下不存在 .chain/ 子目录", root);
    }

    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    // 记录 (filename, &Node) 用于结构级校验
    let mut nodes_with_files: Vec<(String, Node)> = Vec::new();

    for entry in WalkDir::new(&nodes_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        let filename = path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                errors.push(format!("[{}] 读取失败: {}", filename, e));
                continue;
            }
        };

        if mode.is_dev() {
            // 开发模式：任何 .md 都是节点，字段全部宽松，无内容要求
            let node = build_dev_node(&filename, &content);
            nodes_with_files.push((filename.clone(), node.clone()));
            nodes.push(node);
            continue;
        }

        // 解析 frontmatter → Mapping + body
        let (fm, body) = match parse(&content) {
            Ok(result) => result,
            Err(e) => {
                errors.push(format!("[{}] frontmatter: {}", filename, e));
                continue;
            }
        };

        // 字段级校验（在 Mapping 上做，能精确报字段错误）
        validator::validate_fields(&filename, &fm, &mut errors);

        // 尝试反序列化为 Node
        let yaml_str = serde_yaml::to_string(&fm).unwrap_or_default();
        match serde_yaml::from_str::<Node>(&yaml_str) {
            Ok(mut node) => {
                node.body = body;
                if let Some(parent_id) = &node.parent {
                    edges.push(Edge {
                        parent: parent_id.clone(),
                        child: node.id.clone(),
                    });
                }
                nodes_with_files.push((filename.clone(), node.clone()));
                nodes.push(node);
            }
            Err(e) => {
                errors.push(format!("[{}] 反序列化失败: {}", filename, e));
            }
        }
    }

    if mode.is_dev() {
        // 开发模式：边两遍构建——parent 指向不存在的节点时静默跳过（等目标出现后边自动出现），
        // 允许多根、孤立节点、环，不做结构校验，不检查 AI_GUIDE
        let ids: std::collections::HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
        for node in &nodes {
            if let Some(parent_id) = &node.parent {
                if ids.contains(parent_id.as_str()) {
                    edges.push(Edge {
                        parent: parent_id.clone(),
                        child: node.id.clone(),
                    });
                }
            }
        }
    } else {
        // 结构级校验
        let refs: Vec<(&str, &Node)> = nodes_with_files
            .iter()
            .map(|(f, n)| (f.as_str(), n))
            .collect();
        validator::validate_structure(&refs, &mut errors, &mut warnings);

        // v1.2：检测 .chain/AI_GUIDE.md 是否陈旧（无版本标记或版本低于内嵌指南）
        check_guide_staleness(&chain_dir, &mut warnings);
    }

    let valid = errors.is_empty();
    let chain_health = build_chain_health(&nodes);
    let active_chain = build_active_chain(&nodes, mode);
    let project_persona = build_project_persona(&nodes, &chain_dir);

    let manifest = Manifest {
        root: root.to_path_buf(),
        node_count: nodes.len(),
        edge_count: edges.len(),
        generated_at: now_iso8601(),
        active_chain,
        chain_health,
        project_persona,
    };

    Ok(ChainSnapshot {
        nodes,
        edges,
        manifest,
        validation: ValidationReport { valid, errors, warnings },
    })
}

/// 开发模式节点构造：frontmatter 可有可无、字段可缺省，全部有兜底默认值。
/// id 一律取文件名（保证 update/delete/链接操作按文件定位），其余字段宽松。
fn build_dev_node(filename: &str, content: &str) -> Node {
    use serde_yaml::Value as YamlValue;
    use crate::model::node::{NodeStatus, NodeType};

    let stem = filename.trim_end_matches(".md");
    let (fm, body) = match parse(content) {
        Ok(result) => result,
        Err(_) => {
            // 无 frontmatter：整个文件即正文
            return Node {
                id: stem.to_string(),
                node_type: NodeType::Task,
                title: first_title_or(stem, content),
                parent: None,
                status: NodeStatus::Pending,
                created: now_iso8601(),
                updated: now_iso8601(),
                revision: 1,
                tags: Vec::new(),
                evidence: Vec::new(),
                body: content.to_string(),
                folded: None,
            };
        }
    };

    let get = |k: &str| fm.get(&YamlValue::String(k.into())).cloned();
    let type_val = get("type")
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .map(|t| match t.as_str() {
            "goal" => NodeType::Goal,
            "design" => NodeType::Design,
            "verification" => NodeType::Verification,
            _ => NodeType::Task,
        })
        .unwrap_or(NodeType::Task);
    let status = get("status")
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .map(|s| match s.as_str() {
            "in_progress" => NodeStatus::InProgress,
            "success" => NodeStatus::Success,
            "failed" => NodeStatus::Failed,
            "blocked" => NodeStatus::Blocked,
            _ => NodeStatus::Pending,
        })
        .unwrap_or(NodeStatus::Pending);
    let title = get("title")
        .and_then(|v| v.as_str().map(|s| s.trim().to_string()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| first_title_or(stem, &body));
    let parent = match get("parent") {
        Some(YamlValue::String(p)) if !p.trim().is_empty() => Some(p),
        _ => None,
    };
    let created = get("created").and_then(|v| v.as_str().map(|s| s.to_string())).unwrap_or_else(now_iso8601);
    let updated = get("updated").and_then(|v| v.as_str().map(|s| s.to_string())).unwrap_or_else(now_iso8601);
    let revision = get("revision")
        .and_then(|v| v.as_u64())
        .filter(|&r| r > 0)
        .unwrap_or(1) as u32;
    let tags = get("tags")
        .and_then(|v| v.as_sequence().map(|seq| {
            seq.iter().filter_map(|i| i.as_str().map(|s| s.to_string())).collect()
        }))
        .unwrap_or_default();
    let evidence = get("evidence")
        .and_then(|v| v.as_sequence().map(|seq| {
            seq.iter().filter_map(|i| i.as_str().map(|s| s.to_string())).collect()
        }))
        .unwrap_or_default();

    Node {
        id: stem.to_string(),
        node_type: type_val,
        title,
        parent,
        status,
        created,
        updated,
        revision,
        tags,
        evidence,
        body,
        folded: None,
    }
}

/// 标题兜底：正文第一个 markdown 一级标题；否则用 id
fn first_title_or(fallback: &str, content: &str) -> String {
    for line in content.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("# ") {
            let title = rest.trim();
            if !title.is_empty() {
                return title.to_string();
            }
        }
    }
    fallback.to_string()
}

/// v1.2：检测 .chain/AI_GUIDE.md 版本是否陈旧（无标记或低于内嵌版本）→ 追加 warning。
/// 不自动刷新——扫描只读；刷新动作由 init_chain 或软件启动向导完成。
fn check_guide_staleness(chain_dir: &std::path::Path, warnings: &mut Vec<String>) {
    use crate::commands::ai_guide::{AI_GUIDE_VERSION, parse_guide_version};

    let guide = chain_dir.join("AI_GUIDE.md");
    let result = if !guide.exists() {
        Some(format!("AI_GUIDE.md 缺失（当前软件版本 v{AI_GUIDE_VERSION}）"))
    } else {
        match std::fs::read_to_string(&guide) {
            Ok(content) => match parse_guide_version(&content) {
                None => Some(format!(
                    "AI_GUIDE.md 无版本标记，可能是旧版（当前软件版本 v{AI_GUIDE_VERSION}），建议重新初始化刷新"
                )),
                Some(v) if v < AI_GUIDE_VERSION => Some(format!(
                    "AI_GUIDE.md 版本过旧（盘上 v{v}，当前 v{AI_GUIDE_VERSION}），建议重新初始化刷新"
                )),
                _ => None,
            },
            Err(_) => None,
        }
    };
    if let Some(w) = result {
        warnings.push(w);
    }
}

/// 统计各状态节点数 + 根目标标题
fn build_chain_health(nodes: &[Node]) -> ChainHealth {
    let mut h = ChainHealth {
        blocked_count: 0,
        failed_count: 0,
        in_progress_count: 0,
        pending_count: 0,
        success_count: 0,
        root_goal: String::new(),
    };
    for n in nodes {
        match n.status {
            crate::model::node::NodeStatus::Blocked => h.blocked_count += 1,
            crate::model::node::NodeStatus::Failed => h.failed_count += 1,
            crate::model::node::NodeStatus::InProgress => h.in_progress_count += 1,
            crate::model::node::NodeStatus::Pending => h.pending_count += 1,
            crate::model::node::NodeStatus::Success => h.success_count += 1,
        }
        if n.parent.is_none() {
            h.root_goal = n.title.clone();
        }
    }
    h
}

/// 生成紧凑树状摘要（≤200 token），只展示非 success 节点。
/// 开发模式：可能无根（全部成环）或多根——无根时取第一个节点起画（visited 防环兜底）。
fn build_active_chain(nodes: &[Node], mode: ScanMode) -> String {
    use std::collections::{HashMap, HashSet};
    use crate::model::node::NodeStatus;

    // 构建 children map
    let mut children: HashMap<&str, Vec<&Node>> = HashMap::new();
    for n in nodes {
        if let Some(ref p) = n.parent {
            children.entry(p.as_str()).or_default().push(n);
        }
    }

    // 找根节点
    let root = match nodes.iter().find(|n| n.parent.is_none()) {
        Some(r) => r,
        None => {
            if nodes.is_empty() {
                return "(无节点)".to_string();
            }
            if mode.is_dev() {
                &nodes[0] // 开发模式全部成环时，从第一个节点开始（visited 防环）
            } else {
                return "(无根节点)".to_string();
            }
        }
    };

    let status_icon = |s: &NodeStatus| -> &str {
        match s {
            NodeStatus::Pending => "⏳",
            NodeStatus::InProgress => "🔄",
            NodeStatus::Success => "✅",
            NodeStatus::Failed => "❌",
            NodeStatus::Blocked => "🚫",
        }
    };

    let type_icon = |n: &Node| -> &str {
        match n.node_type {
            crate::model::node::NodeType::Goal => "🎯",
            crate::model::node::NodeType::Design => "📐",
            crate::model::node::NodeType::Task => "🔧",
            crate::model::node::NodeType::Verification => "🔍",
        }
    };

    let mut lines = Vec::new();
    let mut visited: HashSet<&str> = HashSet::new();
    fn walk<'a>(
        node: &'a Node,
        children: &HashMap<&str, Vec<&'a Node>>,
        prefix: &str,
        is_last: bool,
        lines: &mut Vec<String>,
        visited: &mut HashSet<&'a str>,
        type_icon: &dyn Fn(&Node) -> &'static str,
        status_icon: &dyn Fn(&NodeStatus) -> &'static str,
    ) {
        if !visited.insert(node.id.as_str()) {
            return; // 防循环：重复 id 导致的无限递归
        }
        let connector = if prefix.is_empty() { "" } else if is_last { "└─ " } else { "├─ " };
        let line = format!(
            "{}{}{} {} [{}]",
            prefix, connector, type_icon(node), node.title, status_icon(&node.status)
        );
        lines.push(line);

        // 只递归非 success 的子节点
        let kids = children.get(node.id.as_str());
        if let Some(kids) = kids {
            let active_kids: Vec<&&Node> = kids
                .iter()
                .filter(|k| k.status != NodeStatus::Success)
                .collect();
            if active_kids.is_empty() {
                return;
            }
            let child_prefix = if prefix.is_empty() {
                "   ".to_string()
            } else if is_last {
                format!("{}    ", prefix)
            } else {
                format!("{}│   ", prefix)
            };
            for (i, kid) in active_kids.iter().enumerate() {
                let last = i == active_kids.len() - 1;
                walk(kid, children, &child_prefix, last, lines, visited, type_icon, status_icon);
            }
        }
    }

    walk(root, &children, "", true, &mut lines, &mut visited, &type_icon, &status_icon);

    // 截断到 ~200 token（约 400 字节）。用 truncate_utf8 按字符边界截断，
    // 否则中文标题下 400 字节恰好落在汉字中间会 panic（已实证）。
    let mut result = lines.join("\n");
    if result.len() > 400 {
        let mut truncated = crate::scanner::frontmatter::truncate_utf8(&result, 400).to_string();
        truncated.push_str("\n…(截断，完整树见 nodes/)");
        result = truncated;
    }
    result
}

/// 从根 goal 和 AI_GUIDE.md 提取项目画像
fn build_project_persona(nodes: &[Node], chain_dir: &std::path::Path) -> Option<ProjectPersona> {
    let root = nodes.iter().find(|n| n.parent.is_none())?;

    // 从根 goal 正文提取技术栈关键词。
    // 词边界匹配：避免 "Go" 命中 "Google"、"C#" 命中 "C#-like" 之类误报。
    let body = &root.body;
    let mut tech_stack = Vec::new();
    let tech_keywords = [
        "Rust", "Svelte", "Tauri", "TypeScript", "Unity", "CUDA", "C++", "C#",
        "Python", "JavaScript", "React", "Vue", "Go", "Java", "Kotlin", "Swift",
        "OpenGL", "Vulkan", "WebGPU", "Compute Shader", "HLSL", "GLSL",
        "URP", "HDRP", "FFT", "PBR", "Ray Tracing", "Path Tracing",
    ];
    for kw in &tech_keywords {
        if contains_keyword(body, kw) {
            tech_stack.push(kw.to_string());
        }
    }

    // 从根 goal 标题推断领域
    let domain = if root.title.contains("渲染") || root.title.contains("shader") || root.title.contains("图形") {
        "rendering".to_string()
    } else if root.title.contains("游戏") || root.title.contains("game") {
        "game_dev".to_string()
    } else if root.title.contains("工程") || root.title.contains("工具") {
        "dev_tools".to_string()
    } else if root.title.contains("AI") || root.title.contains("模型") {
        "ai_ml".to_string()
    } else {
        "general".to_string()
    };

    // 从 AI_GUIDE.md 提取关键约定
    let mut key_conventions = Vec::new();
    let guide_path = chain_dir.join("AI_GUIDE.md");
    if let Ok(guide) = std::fs::read_to_string(&guide_path) {
        if guide.contains("commit 带基线") || guide.contains("基线号") {
            key_conventions.push("commit带基线号".to_string());
        }
        if guide.contains("单源") {
            key_conventions.push("单源指南".to_string());
        }
        if guide.contains("不拍脑袋") {
            key_conventions.push("有依据再动手".to_string());
        }
        if guide.contains("受控回溯") {
            key_conventions.push("受控回溯".to_string());
        }
    }

    let coding_style = if tech_stack.contains(&"Rust".to_string()) {
        "Rust edition 2021, anyhow/serde".to_string()
    } else if tech_stack.contains(&"CUDA".to_string()) || tech_stack.contains(&"C++".to_string()) {
        "UTF-8 with BOM".to_string()
    } else {
        String::new()
    };

    if tech_stack.is_empty() && key_conventions.is_empty() && domain == "general" {
        return None;
    }

    Some(ProjectPersona {
        domain,
        tech_stack,
        coding_style,
        key_conventions,
    })
}

/// 词边界匹配：关键词前后必须是「非字母数字」字符才算命中。
/// 特殊符号结尾的关键词（C++/C#）按「关键词+符号」整体识别，
/// 其后仍要求边界（避免 C# 命中 C#-like？不——C# 与 - 之间无边界需求，容忍）。
fn contains_keyword(haystack: &str, kw: &str) -> bool {
    let bytes = haystack.as_bytes();
    let kwb = kw.as_bytes();
    if kwb.is_empty() {
        return false;
    }
    let mut start = 0;
    while let Some(pos) = haystack[start..].find(kw) {
        let abs_pos = start + pos;
        let before_ok = if abs_pos == 0 {
            true
        } else {
            let prev = bytes[abs_pos - 1];
            !(prev.is_ascii_alphanumeric())
        };
        let after_idx = abs_pos + kw.len();
        let after_ok = if after_idx >= bytes.len() {
            true
        } else {
            let next = bytes[after_idx];
            !(next.is_ascii_alphanumeric())
        };
        if before_ok && after_ok {
            return true;
        }
        start = abs_pos + 1;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_scan_empty_chain_dir() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join(".chain").join("nodes")).unwrap();

        let snapshot = scan_chain_dir(root).unwrap();
        assert_eq!(snapshot.nodes.len(), 0);
        assert_eq!(snapshot.edges.len(), 0);
        assert_eq!(snapshot.manifest.node_count, 0);
    }

    #[test]
    fn test_scan_multiple_nodes() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let nodes_dir = root.join(".chain").join("nodes");
        fs::create_dir_all(&nodes_dir).unwrap();

        fs::write(
            nodes_dir.join("g-001.md"),
            "---\nid: g-001\ntype: goal\ntitle: 顶层目标\nparent: null\nstatus: pending\ncreated: 2026-08-13T10:00:00+08:00\nupdated: 2026-08-13T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# 顶层目标\n",
        ).unwrap();

        fs::write(
            nodes_dir.join("d-001.md"),
            "---\nid: d-001\ntype: design\ntitle: 设计1\nparent: g-001\nstatus: in_progress\ncreated: 2026-08-13T10:00:00+08:00\nupdated: 2026-08-13T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# 设计1\n",
        ).unwrap();

        fs::write(
            nodes_dir.join("t-001.md"),
            "---\nid: t-001\ntype: task\ntitle: 任务1\nparent: d-001\nstatus: pending\ncreated: 2026-08-13T10:00:00+08:00\nupdated: 2026-08-13T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# 任务1\n",
        ).unwrap();

        let snapshot = scan_chain_dir(root).unwrap();
        assert_eq!(snapshot.nodes.len(), 3);
        assert_eq!(snapshot.edges.len(), 2);
        assert_eq!(snapshot.manifest.node_count, 3);
        assert_eq!(snapshot.manifest.edge_count, 2);
        assert!(snapshot.validation.valid, "合法数据应通过校验: {:?}", snapshot.validation.errors);
    }

    #[test]
    fn test_scan_no_chain_dir() {
        let tmp = TempDir::new().unwrap();
        let result = scan_chain_dir(tmp.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_guide_stale_warning_on_scan() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let nodes_dir = root.join(".chain").join("nodes");
        fs::create_dir_all(&nodes_dir).unwrap();
        fs::write(
            nodes_dir.join("g-001.md"),
            "---\nid: g-001\ntype: goal\ntitle: 顶层目标\nparent: null\nstatus: pending\ncreated: 2026-08-13T10:00:00+08:00\nupdated: 2026-08-13T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# 顶层目标\n",
        ).unwrap();
        // 写一个无版本标记的旧版指南
        fs::write(root.join(".chain").join("AI_GUIDE.md"), "旧版无标记指南").unwrap();

        let snap = scan_chain_dir(root).unwrap();
        assert!(snap.validation.warnings.iter().any(|w| w.contains("无版本标记")),
                "无标记指南应产生 warning: {:?}", snap.validation.warnings);
    }

    #[test]
    fn test_guide_current_no_warning() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let nodes_dir = root.join(".chain").join("nodes");
        fs::create_dir_all(&nodes_dir).unwrap();
        fs::write(
            nodes_dir.join("g-001.md"),
            "---\nid: g-001\ntype: goal\ntitle: 顶层目标\nparent: null\nstatus: pending\ncreated: 2026-08-13T10:00:00+08:00\nupdated: 2026-08-13T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# 顶层目标\n",
        ).unwrap();
        // 写一个当前版本标记的指南
        fs::write(
            root.join(".chain").join("AI_GUIDE.md"),
            format!("<!-- CHAIN_GUIDE_VERSION: {} -->\n新版指南", crate::commands::ai_guide::AI_GUIDE_VERSION),
        ).unwrap();

        let snap = scan_chain_dir(root).unwrap();
        assert!(!snap.validation.warnings.iter().any(|w| w.contains("AI_GUIDE.md")),
                "当前版本指南不应有陈旧 warning: {:?}", snap.validation.warnings);
    }

    // ── v1.3 新增测试：chain_health / active_chain / project_persona ──

    #[test]
    fn test_chain_health_counts() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let nodes_dir = root.join(".chain").join("nodes");
        fs::create_dir_all(&nodes_dir).unwrap();
        let node = |id: &str, node_type: &str, status: &str, title: &str, parent: &str| {
            let parent_line = if parent == "null" { "parent: null".to_string() } else { format!("parent: {parent}") };
            format!("---\nid: {id}\ntype: {node_type}\ntitle: {title}\n{parent_line}\nstatus: {status}\ncreated: 2026-08-13T10:00:00+08:00\nupdated: 2026-08-13T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# {title}\n")
        };
        fs::write(nodes_dir.join("g-001.md"), node("g-001", "goal", "in_progress", "根目标", "null")).unwrap();
        fs::write(nodes_dir.join("t-001.md"), node("t-001", "task", "success", "任务1", "g-001")).unwrap();
        fs::write(nodes_dir.join("t-002.md"), node("t-002", "task", "failed", "任务2", "g-001")).unwrap();
        fs::write(nodes_dir.join("t-003.md"), node("t-003", "task", "blocked", "任务3", "g-001")).unwrap();
        fs::write(nodes_dir.join("t-004.md"), node("t-004", "task", "pending", "任务4", "g-001")).unwrap();

        let snap = scan_chain_dir(root).unwrap();
        let h = &snap.manifest.chain_health;
        assert_eq!(h.in_progress_count, 1);
        assert_eq!(h.success_count, 1);
        assert_eq!(h.failed_count, 1);
        assert_eq!(h.blocked_count, 1);
        assert_eq!(h.pending_count, 1);
        assert_eq!(h.root_goal, "根目标");
    }

    #[test]
    fn test_active_chain_filters_success() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let nodes_dir = root.join(".chain").join("nodes");
        fs::create_dir_all(&nodes_dir).unwrap();
        let node = |id: &str, node_type: &str, status: &str, title: &str, parent: &str| {
            let parent_line = if parent == "null" { "parent: null".to_string() } else { format!("parent: {parent}") };
            format!("---\nid: {id}\ntype: {node_type}\ntitle: {title}\n{parent_line}\nstatus: {status}\ncreated: 2026-08-13T10:00:00+08:00\nupdated: 2026-08-13T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# {title}\n")
        };
        fs::write(nodes_dir.join("g-001.md"), node("g-001", "goal", "in_progress", "根目标", "null")).unwrap();
        fs::write(nodes_dir.join("t-001.md"), node("t-001", "task", "success", "已完成任务", "g-001")).unwrap();
        fs::write(nodes_dir.join("t-002.md"), node("t-002", "task", "pending", "待办任务", "g-001")).unwrap();

        let snap = scan_chain_dir(root).unwrap();
        let a = &snap.manifest.active_chain;
        assert!(a.contains("根目标"), "active_chain 应含根: {a}");
        assert!(a.contains("待办任务"), "active_chain 应含 pending 节点: {a}");
        assert!(!a.contains("已完成任务"), "active_chain 不应含 success 节点: {a}");
        assert!(a.contains("⏳"), "pending 应显示 ⏳ 图标: {a}");
    }

    #[test]
    fn test_active_chain_chinese_truncation_no_panic() {
        // 中文长标题把 active_chain 撑过 400 字节，截断必须落在字符边界（不 panic）
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let nodes_dir = root.join(".chain").join("nodes");
        fs::create_dir_all(&nodes_dir).unwrap();
        let long_title = "这是一个非常长的中文任务标题用于撑过字节截断边界".repeat(4); // 24字*4=96字=288字节/条
        let node = |id: &str, node_type: &str, status: &str, title: &str, parent: &str| {
            let parent_line = if parent == "null" { "parent: null".to_string() } else { format!("parent: {parent}") };
            format!("---\nid: {id}\ntype: {node_type}\ntitle: {title}\n{parent_line}\nstatus: {status}\ncreated: 2026-08-13T10:00:00+08:00\nupdated: 2026-08-13T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# {title}\n")
        };
        fs::write(nodes_dir.join("g-001.md"), node("g-001", "goal", "in_progress", "根目标", "null")).unwrap();
        for i in 1..=8 {
            let id = format!("t-{:03}", i);
            fs::write(nodes_dir.join(format!("{id}.md")), node(&id, "task", "pending", &long_title, "g-001")).unwrap();
        }

        let snap = scan_chain_dir(root).unwrap();
        let a = &snap.manifest.active_chain;
        assert!(a.len() <= 450, "active_chain 应被截断: len={}", a.len());
        assert!(a.contains("截断"), "超长应标注截断: {a}");
    }

    #[test]
    fn test_project_persona_tech_stack() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let nodes_dir = root.join(".chain").join("nodes");
        fs::create_dir_all(&nodes_dir).unwrap();
        fs::write(
            nodes_dir.join("g-001.md"),
            "---\nid: g-001\ntype: goal\ntitle: 优化路径追踪实时渲染\nparent: null\nstatus: in_progress\ncreated: 2026-08-13T10:00:00+08:00\nupdated: 2026-08-13T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# 优化路径追踪实时渲染\n\n使用 CUDA 与 C++ 实现 Path Tracing 降噪。\n",
        ).unwrap();

        let snap = scan_chain_dir(root).unwrap();
        let p = snap.manifest.project_persona.as_ref().expect("应提取到画像");
        assert_eq!(p.domain, "rendering");
        assert!(p.tech_stack.iter().any(|t| t == "CUDA"));
        assert!(p.tech_stack.iter().any(|t| t == "C++"));
        assert!(p.tech_stack.iter().any(|t| t == "Path Tracing"));
        assert_eq!(p.coding_style, "UTF-8 with BOM");
    }

    #[test]
    fn test_project_persona_no_false_positive() {
        // "Google" 不应命中 "Go"；无技术栈且 general 领域 → None
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let nodes_dir = root.join(".chain").join("nodes");
        fs::create_dir_all(&nodes_dir).unwrap();
        fs::write(
            nodes_dir.join("g-001.md"),
            "---\nid: g-001\ntype: goal\ntitle: 周末去哪玩\nparent: null\nstatus: pending\ncreated: 2026-08-13T10:00:00+08:00\nupdated: 2026-08-13T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# 周末去哪玩\n\n用 Google 搜索攻略。\n",
        ).unwrap();

        let snap = scan_chain_dir(root).unwrap();
        assert!(snap.manifest.project_persona.is_none(), "无技术栈的通用内容不应生成画像");
    }

    // ── v2.0 开发模式（自由知识图谱）测试 ──

    #[test]
    fn test_dev_mode_plain_md_becomes_node() {
        // 无 frontmatter 的普通 .md 也是节点：id=文件名、标题=首个 H1、字段默认
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let nodes_dir = root.join(".chain").join("nodes");
        fs::create_dir_all(&nodes_dir).unwrap();
        fs::write(nodes_dir.join("物理笔记.md"), "# 费曼讲义要点\n\n正文内容随便写。").unwrap();
        fs::write(nodes_dir.join("杂记.md"), "没有任何标题的纯文本。").unwrap();

        let snap = scan_chain_dir_mode(root, ScanMode::Dev).unwrap();
        assert!(snap.validation.valid, "开发模式无内容要求: {:?}", snap.validation.errors);
        assert_eq!(snap.nodes.len(), 2);
        let feynman = snap.nodes.iter().find(|n| n.id == "物理笔记").unwrap();
        assert_eq!(feynman.title, "费曼讲义要点");
        assert_eq!(feynman.node_type, crate::model::node::NodeType::Task);
        assert_eq!(feynman.parent, None);
        let misc = snap.nodes.iter().find(|n| n.id == "杂记").unwrap();
        assert_eq!(misc.title, "杂记");
    }

    #[test]
    fn test_dev_mode_allows_multi_root_dangling_cycle() {
        // 多根 + 悬空 parent + 环：全部合法（无结构错误）
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let nodes_dir = root.join(".chain").join("nodes");
        fs::create_dir_all(&nodes_dir).unwrap();
        let node = |id: &str, parent: &str| {
            format!("---\nid: {id}\ntype: task\ntitle: {id}\nparent: {parent}\nstatus: pending\ncreated: 2026-08-27T10:00:00+08:00\nupdated: 2026-08-27T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# {id}\n")
        };
        fs::write(nodes_dir.join("a.md"), node("a", "null")).unwrap();      // 根 1
        fs::write(nodes_dir.join("b.md"), node("b", "null")).unwrap();      // 根 2（多根 OK）
        fs::write(nodes_dir.join("c.md"), node("c", "ghost")).unwrap();     // 悬空 parent → 边静默跳过
        fs::write(nodes_dir.join("d.md"), node("d", "a")).unwrap();
        fs::write(nodes_dir.join("e.md"), node("e", "d")).unwrap();

        let snap = scan_chain_dir_mode(root, ScanMode::Dev).unwrap();
        assert!(snap.validation.valid, "开发模式应全合法: {:?}", snap.validation.errors);
        assert_eq!(snap.nodes.len(), 5);
        // 悬空 c→ghost 无边；a→d、d→e 两条
        assert_eq!(snap.edges.len(), 2, "悬空边应被跳过: {:?}", snap.edges);

        // 再改成环 a→c→a：也合法，且 active_chain 不 panic
        fs::write(nodes_dir.join("a.md"), node("a", "c")).unwrap();
        let snap2 = scan_chain_dir_mode(root, ScanMode::Dev).unwrap();
        assert!(snap2.validation.valid, "环在开发模式应合法: {:?}", snap2.validation.errors);
        assert!(!snap2.manifest.active_chain.is_empty());
    }

    #[test]
    fn test_dev_mode_skips_guide_warning() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let nodes_dir = root.join(".chain").join("nodes");
        fs::create_dir_all(&nodes_dir).unwrap();
        fs::write(nodes_dir.join("a.md"), "# 随便写\n\n无协议内容。").unwrap();

        let snap = scan_chain_dir_mode(root, ScanMode::Dev).unwrap();
        assert!(!snap.validation.warnings.iter().any(|w| w.contains("AI_GUIDE")),
                "开发模式不应有指南陈旧告警: {:?}", snap.validation.warnings);
        // 分析模式同样数据应报指南缺失 warning
        let snap_analysis = scan_chain_dir(root).unwrap();
        assert!(snap_analysis.validation.warnings.iter().any(|w| w.contains("AI_GUIDE")));
    }
}
