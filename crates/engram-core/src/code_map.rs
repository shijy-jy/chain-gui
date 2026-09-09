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

/// 语言矩阵（v2.16 扩展：rust / csharp / cpp；hlsl·glsl·cuda 走 C 系语法同一解析器）
fn ts_language(lang: &str) -> Result<tree_sitter::Language, String> {
    match lang {
        "rust" => Ok(tree_sitter_rust::LANGUAGE.into()),
        "csharp" => Ok(tree_sitter_c_sharp::LANGUAGE.into()),
        "cpp" | "hlsl" | "glsl" | "cuda" => Ok(tree_sitter_cpp::LANGUAGE.into()),
        other => Err(format!(
            "不支持的语言「{other}」（支持 rust / csharp / cpp / hlsl / glsl / cuda）"
        )),
    }
}

/// 按源码路径自动判语言：有已知代码扩展名 → 直接按扩展名判（路径可尚未创建）；
/// 目录 → 浅层统计扩展名占比；.cs 与 shader 族并存 → "unity"（双解析器一次提取）
pub fn detect_lang(path: &Path) -> String {
    let ext_lang = |ext: &str| -> &'static str {
        match ext.trim_start_matches('.').to_ascii_lowercase().as_str() {
            "cs" => "csharp",
            "cpp" | "cc" | "cxx" | "hpp" | "hh" | "h" | "cu" | "inl" | "hlsl" | "shader"
            | "glsl" | "cginc" | "compute" => "cpp",
            "rs" => "rust",
            _ => "rust",
        }
    };
    let is_shader_ext = |ext: &str| -> bool {
        matches!(
            ext.trim_start_matches('.').to_ascii_lowercase().as_str(),
            "hlsl" | "shader" | "glsl" | "cginc" | "compute"
        )
    };
    if let Some(e) = path.extension().and_then(|e| e.to_str()) {
        if path.is_file() || !path.exists() {
            return ext_lang(e).to_string();
        }
    }
    let mut cs = 0usize;
    let mut cpp = 0usize;
    let mut shader = 0usize;
    if let Ok(rd) = std::fs::read_dir(path) {
        for entry in rd.flatten().take(300) {
            let p = entry.path();
            if p.is_file() {
                if let Some(e) = p.extension().and_then(|e| e.to_str()) {
                    match ext_lang(e) {
                        "csharp" => cs += 1,
                        "cpp" => {
                            if is_shader_ext(e) {
                                shader += 1;
                            } else {
                                cpp += 1;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    // 顶层没有代码文件（代码在子目录深处）→ 有界深扫（深度 ≤5、至多 600 文件）
    if cs == 0 && cpp == 0 && shader == 0 {
        let mut seen = 0usize;
        for entry in walkdir::WalkDir::new(path)
            .max_depth(5)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let p = entry.path();
            if !p.is_file() {
                continue;
            }
            if let Some(e) = p.extension().and_then(|e| e.to_str()) {
                match ext_lang(e) {
                    "csharp" => cs += 1,
                    "cpp" => {
                        if is_shader_ext(e) {
                            shader += 1;
                        } else {
                            cpp += 1;
                        }
                    }
                    _ => {}
                }
            }
            seen += 1;
            if seen >= 600 {
                break;
            }
        }
    }
    // Unity 混合目录：.cs 与 shader 族并存 → unity 双解析器
    if cs > 0 && shader > 0 {
        return "unity".to_string();
    }
    if cs >= cpp && cs >= shader && cs > 0 {
        "csharp".to_string()
    } else if cpp > 0 {
        "cpp".to_string()
    } else if shader > 0 {
        "cpp".to_string()
    } else {
        "rust".to_string()
    }
}

/// 各语言参与提取的扩展名集合（目录递归时过滤；unity = .cs + shader 族双解析器）
fn lang_exts(lang: &str) -> &'static [&'static str] {
    match lang {
        "csharp" => &["cs"],
        "cpp" | "hlsl" | "glsl" | "cuda" => &[
            "cpp", "cc", "cxx", "hpp", "hh", "h", "cu", "inl", "hlsl", "shader", "glsl", "cginc",
            "compute",
        ],
        "unity" => &["cs", "hlsl", "shader", "glsl", "cginc", "compute"],
        _ => &["rs"],
    }
}

fn node_text<'a>(node: tree_sitter::Node, source: &'a [u8]) -> &'a str {
    node.utf8_text(source).unwrap_or("")
}

/// 名称（identifier 即名本身；其余取 name 字段；scoped/qualified 取末段；field_expression 取 field 名）
fn name_of(node: tree_sitter::Node, source: &[u8]) -> Option<String> {
    if matches!(
        node.kind(),
        "identifier" | "type_identifier" | "field_identifier" | "namespace_identifier"
    ) {
        return Some(node_text(node, source).trim().to_string());
    }
    if let Some(id) = node.child_by_field_name("name") {
        if matches!(
            id.kind(),
            "identifier" | "type_identifier" | "field_identifier" | "namespace_identifier"
        ) {
            return Some(node_text(id, source).trim().to_string());
        }
        // qualified_name（a.b）/ nested_identifier_specifier（a::b）→ 末段
        let mut last = None;
        let mut cursor = id.walk();
        for c in id.children(&mut cursor) {
            if matches!(
                c.kind(),
                "identifier" | "type_identifier" | "namespace_identifier"
            ) && c.is_named()
            {
                last = Some(node_text(c, source).trim().to_string());
            }
        }
        if last.is_some() {
            return last;
        }
    }
    // field_expression（x.foo 调用目标）→ field 字段名
    if node.kind() == "field_expression" {
        return node
            .child_by_field_name("field")
            .and_then(|f| name_of(f, source));
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

// ── C# 提取（v2.16，Unity 工程主语言）───────────────────────

fn extract_file_csharp(
    path: &str,
    source: &[u8],
    tree: &tree_sitter::Tree,
    exports: &mut Vec<ExportSig>,
    edges: &mut Vec<CallEdge>,
) {
    // 一遍：方法/类型简单名收集（调用边解析）
    let mut known: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    fn walk_names(
        node: tree_sitter::Node,
        source: &[u8],
        known: &mut std::collections::BTreeSet<String>,
    ) {
        if matches!(
            node.kind(),
            "method_declaration"
                | "constructor_declaration"
                | "class_declaration"
                | "struct_declaration"
                | "interface_declaration"
                | "enum_declaration"
        ) {
            if let Some(n) = name_of(node, source) {
                known.insert(n);
            }
        }
        let mut cursor = node.walk();
        for c in node.children(&mut cursor) {
            walk_names(c, source, known);
        }
    }
    walk_names(tree.root_node(), source, &mut known);

    // 修饰符可见性：public/internal/protected 才导出（fn item：可被嵌套 walk fn 调用）
    // 注意：c-sharp 每个修饰符是独立的 (modifier) 子节点，文本即关键字
    fn has_public_mod(node: tree_sitter::Node, source: &[u8]) -> bool {
        let mut cursor = node.walk();
        for c in node.children(&mut cursor) {
            if c.kind() == "modifier" {
                let t = node_text(c, source);
                if t == "public" || t == "internal" || t == "protected" {
                    return true;
                }
            }
        }
        false
    }

    fn walk(
        node: tree_sitter::Node,
        source: &[u8],
        path: &str,
        ns: &mut Vec<String>,
        fn_stack: &mut Vec<String>,
        exports: &mut Vec<ExportSig>,
        edges: &mut Vec<CallEdge>,
        known: &std::collections::BTreeSet<String>,
    ) {
        let kind = node.kind();
        let mut pushed = false;
        // 类型导出在「自身名入栈」之前：全名 = 外层 ns + 自身名（否则会重复拼出 Class::Class）
        if matches!(
            kind,
            "class_declaration" | "struct_declaration" | "interface_declaration" | "enum_declaration"
        ) {
            if let Some(n) = name_of(node, source) {
                let full = if ns.is_empty() {
                    n.clone()
                } else {
                    format!("{}::{}", ns.join("::"), n)
                };
                let sig = String::from_utf8_lossy(&source[node.start_byte()..node.end_byte()])
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                exports.push(ExportSig {
                    name: full,
                    kind: kind.trim_end_matches("_declaration").into(),
                    signature: crate::scanner::frontmatter::truncate_utf8(&sig, 300).to_string(),
                    loc: loc_str(path, node),
                });
            }
        }
        if kind == "namespace_declaration"
            || matches!(
                kind,
                "class_declaration" | "struct_declaration" | "interface_declaration"
            )
        {
            if let Some(n) = name_of(node, source) {
                ns.push(n);
                pushed = true;
            }
        }
        if matches!(
            kind,
            "method_declaration" | "constructor_declaration" | "property_declaration"
        ) {
            if has_public_mod(node, source) {
                if let Some(n) = name_of(node, source) {
                    let full = if ns.is_empty() {
                        n.clone()
                    } else {
                        format!("{}::{}", ns.join("::"), n)
                    };
                    let body = node.child_by_field_name("body");
                    let end = body.map(|b| b.start_byte()).unwrap_or(node.end_byte());
                    let sig = String::from_utf8_lossy(&source[node.start_byte()..end])
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ");
                    fn_stack.push(full.clone());
                    exports.push(ExportSig {
                        name: full,
                        kind: if kind == "property_declaration" { "property" } else { "fn" }.into(),
                        signature: crate::scanner::frontmatter::truncate_utf8(&sig, 300).to_string(),
                        loc: loc_str(path, node),
                    });
                }
            }
        }
        if kind == "invocation_expression" {
            if let Some(target) = node
                .child_by_field_name("function")
                .and_then(|f| name_of(f, source))
            {
                if let Some(caller) = fn_stack.last() {
                    if !target.is_empty() && known.contains(&target) && &target != caller {
                        edges.push(CallEdge {
                            from: caller.clone(),
                            to: target,
                            loc: loc_str(path, node),
                        });
                    }
                }
            }
        }
        let mut cursor = node.walk();
        for c in node.children(&mut cursor) {
            walk(c, source, path, ns, fn_stack, exports, edges, known);
        }
        if pushed {
            ns.pop();
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

// ── C++ 提取（v2.16：render_unified_oss/RESTRI 仓库；hlsl·glsl·cuda 同解析器）──

fn extract_file_cpp(
    path: &str,
    source: &[u8],
    tree: &tree_sitter::Tree,
    exports: &mut Vec<ExportSig>,
    edges: &mut Vec<CallEdge>,
) {
    // 函数定义名：function_definition → declarator(function_declarator) → declarator(identifier)
    fn func_name(node: tree_sitter::Node, source: &[u8]) -> Option<String> {
        let dec = node.child_by_field_name("declarator")?;
        if dec.kind() == "function_declarator" {
            return dec
                .child_by_field_name("declarator")
                .and_then(|id| name_of(id, source));
        }
        None
    }
    // 文件内 static 函数/匿名符号不导出（实现细节）
    fn is_static(node: tree_sitter::Node, source: &[u8]) -> bool {
        let mut cursor = node.walk();
        for c in node.children(&mut cursor) {
            if c.kind() == "storage_class_specifier" && node_text(c, source) == "static" {
                return true;
            }
        }
        false
    }

    let mut known: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    fn walk_names(
        node: tree_sitter::Node,
        source: &[u8],
        known: &mut std::collections::BTreeSet<String>,
    ) {
        if node.kind() == "function_definition" {
            if let Some(n) = func_name(node, source) {
                known.insert(n);
            }
        }
        if matches!(
            node.kind(),
            "class_specifier" | "struct_specifier" | "enum_specifier"
        ) {
            if let Some(n) = name_of(node, source) {
                known.insert(n);
            }
        }
        let mut cursor = node.walk();
        for c in node.children(&mut cursor) {
            walk_names(c, source, known);
        }
    }
    walk_names(tree.root_node(), source, &mut known);

    fn walk(
        node: tree_sitter::Node,
        source: &[u8],
        path: &str,
        ns: &mut Vec<String>,
        fn_stack: &mut Vec<String>,
        exports: &mut Vec<ExportSig>,
        edges: &mut Vec<CallEdge>,
        known: &std::collections::BTreeSet<String>,
    ) {
        let kind = node.kind();
        let mut pushed = false;
        // 类型导出在「自身名入栈」之前（避免 Class::Class 重复拼名）
        if matches!(
            kind,
            "class_specifier" | "struct_specifier" | "enum_specifier" | "union_specifier"
        ) {
            if let Some(n) = name_of(node, source) {
                let full = if ns.is_empty() {
                    n.clone()
                } else {
                    format!("{}::{}", ns.join("::"), n)
                };
                let sig = String::from_utf8_lossy(&source[node.start_byte()..node.end_byte()])
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                exports.push(ExportSig {
                    name: full,
                    kind: kind.trim_end_matches("_specifier").into(),
                    signature: crate::scanner::frontmatter::truncate_utf8(&sig, 300).to_string(),
                    loc: loc_str(path, node),
                });
            }
        }
        if kind == "namespace_definition"
            || matches!(kind, "class_specifier" | "struct_specifier")
        {
            if let Some(n) = name_of(node, source) {
                ns.push(n);
                pushed = true;
            }
        }
        if kind == "function_definition" && !is_static(node, source) {
            if let Some(n) = func_name(node, source) {
                let full = if ns.is_empty() {
                    n.clone()
                } else {
                    format!("{}::{}", ns.join("::"), n)
                };
                let body = node.child_by_field_name("body");
                let end = body.map(|b| b.start_byte()).unwrap_or(node.end_byte());
                let sig = String::from_utf8_lossy(&source[node.start_byte()..end])
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                fn_stack.push(full.clone());
                exports.push(ExportSig {
                    name: full,
                    kind: "fn".into(),
                    signature: crate::scanner::frontmatter::truncate_utf8(&sig, 300).to_string(),
                    loc: loc_str(path, node),
                });
            }
        }
        if kind == "call_expression" {
            if let Some(target) = node
                .child_by_field_name("function")
                .and_then(|f| name_of(f, source))
            {
                if let Some(caller) = fn_stack.last() {
                    if !target.is_empty() && known.contains(&target) && &target != caller {
                        edges.push(CallEdge {
                            from: caller.clone(),
                            to: target,
                            loc: loc_str(path, node),
                        });
                    }
                }
            }
        }
        let mut cursor = node.walk();
        for c in node.children(&mut cursor) {
            walk(c, source, path, ns, fn_stack, exports, edges, known);
        }
        if pushed {
            ns.pop();
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

/// 提取骨架（框架 §5.7）。src 可为单个源文件或目录（目录递归，扩展名按语言过滤）。
/// 语言（v2.16）：rust（T12 试点）/ csharp / cpp（hlsl·glsl·cuda 同解析器）。
pub fn extract_skeleton(src: &Path, node_id: &str, lang: &str) -> Result<Skeleton, String> {
    // 语言前置校验（unity 是 .cs+shader 双解析器组合，逐文件再选具体解析器）
    if lang != "unity" {
        ts_language(lang)?;
    }
    let exts = lang_exts(lang);
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
            let ok = p
                .extension()
                .and_then(|s| s.to_str())
                .map(|e| exts.contains(&e))
                .unwrap_or(false);
            if ok {
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
        return Err(format!(
            "源码目录无匹配文件（lang={lang}，扩展名 {:?}）：{}",
            exts,
            src.display()
        ));
    }
    for (rel, p) in files {
        let source = std::fs::read(p.clone())
            .map_err(|e| format!("读源码失败 {}：{e}", p.display()))?;
        // v2.16 unity 双解析器：按文件扩展名选（.cs → csharp；shader 族 → cpp）
        let file_lang = if lang == "unity" {
            let e = p.extension().and_then(|s| s.to_str()).unwrap_or("");
            if matches!(e, "hlsl" | "shader" | "glsl" | "cginc" | "compute") {
                "cpp"
            } else {
                "csharp"
            }
        } else {
            lang
        };
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&ts_language(file_lang)?)
            .map_err(|e| format!("tree-sitter 语言加载失败：{e}"))?;
        let tree = parser
            .parse(&source, None)
            .ok_or_else(|| format!("解析失败：{}", p.display()))?;
        match file_lang {
            "csharp" => extract_file_csharp(&rel, &source, &tree, &mut exports, &mut edges),
            "cpp" | "hlsl" | "glsl" | "cuda" => {
                extract_file_cpp(&rel, &source, &tree, &mut exports, &mut edges)
            }
            _ => extract_file(&rel, &source, &tree, &mut exports, &mut edges),
        }
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
        out.push(format!("```{}", s.language));
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

/// 解析 code_map 路径：相对工作区根，或 v2.16 跨盘挂载的绝对路径（Path::join 对绝对路径取自身）
fn resolve_code_map_path(root: &Path, code_map: &str) -> std::path::PathBuf {
    let p = Path::new(code_map);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        root.join(p)
    }
}

/// 重新提取节点引用的源码骨架并写派生文件（框架 §5.7 refresh_code_map）。
/// 节点 frontmatter `code_map: <相对路径或绝对路径>`（文件或目录）；语言按路径自动检测。
pub fn refresh_code_map(root: &Path, node_id: &str) -> Result<Skeleton, String> {
    refresh_code_map_lang(root, node_id, None)
}

/// 同 refresh_code_map，但显式指定语言（CLI --lang；None = 自动检测）
pub fn refresh_code_map_lang(
    root: &Path,
    node_id: &str,
    lang_override: Option<&str>,
) -> Result<Skeleton, String> {
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
    let src = resolve_code_map_path(root, code_map);
    let lang = lang_override
        .map(|l| l.to_string())
        .unwrap_or_else(|| detect_lang(&src));
    let mut sk = extract_skeleton(&src, node_id, &lang)?;
    // refresh = 重新提取 → 骨架新鲜；清除 watcher 留下的 stale 标记（T14）
    sk.stale = false;
    let _ = std::fs::remove_file(stale_marker(root, node_id));
    let md = skeleton_to_markdown(&sk);
    std::fs::create_dir_all(code_map_dir(root)).map_err(|e| format!("创建 code_map/ 失败：{e}"))?;
    atomic_write(&skeleton_path(root, node_id), &md)?;
    Ok(sk)
}

/// 挂载代码骨架（GUI「代码栏」/CLI 通用）：写节点 frontmatter `code_map: <相对或绝对路径>`，
/// 校验源码路径存在，经核心唯一写路径（revision+1）落盘后提取骨架。
/// 语义：骨架挂在**理论/概念节点**上（节点信息栏「代码」节），不鼓励独立骨架节点群。
/// v2.16：允许绝对路径（跨盘挂载，如 G 盘工作区 ← D 盘 Unity 工程）；语言自动检测。
pub fn attach_code_map(root: &Path, node_id: &str, rel: &str) -> Result<Skeleton, String> {
    let node_file = root.join(".chain").join("nodes").join(format!("{node_id}.md"));
    if !node_file.exists() {
        return Err(format!("节点 {node_id} 不存在"));
    }
    let rel = rel
        .trim()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string();
    if rel.is_empty() {
        return Err("code_map 路径不能为空".into());
    }
    let src = resolve_code_map_path(root, &rel);
    if !src.is_file() && !src.is_dir() {
        return Err(format!(
            "源码路径不存在（相对工作区根 {root} 或绝对路径）：{rel}",
            root = root.display()
        ));
    }
    let raw =
        std::fs::read_to_string(&node_file).map_err(|e| format!("读节点失败：{e}"))?;
    let (mut fm, body) = crate::ops::parse_lenient(&raw, node_id)?;
    fm.insert(
        serde_yaml::Value::String("code_map".into()),
        serde_yaml::Value::String(rel.clone()),
    );
    let fields = crate::model::UpdateFields {
        title: None,
        status: None,
        body: None,
        tags: None,
        evidence: None,
        parent: None,
        rel: None,
    };
    crate::model::node::apply_update(&mut fm, &fields).map_err(|e| format!("应用更新失败：{e}"))?;
    let content =
        crate::scanner::frontmatter::serialize(&fm, &body).map_err(|e| format!("序列化失败：{e}"))?;
    atomic_write(&node_file, &content)?;
    refresh_code_map(root, node_id)
}

/// 移除代码挂载（GUI「代码栏」移除按钮）：清 code_map 字段 + 删除骨架派生物（含 stale 标记）
pub fn detach_code_map(root: &Path, node_id: &str) -> Result<(), String> {
    let node_file = root.join(".chain").join("nodes").join(format!("{node_id}.md"));
    if !node_file.exists() {
        return Err(format!("节点 {node_id} 不存在"));
    }
    let raw =
        std::fs::read_to_string(&node_file).map_err(|e| format!("读节点失败：{e}"))?;
    let (mut fm, body) = crate::ops::parse_lenient(&raw, node_id)?;
    if fm
        .get(serde_yaml::Value::String("code_map".into()))
        .is_none()
    {
        return Ok(()); // 未挂载：幂等
    }
    fm.remove(serde_yaml::Value::String("code_map".into()));
    let fields = crate::model::UpdateFields {
        title: None,
        status: None,
        body: None,
        tags: None,
        evidence: None,
        parent: None,
        rel: None,
    };
    crate::model::node::apply_update(&mut fm, &fields).map_err(|e| format!("应用更新失败：{e}"))?;
    let content =
        crate::scanner::frontmatter::serialize(&fm, &body).map_err(|e| format!("序列化失败：{e}"))?;
    atomic_write(&node_file, &content)?;
    let _ = std::fs::remove_file(skeleton_path(root, node_id));
    let _ = std::fs::remove_file(stale_marker(root, node_id));
    Ok(())
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

/// 检索集成（指南 v10 附「检索语义」）：节点检索文本 = title + body + 代码骨架（如有）——
/// 模块名/函数名/签名随骨架进入召回与关键词检索（骨架即概念的可执行证据）。
/// 返回 (检索文本, 检索哈希)。哈希与文本绑定：骨架重建 → 哈希变化 → 召回按需重嵌。
/// 无 code_map 的节点沿用既有口径（title+body / 文件哈希），调用方自行保持旧约定。
pub fn node_retrieval_text(
    root: &Path,
    id: &str,
    title: &str,
    body: &str,
    has_code_map: bool,
) -> (String, String) {
    let mut text = format!("{title}\n{body}");
    if has_code_map {
        if let Some(md) = read_skeleton_md(root, id) {
            text.push('\n');
            text.push_str(&md);
        }
    }
    let hash = crate::index::content_hash(&text);
    (text, hash)
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
    fn attach_and_detach_code_map_roundtrip() {
        // 骨架挂概念节点：attach 写 frontmatter + 生成骨架；detach 清理 + 删除派生物
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".chain").join("nodes")).unwrap();
        fs::write(tmp.path().join(".chain").join(".mode"), "dev").unwrap();
        fs::write(tmp.path().join("lib.rs"), SAMPLE).unwrap();
        fs::write(
            tmp.path().join(".chain/nodes/n1.md"),
            "---\nid: n1\ntype: note\ntitle: 概念节点\nparent: null\nstatus: none\ncreated: 2026-09-01T10:00:00+08:00\nupdated: 2026-09-01T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# 概念节点\n\n一句概述。\n",
        )
        .unwrap();

        let sk = attach_code_map(tmp.path(), "n1", "lib.rs").unwrap();
        assert_eq!(sk.node_id, "n1");
        assert!(sk.exports.iter().any(|e| e.name == "compute"));
        let raw = fs::read_to_string(tmp.path().join(".chain/nodes/n1.md")).unwrap();
        assert!(raw.contains("code_map: lib.rs"), "frontmatter 应挂载：{raw}");
        assert!(raw.contains("revision: 2"), "走唯一写路径 revision+1");
        assert!(skeleton_path(tmp.path(), "n1").exists());

        // 路径不存在 → 拒绝
        let err = attach_code_map(tmp.path(), "n1", "ghost.rs").unwrap_err();
        assert!(err.contains("不存在"), "{err}");

        // detach：清字段 + 删派生物（幂等）
        detach_code_map(tmp.path(), "n1").unwrap();
        let raw = fs::read_to_string(tmp.path().join(".chain/nodes/n1.md")).unwrap();
        assert!(!raw.contains("code_map"), "detach 应清挂载：{raw}");
        assert!(!skeleton_path(tmp.path(), "n1").exists());
        detach_code_map(tmp.path(), "n1").unwrap(); // 幂等
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
        assert!(err.contains("不支持的语言"), "{err}");
    }

    #[test]
    fn csharp_extraction_exports_types_and_public_members() {
        const CS: &str = r#"
namespace WaterRender {
    public class OceanFFT {
        public void Init(int size) { Bake(); }
        private void Bake() { }
        public float Height { get; set; }
    }
    internal struct WaveConfig { public int N; }
}
"#;
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("Ocean.cs"), CS).unwrap();
        let s = extract_skeleton(&tmp.path().join("Ocean.cs"), "n1", "csharp").unwrap();
        assert!(
            s.exports
                .iter()
                .any(|e| e.kind == "class" && e.name == "WaterRender::OceanFFT"),
            "{:?}",
            s.exports
        );
        assert!(
            s.exports.iter().any(|e| e.kind == "fn" && e.name.ends_with("::Init")),
            "{:?}",
            s.exports
        );
        assert!(
            !s.exports.iter().any(|e| e.name.ends_with("::Bake")),
            "私有方法不得导出：{:?}",
            s.exports
        );
        assert!(
            s.exports
                .iter()
                .any(|e| e.kind == "property" && e.name.ends_with("::Height")),
            "{:?}",
            s.exports
        );
        assert!(
            s.exports
                .iter()
                .any(|e| e.kind == "struct" && e.name == "WaterRender::WaveConfig"),
            "{:?}",
            s.exports
        );
        assert!(
            s.call_edges.iter().any(|e| e.to == "Bake"),
            "Init→Bake 调用边：{:?}",
            s.call_edges
        );
    }

    #[test]
    fn cpp_extraction_exports_functions_and_classes() {
        const CPP: &str = r#"
namespace rndr {
class Camera { public: void update(float dt); };
struct Ray { float ox, oy; };
static int helper() { return 1; }
int render(Camera& cam) { helper(); return 1; }
}
"#;
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("main.cpp"), CPP).unwrap();
        let s = extract_skeleton(&tmp.path().join("main.cpp"), "n2", "cpp").unwrap();
        assert!(
            s.exports
                .iter()
                .any(|e| e.kind == "class" && e.name == "rndr::Camera"),
            "{:?}",
            s.exports
        );
        assert!(
            s.exports
                .iter()
                .any(|e| e.kind == "struct" && e.name == "rndr::Ray"),
            "{:?}",
            s.exports
        );
        assert!(
            s.exports
                .iter()
                .any(|e| e.kind == "fn" && e.name == "rndr::render"),
            "{:?}",
            s.exports
        );
        assert!(
            !s.exports.iter().any(|e| e.name.ends_with("helper")),
            "static 函数不得导出：{:?}",
            s.exports
        );
        assert!(
            s.call_edges.iter().any(|e| e.to == "helper"),
            "{:?}",
            s.call_edges
        );
    }

    #[test]
    fn detect_lang_by_extension_and_dir() {
        assert_eq!(detect_lang(Path::new("x.cs")), "csharp");
        assert_eq!(detect_lang(Path::new("y.hpp")), "cpp");
        assert_eq!(detect_lang(Path::new("z.hlsl")), "cpp");
        assert_eq!(detect_lang(Path::new("w.rs")), "rust");
        // cs 与 shader 族并存 → unity 双解析器（Unity 工程目录）
        let mixed = TempDir::new().unwrap();
        fs::write(mixed.path().join("a.cs"), "").unwrap();
        fs::write(mixed.path().join("b.shader"), "").unwrap();
        assert_eq!(detect_lang(mixed.path()), "unity", "cs+shader 混合判 unity");
        // 纯 cs 目录 → csharp
        let csonly = TempDir::new().unwrap();
        fs::write(csonly.path().join("a.cs"), "").unwrap();
        fs::write(csonly.path().join("b.cs"), "").unwrap();
        assert_eq!(detect_lang(csonly.path()), "csharp", "纯 cs 目录判 csharp");
    }
}
