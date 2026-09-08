//! consolidate 蒸馏核心（框架 §5.6 / T9 / ADR 0009）：
//! - 聚类：BFS 连通分量（无向，size ≥ 2），targets 过滤，k = 簇数上限
//! - 产物：骨架节点（derived:true + 标题 [蒸馏]）+ 正文逐条来源引用——**骨架而非全文摘要**
//!   （无 LLM：模板化骨架 = 成员一句话 + 来源 id 引用，可追溯防幻觉）
//! - 人审摘帽 = 删除 derived 标记后转普通节点（GUI/编辑器）
//! 纯函数层：不落盘、不依赖 Workspace（写入由 ops::consolidate 完成）。

use crate::model::chain::ChainSnapshot;
use crate::model::node::Node;

pub struct Cluster {
    /// 簇成员节点 id（升序，跨平台确定）
    pub members: Vec<String>,
    /// 来源节点 id（= 成员；逐条引用的锚点）
    pub sources: Vec<String>,
    /// 骨架摘要全文（title 与 body 的合成物：来源行 + 成员一句话清单）
    pub summary: String,
    /// 骨架节点标题（[蒸馏] 前缀由调用方加；此处为去前缀的裸标题）
    pub title: String,
}

pub struct ConsolidatePlan {
    pub clusters: Vec<Cluster>,
}

/// 归一化标题（大小写/空白不敏感，供最长公共前缀计算）
fn normalize_title(t: &str) -> String {
    t.chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_lowercase()
}

/// 骨架标题：成员标题（id 升序）的最长公共前缀。
/// 前缀长度按归一化标题（大小写/空白不敏感）计算，取字时从首个原始标题跳过空白截取——
/// 保留原始大小写（如 Golden A / Golden B → "Golden"）；长度 < 2 → 取首成员标题。
fn skeleton_title(members: &[&Node]) -> String {
    let titles: Vec<String> = members.iter().map(|n| normalize_title(&n.title)).collect();
    let first = &titles[0];
    let mut end = first.chars().count();
    for t in &titles[1..] {
        let common = first
            .chars()
            .zip(t.chars())
            .take_while(|(a, b)| a == b)
            .count();
        end = end.min(common);
        if end == 0 {
            break;
        }
    }
    if end < 2 {
        return members[0].title.trim().to_string();
    }
    let prefix: String = members[0]
        .title
        .chars()
        .filter(|c| !c.is_whitespace())
        .take(end)
        .collect();
    prefix.trim().to_string()
}

/// 成员一句话：正文首个非标题非空行，截 120 字符；无内容行回落标题
fn one_liner(n: &Node) -> String {
    let line = n
        .body
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .next()
        .unwrap_or("");
    let text = if line.is_empty() { n.title.as_str() } else { line };
    crate::scanner::frontmatter::truncate_utf8(text, 120).to_string()
}

/// 骨架正文：来源引用行 + 成员一句话清单（逐条来源引用，可追溯防幻觉）
fn skeleton_body(members: &[&Node]) -> String {
    let mut lines = Vec::new();
    let src: Vec<String> = members
        .iter()
        .map(|n| format!("{}「{}」", n.id, n.title))
        .collect();
    lines.push(format!(
        "> 来源：{}（共 {} 个节点蒸馏，逐条可追溯）",
        src.join("、"),
        members.len()
    ));
    lines.push(String::new());
    lines.push("## 骨架".to_string());
    for n in members {
        lines.push(format!("- {}：{}", n.title, one_liner(n)));
    }
    lines.join("\n")
}

