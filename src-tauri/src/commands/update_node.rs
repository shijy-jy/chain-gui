use std::path::Path;
use tauri::command;
use crate::model::ScanMode;
use crate::model::chain::ChainSnapshot;
use crate::model::UpdateFields;
use crate::scanner;

/// 修改指定节点的字段并写回磁盘。
/// v2.0：mode 区分——分析模式严格（body 不能为空）；开发模式宽松（无内容要求，
/// 连 frontmatter 都没有的 .md 也能改：先补一个最小 frontmatter 再应用字段）。
#[command]
pub fn update_node(
    dir: String,
    node_id: String,
    fields: UpdateFields,
    mode: Option<String>,
) -> Result<ChainSnapshot, String> {
    let scan_mode = mode.as_deref().map(ScanMode::from_str).unwrap_or(ScanMode::Analysis);
    let dir = Path::new(&dir);
    let node_path = dir
        .join(".chain")
        .join("nodes")
        .join(format!("{}.md", node_id));

    if !node_path.exists() {
        return Err(format!("节点文件不存在：{}", node_path.display()));
    }

    // body 前置校验：分析模式下空 body 直接拒绝（在读文件之前失败，错误来源直观）
    if !scan_mode.is_dev() {
        if let Some(b) = &fields.body {
            if b.trim().is_empty() {
                return Err("body 不能为空".into());
            }
        }
    }

    // 1. 读原文件
    let raw = std::fs::read_to_string(&node_path)
        .map_err(|e| format!("读取失败：{}", e))?;

    // 2. 解析 frontmatter（开发模式：无 frontmatter 时兜底为最小 frontmatter）
    let (mut fm, body) = match scanner::frontmatter::parse(&raw) {
        Ok(result) => result,
        Err(_) if scan_mode.is_dev() => {
            use serde_yaml::Value as YamlValue;
            let now = scanner::frontmatter::now_iso8601();
            let mut m = serde_yaml::Mapping::new();
            m.insert(YamlValue::String("id".into()), YamlValue::String(node_id.clone()));
            m.insert(YamlValue::String("type".into()), YamlValue::String("note".into()));
            m.insert(YamlValue::String("status".into()), YamlValue::String("none".into()));
            m.insert(YamlValue::String("title".into()), YamlValue::String(node_id.clone()));
            m.insert(YamlValue::String("created".into()), YamlValue::String(now.clone()));
            m.insert(YamlValue::String("updated".into()), YamlValue::String(now));
            m.insert(YamlValue::String("revision".into()), YamlValue::Number(1u64.into()));
            m.insert(YamlValue::String("parent".into()), YamlValue::Null);
            (m, raw.clone())
        }
        Err(e) => return Err(format!("解析 frontmatter 失败：{}", e)),
    };

    // 3. 应用 fields
    crate::model::node::apply_update(&mut fm, &fields)
        .map_err(|e| format!("应用更新失败：{}", e))?;

    // 4. body 替换
    let new_body = match &fields.body {
        Some(b) => b.clone(),
        None => body,
    };

    // 5. 写回文件
    let new_content = crate::scanner::frontmatter::serialize(&fm, &new_body)
        .map_err(|e| format!("序列化失败：{}", e))?;
    std::fs::write(&node_path, new_content).map_err(|e| format!("写回失败：{}", e))?;

    // 6. 按当前模式重扫整个 chain，返回新 snapshot
    scanner::walker::scan_chain_dir_mode(dir, scan_mode).map_err(|e| format!("重扫失败：{}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{UpdateFields, node::NodeStatus};
    use std::fs;
    use tempfile::TempDir;

    fn setup_test_chain() -> TempDir {
        let tmp = TempDir::new().unwrap();
        let nodes_dir = tmp.path().join(".chain").join("nodes");
        fs::create_dir_all(&nodes_dir).unwrap();

        fs::write(
            nodes_dir.join("g-001.md"),
            "---\nid: g-001\ntype: goal\ntitle: 顶层目标\nparent: null\nstatus: pending\ncreated: 2026-08-13T10:00:00+08:00\nupdated: 2026-08-13T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# 顶层目标\n",
        ).unwrap();

        fs::write(
            nodes_dir.join("d-001.md"),
            "---\nid: d-001\ntype: design\ntitle: 设计1\nparent: g-001\nstatus: in_progress\ncreated: 2026-08-13T10:00:00+08:00\nupdated: 2026-08-13T10:00:00+08:00\nrevision: 1\ntags: [old]\n---\n\n# 设计1\n",
        ).unwrap();

        tmp
    }

    #[test]
    fn test_update_title() {
        let tmp = setup_test_chain();
        let dir = tmp.path().to_str().unwrap().to_string();

        let fields = UpdateFields {
            title: Some("新的目标标题".into()),
            status: None,
            body: None,
            tags: None,
            evidence: None,
            parent: None,
        };
        let snap = update_node(dir, "g-001".into(), fields, None).unwrap();

        let g = snap.nodes.iter().find(|n| n.id == "g-001").unwrap();
        assert_eq!(g.title, "新的目标标题");
        assert_eq!(g.revision, 2);

        let raw = fs::read_to_string(tmp.path().join(".chain/nodes/g-001.md")).unwrap();
        assert!(raw.contains("title: 新的目标标题"));
        assert!(raw.contains("revision: 2"));
    }

    #[test]
    fn test_update_status() {
        let tmp = setup_test_chain();
        let dir = tmp.path().to_str().unwrap().to_string();

        let fields = UpdateFields {
            title: None,
            status: Some(NodeStatus::InProgress),
            body: None,
            tags: None,
            evidence: None,
            parent: None,
        };
        let snap = update_node(dir, "g-001".into(), fields, None).unwrap();

        let g = snap.nodes.iter().find(|n| n.id == "g-001").unwrap();
        assert_eq!(g.status, NodeStatus::InProgress);

        let raw = fs::read_to_string(tmp.path().join(".chain/nodes/g-001.md")).unwrap();
        assert!(raw.contains("status: in_progress"));
    }

    #[test]
    fn test_update_tags() {
        let tmp = setup_test_chain();
        let dir = tmp.path().to_str().unwrap().to_string();

        let fields = UpdateFields {
            title: None,
            status: None,
            body: None,
            tags: Some(vec!["new1".into(), "new2".into()]),
            evidence: None,
            parent: None,
        };
        let snap = update_node(dir, "d-001".into(), fields, None).unwrap();

        let d = snap.nodes.iter().find(|n| n.id == "d-001").unwrap();
        assert_eq!(d.tags, vec!["new1", "new2"]);

        let raw = fs::read_to_string(tmp.path().join(".chain/nodes/d-001.md")).unwrap();
        assert!(raw.contains("new1"));
        assert!(raw.contains("new2"));
    }

    #[test]
    fn test_update_node_not_found() {
        let tmp = setup_test_chain();
        let dir = tmp.path().to_str().unwrap().to_string();

        let fields = UpdateFields {
            title: Some("不存在".into()),
            status: None,
            body: None,
            tags: None,
            evidence: None,
            parent: None,
        };
        let result = update_node(dir, "nonexistent".into(), fields, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("节点文件不存在"));
    }

    #[test]
    fn test_update_body_empty() {
        let tmp = setup_test_chain();
        let dir = tmp.path().to_str().unwrap().to_string();

        let fields = UpdateFields {
            title: None,
            status: None,
            body: Some("".into()),
            tags: None,
            evidence: None,
            parent: None,
        };
        let result = update_node(dir, "g-001".into(), fields, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("body 不能为空"));
    }

    #[test]
    fn test_update_evidence() {
        let tmp = setup_test_chain();
        let dir = tmp.path().to_str().unwrap().to_string();

        let fields = UpdateFields {
            title: None,
            status: None,
            body: None,
            tags: None,
            evidence: Some(vec!["artifacts/d-001/架构图.png".into()]),
            parent: None,
        };
        let snap = update_node(dir, "d-001".into(), fields, None).unwrap();

        let d = snap.nodes.iter().find(|n| n.id == "d-001").unwrap();
        assert_eq!(d.evidence, vec!["artifacts/d-001/架构图.png"]);

        let raw = fs::read_to_string(tmp.path().join(".chain/nodes/d-001.md")).unwrap();
        assert!(raw.contains("evidence:"));
        assert!(raw.contains("架构图.png"));
    }

    #[test]
    fn test_update_writes_valid_rfc3339_updated() {
        // 防止 now_iso8601 回归成垃圾字符串（历史 bug：曾输出 "1970-01-01T00:00:00+00:00 +1723511234s"）
        let tmp = setup_test_chain();
        let dir = tmp.path().to_str().unwrap().to_string();

        let fields = UpdateFields {
            title: Some("触发 updated 更新".into()),
            status: None,
            body: None,
            tags: None,
            evidence: None,
            parent: None,
        };
        update_node(dir, "g-001".into(), fields, None).unwrap();

        let raw = fs::read_to_string(tmp.path().join(".chain/nodes/g-001.md")).unwrap();
        let updated_line = raw
            .lines()
            .find(|l| l.trim_start().starts_with("updated:"))
            .expect("写回的文件必须有 updated 字段");
        let ts = updated_line
            .split_once(':')
            .unwrap()
            .1
            .trim()
            .trim_matches('"')
            .trim_matches('\'');
        // RFC3339（UTC+8）：YYYY-MM-DDTHH:MM:SS+08:00，25 字符
        assert_eq!(ts.len(), 25, "updated 必须是 RFC3339 格式，实际：{}", ts);
        assert_eq!(&ts[10..11], "T");
        assert!(ts.ends_with("+08:00"), "updated 必须以 +08:00 结尾，实际：{}", ts);
        let year: i32 = ts[0..4].parse().expect("年份必须是数字");
        assert!(year >= 2026 && year < 3000, "年份不合理：{}", year);
    }
}
