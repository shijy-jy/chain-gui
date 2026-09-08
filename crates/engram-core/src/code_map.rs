//! M-Code 代码骨架（框架 §5.7 / ADR 0010 / T12–T14）：tree-sitter-rust 试点语言。
//! - 提取：公开接口优先（pub fn/struct/trait/enum/impl + 签名）→ 调用边（同文件内解析）→ Mermaid 文本
//! - 派生物：`.chain/code_map/<node-id>.md`（骨架，可重建、不进事实源，ADR 0004）
//! - 挂载：节点 frontmatter `code_map: <相对路径>` 引用源码（文件或目录）；正文只放一句概述（T13）
//! - stale：`.chain/code_map/<node-id>.stale` 标记文件（watcher 联动 T14）；refresh 后清除
//! 验收（框架 §7）：用 Engram 自身验证——提取 engram-core 骨架，人读骨架能还原模块职责。

use crate::ops::atomic_write;
use std::path::Path;

#[derive(Debug)]
pub struct ExportSig {
    pub name: String,
    /// fn | struct | trait | enum | impl
    pub kind: String,
    pub signature: String,
    /// 相对路径:行:列（行/列 1 基）
    pub loc: String,
}

#[derive(Debug)]
pub struct CallEdge {
    pub from: String,
    pub to: String,
    pub loc: String,
}

#[derive(Debug)]
pub struct Skeleton {
    pub node_id: String,
    pub language: String,
    pub exports: Vec<ExportSig>,
    pub call_edges: Vec<CallEdge>,
    pub mermaid: String,
    pub stale: bool,
}

fn code_map_dir(root: &Path) -> std::path::PathBuf {
    root.join(".chain").join("code_map")
}

fn skeleton_path(root: &Path, node_id: &str) -> std::path::PathBuf {
    code_map_dir(root).join(format!("{node_id}.md"))
}

fn stale_marker(root: &Path, node_id: &str) -> std::path::PathBuf {
    code_map_dir(root).join(format!("{node_id}.stale"))
}

// ── tree-sitter 提取 ───────────────────────────────────────

fn ts_language() -> Result<tree_sitter::Language, String> {
    Ok(tree_sitter_rust::language())
}

fn node_text<'a>(node: tree_sitter::Node, source: &'a [u8]) -> &'a str {
    node.utf8_text(source).unwrap_or("")
}

/// 名称（identifier 即名本身；其余取 name 字段；scoped_identifier 取末段）
fn name_of(node: tree_sitter::Node, source: &[u8]) -> Option<String> {
    if matches!(
        node.kind(),
        "identifier" | "type_identifier" | "field_identifier"
    ) {
        return Some(node_text(node, source).trim().to_string());
    }
    if let Some(id) = node.child_by_field_name("name") {
        if matches!(id.kind(), "identifier" | "type_identifier" | "field_identifier") {
            return Some(node_text(id, source).trim().to_string());
        }
    }
    // scoped_identifier（a::b）→ 末段
    if node.kind() == "scoped_identifier" {
        let mut last = None;
        let mut cursor = node.walk();
        for c in node.children(&mut cursor) {
            if matches!(c.kind(), "identifier" | "type_identifier") && c.is_named() {
                last = Some(node_text(c, source).trim().to_string());
            }
        }
        return last;
    }
    None
}

fn is_pub(node: tree_sitter::Node, source: &[u8]) -> bool {
    // tree-sitter-rust 0.21：visibility_modifier 是子节点而非具名字段
    let mut cursor = node.walk();
    for c in node.children(&mut cursor) {
        if c.kind() == "visibility_modifier" && node_text(c, source).contains("pub") {
            return true;
        }
    }
    false
}

fn loc_str(path: &str, node: tree_sitter::Node) -> String {
    let p = node.start_position();
    format!("{}:{}:{}", path, p.row + 1, p.column + 1)
}