/// BFS 连通分量聚类（无向；活跃节点）；targets 过滤；k = 簇数上限（默认 8）。
/// 簇成员/簇序均按 id 升序（跨平台确定性）。
pub fn build_plan(
    snap: &ChainSnapshot,
    targets: Option<&[String]>,
    k: usize,
) -> ConsolidatePlan {
    use std::collections::{BTreeMap, BTreeSet, VecDeque};
    // 邻接表（BTreeMap 保序）
    let mut adj: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for e in &snap.edges {
        adj.entry(e.parent.as_str())
            .or_default()
            .insert(e.child.as_str());
        adj.entry(e.child.as_str())
            .or_default()
            .insert(e.parent.as_str());
    }
    let by_id: BTreeMap<&str, &Node> = snap.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    // BFS 分量（节点按 id 升序起手 → 分量发现顺序确定）
    let mut visited: BTreeSet<&str> = BTreeSet::new();
    let mut components: Vec<Vec<&str>> = Vec::new();
    for start in by_id.keys() {
        if visited.contains(*start) {
            continue;
        }
        let mut comp = Vec::new();
        let mut queue = VecDeque::from([*start]);
        visited.insert(*start);
        while let Some(cur) = queue.pop_front() {
            comp.push(cur);
            if let Some(neis) = adj.get(cur) {
                for nei in neis {
                    if visited.insert(*nei) {
                        queue.push_back(*nei);
                    }
                }
            }
        }
        components.push(comp);
    }

    let k = k.max(1);
    let mut clusters: Vec<Cluster> = Vec::new();
    for comp in components {
        // 单节点簇不可蒸馏（无共现关系）
        if comp.len() < 2 {
            continue;
        }
        let mut members: Vec<&str> = comp;
        if let Some(t) = targets {
            // targets 语义：只蒸馏指定节点参与的簇，成员截取为 targets ∩ 簇
            let wanted: BTreeSet<&str> = t.iter().map(|s| s.as_str()).collect();
            if !members.iter().any(|m| wanted.contains(*m)) {
                continue;
            }
            members.retain(|m| wanted.contains(*m));
            if members.len() < 2 {
                continue;
            }
        }
        members.sort_unstable(); // 已按 BTreeSet 序，此处兜底
        let nodes: Vec<&Node> = members
            .iter()
            .filter_map(|id| by_id.get(*id).copied())
            .collect();
        let members_owned: Vec<String> = members.iter().map(|s| s.to_string()).collect();
        clusters.push(Cluster {
            sources: members_owned.clone(),
            members: members_owned,
            title: skeleton_title(&nodes),
            summary: skeleton_body(&nodes),
        });
        if clusters.len() >= k {
            break;
        }
    }
    ConsolidatePlan { clusters }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup() -> (TempDir, ChainSnapshot) {
        let tmp = TempDir::new().unwrap();
        let nodes_dir = tmp.path().join(".chain").join("nodes");
        fs::create_dir_all(&nodes_dir).unwrap();
        fs::write(tmp.path().join(".chain").join(".mode"), "dev").unwrap();
        let node = |id: &str, title: &str, parent: &str, body: &str| {
            format!(
                "---\nid: {id}\ntype: note\ntitle: {title}\nparent: {parent}\nstatus: none\ncreated: 2026-09-01T10:00:00+08:00\nupdated: 2026-09-01T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# {title}\n\n{body}\n"
            )
        };
        fs::write(
            nodes_dir.join("a.md"),
            node("a", "力导向布局", "null", "斥力引力模型"),
        )
        .unwrap();
        fs::write(
            nodes_dir.join("b.md"),
            node("b", "力导向布局调优", "a", "稳定化参数"),
        )
        .unwrap();
        fs::write(nodes_dir.join("c.md"), node("c", "贝叶斯推理", "a", "后验更新")).unwrap();
        fs::write(nodes_dir.join("d.md"), node("d", "孤岛", "null", "无链接")).unwrap();
        let snap = crate::scanner::walker::scan_chain_dir_mode(tmp.path(), crate::model::ScanMode::Dev)
            .unwrap();
        (tmp, snap)
    }

    #[test]
    fn plan_clusters_connected_components_only() {
        let (_tmp, snap) = setup();
        let plan = build_plan(&snap, None, 8);
        assert_eq!(plan.clusters.len(), 1, "a-b-c 一个分量；孤岛 d 单节点不可蒸馏");
        let c = &plan.clusters[0];
        assert_eq!(c.members, vec!["a", "b", "c"]);
        assert_eq!(c.sources, c.members);
        assert!(c.summary.contains("来源：a「力导向布局」"), "{}", c.summary);
        assert!(c.summary.contains("b「力导向布局调优」"));
        assert!(c.summary.contains("斥力引力模型"), "成员一句话应含正文首行");
        // 标题 = 最长公共前缀（归一化）：力导向布局 与 力导向布局调优 与 贝叶斯推理 → LCP 空 → 首成员标题
        assert_eq!(c.title, "力导向布局");
    }

    #[test]
    fn plan_lcp_title_and_targets_filter() {
        let (_tmp, snap) = setup();
        // targets 只取 a/b → 成员截取后仍 ≥2
        let plan = build_plan(&snap, Some(&["a".to_string(), "b".to_string()]), 8);
        assert_eq!(plan.clusters.len(), 1);
        assert_eq!(plan.clusters[0].members, vec!["a", "b"]);
        // targets 只取 c → 截取后 1 个 → 跳过
        let plan2 = build_plan(&snap, Some(&["c".to_string()]), 8);
        assert!(plan2.clusters.is_empty());
    }

    #[test]
    fn plan_lcp_title_common_prefix() {
        // Golden A / Golden B → LCP "Golden"
        let tmp = TempDir::new().unwrap();
        let nodes_dir = tmp.path().join(".chain").join("nodes");
        fs::create_dir_all(&nodes_dir).unwrap();
        fs::write(tmp.path().join(".chain").join(".mode"), "dev").unwrap();
        for (id, title) in [("node-1", "Golden A"), ("node-2", "Golden B")] {
            fs::write(
                nodes_dir.join(format!("{id}.md")),
                format!(
                    "---\nid: {id}\ntype: note\ntitle: {title}\nparent: {}\nstatus: none\ncreated: 2026-09-01T10:00:00+08:00\nupdated: 2026-09-01T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# {title}\n\nbody {id}\n",
                    if id == "node-1" { "null" } else { "node-1" }
                ),
            )
            .unwrap();
        }
        let snap = crate::scanner::walker::scan_chain_dir_mode(tmp.path(), crate::model::ScanMode::Dev)
            .unwrap();
        let plan = build_plan(&snap, None, 8);
        assert_eq!(plan.clusters[0].title, "Golden", "LCP 应得 Golden");
    }

    #[test]
    fn plan_empty_graph_no_clusters() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".chain").join("nodes")).unwrap();
        fs::write(tmp.path().join(".chain").join(".mode"), "dev").unwrap();
        let snap = crate::scanner::walker::scan_chain_dir_mode(tmp.path(), crate::model::ScanMode::Dev)
            .unwrap();
        assert!(build_plan(&snap, None, 8).clusters.is_empty());
    }
}