/// 提取单个 Rust 源文件。namespaced 前缀来自模块栈；known_fns 跨两遍收集调用边。
#[allow(clippy::too_many_arguments)]
fn extract_file<'a>(
    path: &str,
    source: &'a [u8],
    tree: &tree_sitter::Tree,
    exports: &mut Vec<ExportSig>,
    edges: &mut Vec<CallEdge>,
) {
    // 一遍：收集全部函数全名（调用边解析用，pub/私有都算）
    let mut known: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    {
        fn walk_names(
            node: tree_sitter::Node,
            source: &[u8],
            ns: &mut Vec<String>,
            known: &mut std::collections::BTreeSet<String>,
        ) {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                let mut pushed = false;
                if child.kind() == "mod_item" {
                    if let Some(n) = name_of(child, source) {
                        ns.push(n);
                        pushed = true;
                    }
                }
                if child.kind() == "function_item" {
                    if let Some(n) = name_of(child, source) {
                        let full = if ns.is_empty() {
                            n
                        } else {
                            format!("{}::{}", ns.join("::"), n)
                        };
                        known.insert(full);
                    }
                }
                if child.is_named() {
                    walk_names(child, source, ns, known);
                }
                if pushed {
                    ns.pop();
                }
            }
        }
        let mut ns = Vec::new();
        walk_names(tree.root_node(), source, &mut ns, &mut known);
    }

    // 二遍：导出接口 + 调用边
    fn walk<'a>(
        node: tree_sitter::Node,
        source: &'a [u8],
        path: &str,
        ns: &mut Vec<String>,
        fn_stack: &mut Vec<String>,
        exports: &mut Vec<ExportSig>,
        edges: &mut Vec<CallEdge>,
        known: &std::collections::BTreeSet<String>,
    ) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let kind = child.kind();
            let mut pushed_ns = false;
            if kind == "mod_item" {
                if let Some(n) = name_of(child, source) {
                    ns.push(n);
                    pushed_ns = true;
                }
            }
            if kind == "function_item" {
                let full = name_of(child, source)
                    .map(|n| {
                        if ns.is_empty() {
                            n
                        } else {
                            format!("{}::{}", ns.join("::"), n)
                        }
                    })
                    .unwrap_or_default();
                if is_pub(child, source) {
                    // 签名 = 声明头（body 之前），多行压缩为单行
                    let body = child.child_by_field_name("body");
                    let end = body
                        .map(|b| b.start_byte())
                        .unwrap_or(child.end_byte());
                    let head = &source[child.start_byte()..end];
                    let sig = String::from_utf8_lossy(head)
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ");
                    exports.push(ExportSig {
                        name: full.clone(),
                        kind: "fn".into(),
                        signature: crate::scanner::frontmatter::truncate_utf8(&sig, 300).to_string(),
                        loc: loc_str(path, child),
                    });
                }
                // 函数体：调用边（同文件内解析）
                fn_stack.push(full.clone());
                if let Some(body) = child.child_by_field_name("body") {
                    collect_calls(body, source, path, &full, fn_stack, edges, known);
                }
                fn_stack.pop();
            }
            if matches!(kind, "struct_item" | "trait_item" | "enum_item") && is_pub(child, source) {
                let full = name_of(child, source)
                    .map(|n| {
                        if ns.is_empty() {
                            n
                        } else {
                            format!("{}::{}", ns.join("::"), n)
                        }
                    })
                    .unwrap_or_default();
                let sig = String::from_utf8_lossy(&source[child.start_byte()..child.end_byte()])
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                exports.push(ExportSig {
                    name: full.clone(),
                    kind: kind.trim_end_matches("_item").into(),
                    signature: crate::scanner::frontmatter::truncate_utf8(&sig, 300).to_string(),
                    loc: loc_str(path, child),
                });
            }
            if kind == "impl_item" {
                // impl [Trait] for Type —— impl 块本身不携带 pub（可见性在方法上），
                // 一律作为公开接口面导出；方法名以 "impl Type::method" 命名空间化
                let trait_name = child
                    .child_by_field_name("trait")
                    .and_then(|t| name_of(t, source));
                let type_name = child
                    .child_by_field_name("type")
                    .and_then(|t| name_of(t, source))
                    .unwrap_or_default();
                let impl_name = match &trait_name {
                    Some(t) => format!("impl {t} for {type_name}"),
                    None => format!("impl {type_name}"),
                };
                exports.push(ExportSig {
                    name: impl_name.clone(),
                    kind: "impl".into(),
                    signature: crate::scanner::frontmatter::truncate_utf8(&impl_name, 300)
                        .to_string(),
                    loc: loc_str(path, child),
                });
                // impl 方法（位于 body 字段下）：导出 pub fn + 调用边
                if let Some(impl_body) = child.child_by_field_name("body") {
                    let mut mc = impl_body.walk();
                    for m in impl_body.children(&mut mc) {
                        if m.kind() == "function_item" && is_pub(m, source) {
                            let full = name_of(m, source)
                                .map(|n| format!("{impl_name}::{n}"))
                                .unwrap_or(impl_name.clone());
                            let body = m.child_by_field_name("body");
                            let end = body.map(|b| b.start_byte()).unwrap_or(m.end_byte());
                            let head = &source[m.start_byte()..end];
                            let sig = String::from_utf8_lossy(head)
                                .split_whitespace()
                                .collect::<Vec<_>>()
                                .join(" ");
                            exports.push(ExportSig {
                                name: full.clone(),
                                kind: "fn".into(),
                                signature: crate::scanner::frontmatter::truncate_utf8(&sig, 300)
                                    .to_string(),
                                loc: loc_str(path, m),
                            });
                            fn_stack.push(full.clone());
                            if let Some(body) = m.child_by_field_name("body") {
                                collect_calls(body, source, path, &full, fn_stack, edges, known);
                            }
                            fn_stack.pop();
                        }
                    }
                }
            }
            // impl 子树已在上面处理，递归跳过（防内部方法被通用分支以裸名重复导出）
            if child.is_named() && kind != "impl_item" {
                walk(
                    child, source, path, ns, fn_stack, exports, edges, known,
                );
            }
            if pushed_ns {
                ns.pop();
            }
        }
    }
    fn collect_calls(
        node: tree_sitter::Node,
        source: &[u8],
        path: &str,
        caller: &str,
        _fn_stack: &mut Vec<String>,
        edges: &mut Vec<CallEdge>,
        known: &std::collections::BTreeSet<String>,
    ) {
        if node.kind() == "call_expression" {
            let target = node
                .child_by_field_name("function")
                .and_then(|f| name_of(f, source))
                .unwrap_or_default();
            if !target.is_empty() && known.contains(&target) && target != caller {
                edges.push(CallEdge {
                    from: caller.to_string(),
                    to: target,
                    loc: loc_str(path, node),
                });
            }
        }
        let mut cursor = node.walk();
        for c in node.children(&mut cursor) {
            collect_calls(c, source, path, caller, _fn_stack, edges, known);
        }
    }

    let mut ns = Vec::new();
    let mut fn_stack = Vec::new();
    walk(
        tree.root_node(),
        source,
        path,
        &mut ns,
        &mut fn_stack,
        exports,
        edges,
        &known,
    );
}

/// 提取骨架（框架 §5.7）。src 可为单个源文件或目录（目录递归 *.rs，loc 用相对路径）。
/// 试点语言：rust（T12 拍板）。
pub fn extract_skeleton(src: &Path, node_id: &str, lang: &str) -> Result<Skeleton, String> {
    if lang != "rust" {
        return Err(format!("不支持的试点语言「{lang}」，当前仅支持 rust"));
    }
    let mut exports = Vec::new();
    let mut edges = Vec::new();
    let mut files: Vec<(String, std::path::PathBuf)> = Vec::new();
    if src.is_file() {
        files.push((
            src.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            src.to_path_buf(),
        ));
    } else if src.is_dir() {
        for entry in walkdir::WalkDir::new(src)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let p = entry.path();
            if p.extension().and_then(|s| s.to_str()) == Some("rs") {
                let rel = p
                    .strip_prefix(src)
                    .unwrap_or(p)
                    .to_string_lossy()
                    .replace('\\', "/");
                files.push((rel, p.to_path_buf()));
            }
        }
    } else {
        return Err(format!("源码路径不存在：{}", src.display()));
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    if files.is_empty() {
        return Err(format!("源码目录无 .rs 文件：{}", src.display()));
    }
    for (rel, p) in files {
        let source = std::fs::read(p.clone())
            .map_err(|e| format!("读源码失败 {}：{e}", p.display()))?;
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&ts_language()?)
            .map_err(|e| format!("tree-sitter 语言加载失败：{e}"))?;
        let tree = parser
            .parse(&source, None)
            .ok_or_else(|| format!("解析失败：{}", p.display()))?;
        extract_file(&rel, &source, &tree, &mut exports, &mut edges);
    }
    let mermaid = build_mermaid(&exports, &edges);
    Ok(Skeleton {
        node_id: node_id.to_string(),
        language: lang.to_string(),
        exports,
        call_edges: edges,
        mermaid,
        stale: false,
    })
}

/// Mermaid 文本（flowchart LR；导出为节点、调用为边；命名清洗后确定性排序）
fn build_mermaid(exports: &[ExportSig], edges: &[CallEdge]) -> String {
    let mut lines = vec!["flowchart LR".to_string()];
    // 节点 id 映射（清洗：仅保留字母数字下划线；重复名共享同 id）
    let mut ids: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    let mut node_lines: Vec<String> = Vec::new();
    let mut add_node = |name: &str, ids: &mut std::collections::BTreeMap<String, String>| {
        let key = name.to_string();
        if ids.contains_key(&key) {
            return;
        }
        let id = format!("n{}", ids.len() + 1);
        let label = name.replace('"', "'");
        node_lines.push(format!("  {id}[\"{label}\"]"));
        ids.insert(key, id);
    };
    for e in exports {
        add_node(&e.name, &mut ids);
    }
    for e in edges {
        add_node(&e.from, &mut ids);
        add_node(&e.to, &mut ids);
    }
    node_lines.sort();
    lines.extend(node_lines);
    let mut edge_lines: Vec<String> = edges
        .iter()
        .map(|e| format!("  {} --> {}", ids[&e.from], ids[&e.to]))
        .collect();
    edge_lines.sort();
    edge_lines.dedup();
    lines.extend(edge_lines);
    lines.join("\n")
}

/// 骨架 → markdown（.chain/code_map/<id>.md 的正文，框架 §5.7）
pub fn skeleton_to_markdown(s: &Skeleton) -> String {
    let mut out = Vec::new();
    out.push(format!("# 代码骨架：{}（{}）", s.node_id, s.language));
    out.push(String::new());
    out.push(format!(
        "> 状态：stale: {} · 生成：{}",
        s.stale,
        crate::scanner::frontmatter::now_iso8601()
    ));
    out.push(String::new());
    out.push(format!("## 导出接口（{}）", s.exports.len()));
    for e in &s.exports {
        out.push(format!("- `{} {}`（{}）", e.kind, e.name, e.loc));
        out.push(String::new());
        out.push("```rust".to_string());
        out.push(e.signature.clone());
        out.push("```".to_string());
        out.push(String::new());
    }
    out.push("## 调用关系".to_string());
    out.push(String::new());
    out.push("```mermaid".to_string());
    out.push(s.mermaid.clone());
    out.push("```".to_string());
    out.push(String::new());
    out.push(format!("## 调用边（{}）", s.call_edges.len()));
    for e in &s.call_edges {
        out.push(format!("- {} → {}（{}）", e.from, e.to, e.loc));
    }
    out.join("\n")
}

/// 重新提取节点引用的源码骨架并写派生文件（框架 §5.7 refresh_code_map）。
/// 节点 frontmatter `code_map: <相对路径>`（相对工作区根，文件或目录）。
pub fn refresh_code_map(root: &Path, node_id: &str) -> Result<Skeleton, String> {
    let node_file = root.join(".chain").join("nodes").join(format!("{node_id}.md"));
    if !node_file.exists() {
        return Err(format!("节点 {node_id} 不存在"));
    }
    let raw =
        std::fs::read_to_string(&node_file).map_err(|e| format!("读节点失败：{e}"))?;
    let (fm, _body) = crate::scanner::frontmatter::parse(&raw)
        .map_err(|e| format!("解析 frontmatter 失败：{e}"))?;
    let code_map = fm
        .get(serde_yaml::Value::String("code_map".into()))
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            format!(
                "节点 {node_id} 无 code_map frontmatter（挂载方式：`code_map: <源码相对路径>`，正文只放一句概述）"
            )
        })?;
    let src = root.join(code_map);
    let mut sk = extract_skeleton(&src, node_id, "rust")?;
    // refresh = 重新提取 → 骨架新鲜；清除 watcher 留下的 stale 标记（T14）
    sk.stale = false;
    let _ = std::fs::remove_file(stale_marker(root, node_id));
    let md = skeleton_to_markdown(&sk);
    std::fs::create_dir_all(code_map_dir(root)).map_err(|e| format!("创建 code_map/ 失败：{e}"))?;
    atomic_write(&skeleton_path(root, node_id), &md)?;
    Ok(sk)
}

/// watcher 联动（T14）：代码目录变化 → 对应骨架标 stale（AI 进场调 refresh_code_map 兜底）
pub fn mark_stale(root: &Path, node_id: &str) -> Result<(), String> {
    std::fs::create_dir_all(code_map_dir(root)).map_err(|e| format!("创建 code_map/ 失败：{e}"))?;
    std::fs::write(stale_marker(root, node_id), b"").map_err(|e| format!("写 stale 标记失败：{e}"))?;
    Ok(())
}

/// 当前陈旧状态（stale 标记文件存在即 true；GUI/CLI 用）
pub fn is_stale(root: &Path, node_id: &str) -> bool {
    stale_marker(root, node_id).exists()
}

/// 只读：骨架 markdown 原文（GUI Mermaid/接口面板用；无则 None）。
/// stale 以标记文件为准——盘上 md 未刷新时按实时状态修正 stale 行。
pub fn read_skeleton_md(root: &Path, node_id: &str) -> Option<String> {
    let md = std::fs::read_to_string(skeleton_path(root, node_id)).ok()?;
    if stale_marker(root, node_id).exists() {
        Some(md.replacen("stale: false", "stale: true", 1))
    } else {
        Some(md)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    const SAMPLE: &str = r#"//! 示例模块
use std::fmt;

pub fn compute(a: i32, b: i32) -> i32 {
    helper(a) + b
}

fn helper(x: i32) -> i32 {
    x * 2
}

pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
    pub fn norm(&self) -> f64 {
        let s = helper(1);
        (self.x * self.x + self.y * self.y + s as f64).sqrt()
    }
}

pub trait Shape {
    fn area(&self) -> f64;
}

pub enum Color {
    Red,
    Blue,
}
"#;

    fn extract_sample() -> Skeleton {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("lib.rs"), SAMPLE).unwrap();
        extract_skeleton(&tmp.path().join("lib.rs"), "n1", "rust").unwrap()
    }

    #[test]
    fn exports_pub_api_only() {
        let s = extract_sample();
        let fns: Vec<&ExportSig> = s.exports.iter().filter(|e| e.kind == "fn").collect();
        // compute 是 pub；helper 私有不导出
        assert!(fns.iter().any(|e| e.name == "compute"), "{:?}", s.exports);
        assert!(!s.exports.iter().any(|e| e.name == "helper"), "私有 fn 不得导出");
        // struct/trait/enum/impl
        assert!(s.exports.iter().any(|e| e.kind == "struct" && e.name == "Point"));
        assert!(s.exports.iter().any(|e| e.kind == "trait" && e.name == "Shape"));
        assert!(s.exports.iter().any(|e| e.kind == "enum" && e.name == "Color"));
        assert!(
            s.exports.iter().any(|e| e.kind == "impl" && e.name.starts_with("impl Point")),
            "{:?}",
            s.exports
        );
        // impl 方法 pub fn 导出（new/norm），helper 不导出
        assert!(s.exports.iter().any(|e| e.name == "impl Point::new"));
        assert!(s.exports.iter().any(|e| e.name == "impl Point::norm"));
        // 签名与位置
        let compute = s.exports.iter().find(|e| e.name == "compute").unwrap();
        assert!(compute.signature.contains("pub fn compute"), "{}", compute.signature);
        assert_eq!(compute.loc, "lib.rs:4:1", "{}", compute.loc);
    }

    #[test]
    fn call_edges_resolved_within_file() {
        let s = extract_sample();
        // compute → helper；norm → helper
        assert!(
            s.call_edges
                .iter()
                .any(|e| e.from == "compute" && e.to == "helper"),
            "{:?}",
            s.call_edges
        );
        assert!(
            s.call_edges
                .iter()
                .any(|e| e.from == "impl Point::norm" && e.to == "helper"),
            "{:?}",
            s.call_edges
        );
        // 无自环、无未知目标
        assert!(s.call_edges.iter().all(|e| e.from != e.to));
        assert!(s.call_edges.iter().all(|e| e.to == "helper"));
    }

    #[test]
    fn mermaid_is_flowchart_and_deterministic() {
        let s = extract_sample();
        assert!(s.mermaid.starts_with("flowchart LR"), "{}", s.mermaid);
        assert!(s.mermaid.contains("compute"), "{}", s.mermaid);
        assert!(s.mermaid.contains("-->"), "{}", s.mermaid);
        // 确定性：同输入两次结果一致
        let s2 = extract_sample();
        assert_eq!(s.mermaid, s2.mermaid);
        assert_eq!(s.call_edges.len(), s2.call_edges.len());
    }

    #[test]
    fn markdown_shape() {
        let s = extract_sample();
        let md = skeleton_to_markdown(&s);
        assert!(md.contains("# 代码骨架：n1（rust）"));
        assert!(md.contains("## 导出接口"));
        assert!(md.contains("```mermaid"), "{md}");
        assert!(md.contains("## 调用边"));
        assert!(md.contains("compute → helper"), "{md}");
    }

    #[test]
    fn refresh_and_stale_marker_roundtrip() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".chain").join("nodes")).unwrap();
        fs::write(tmp.path().join(".chain").join(".mode"), "dev").unwrap();
        fs::write(tmp.path().join("lib.rs"), SAMPLE).unwrap();
        fs::write(
            tmp.path().join(".chain/nodes/n1.md"),
            "---\nid: n1\ntype: note\ntitle: 代码节点\nparent: null\nstatus: none\ncreated: 2026-09-01T10:00:00+08:00\nupdated: 2026-09-01T10:00:00+08:00\nrevision: 1\ntags: []\ncode_map: lib.rs\n---\n\n# 代码节点\n\n本库公开接口骨架。\n",
        )
        .unwrap();

        let s = refresh_code_map(tmp.path(), "n1").unwrap();
        assert!(!s.stale);
        assert_eq!(s.node_id, "n1");
        let md_path = tmp.path().join(".chain/code_map/n1.md");
        assert!(md_path.exists(), "骨架派生文件应落盘");
        let md = fs::read_to_string(&md_path).unwrap();
        assert!(md.contains("pub fn compute"), "{md}");

        // watcher 联动：mark_stale → 标记可见 → refresh 后清除
        mark_stale(tmp.path(), "n1").unwrap();
        assert!(stale_marker(tmp.path(), "n1").exists());
        assert!(is_stale(tmp.path(), "n1"));
        // 未刷新时 read_skeleton_md 应反映实时 stale 状态
        let live = read_skeleton_md(tmp.path(), "n1").unwrap();
        assert!(live.contains("stale: true"), "{live}");
        let s2 = refresh_code_map(tmp.path(), "n1").unwrap();
        assert!(!s2.stale, "refresh 后骨架新鲜");
        assert!(!stale_marker(tmp.path(), "n1").exists());
        assert!(!is_stale(tmp.path(), "n1"));
        // read_skeleton_md 只读
        let fresh = read_skeleton_md(tmp.path(), "n1").unwrap();
        assert!(fresh.contains("stale: false"), "{fresh}");
    }

    #[test]
    fn refresh_errors_without_code_map_frontmatter() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".chain").join("nodes")).unwrap();
        fs::write(tmp.path().join(".chain").join(".mode"), "dev").unwrap();
        fs::write(
            tmp.path().join(".chain/nodes/n1.md"),
            "---\nid: n1\ntype: note\ntitle: 无挂载\nparent: null\nstatus: none\ncreated: 2026-09-01T10:00:00+08:00\nupdated: 2026-09-01T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# 无挂载\n",
        )
        .unwrap();
        let err = refresh_code_map(tmp.path(), "n1").unwrap_err();
        assert!(err.contains("code_map"), "{err}");
        // 不支持的试点语言
        let err = extract_skeleton(Path::new("x.rs"), "n", "python").unwrap_err();
        assert!(err.contains("仅支持 rust"), "{err}");
    }
}
