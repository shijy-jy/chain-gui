use std::collections::HashSet;
use serde_yaml::Value as YamlValue;
use crate::model::node::{Node, NodeType};
use crate::model::validation::ValidationReport;

// ── Public API ──────────────────────────────────────────────

/// 字段级校验：逐字段检查 YAML frontmatter
pub fn validate_fields(filename: &str, fm: &serde_yaml::Mapping, errors: &mut Vec<String>) {
    // id
    let id = match get_str(fm, "id") {
        Some(s) => s,
        None => {
            errors.push(format!("[{}] id: 字段缺失", filename));
            return; // 没有 id 无法继续
        }
    };
    if !is_valid_id_format(&id) {
        errors.push(format!("[{}] id: 格式非法（应为 type-NNN，如 g-001）", filename));
    }
    let expected_id = filename.trim_end_matches(".md");
    if id != expected_id {
        errors.push(format!(
            "[{}] id: 与文件名不匹配（期望 {}，实际 {}）", filename, expected_id, id
        ));
    }

    // type
    match get_str(fm, "type") {
        Some(t) if matches!(t.as_str(), "goal" | "design" | "task" | "verification") => {}
        Some(t) => errors.push(format!("[{}] type: 非法枚举值 '{}'", filename, t)),
        None => errors.push(format!("[{}] type: 字段缺失", filename)),
    }

    // status
    match get_str(fm, "status") {
        Some(s) if matches!(s.as_str(), "pending" | "in_progress" | "success" | "failed" | "blocked") => {}
        Some(s) => errors.push(format!("[{}] status: 非法枚举值 '{}'", filename, s)),
        None => errors.push(format!("[{}] status: 字段缺失", filename)),
    }

    // title
    match get_str(fm, "title") {
        Some(t) if t.trim().is_empty() => {
            errors.push(format!("[{}] title: 不能为空", filename));
        }
        Some(_) => {}
        None => errors.push(format!("[{}] title: 字段缺失", filename)),
    }

    // created
    let created = match get_str(fm, "created") {
        Some(s) => {
            if !is_valid_rfc3339(&s) {
                errors.push(format!("[{}] created: 不符合 RFC3339 格式 '{}'", filename, s));
            }
            s
        }
        None => {
            errors.push(format!("[{}] created: 字段缺失", filename));
            String::new()
        }
    };

    // updated
    let updated = match get_str(fm, "updated") {
        Some(s) => {
            if !is_valid_rfc3339(&s) {
                errors.push(format!("[{}] updated: 不符合 RFC3339 格式 '{}'", filename, s));
            }
            s
        }
        None => {
            errors.push(format!("[{}] updated: 字段缺失", filename));
            String::new()
        }
    };

    // updated >= created
    if !created.is_empty() && !updated.is_empty() {
        if let (Some(c), Some(u)) = (parse_rfc3339_to_epoch(&created), parse_rfc3339_to_epoch(&updated)) {
            if u < c {
                errors.push(format!(
                    "[{}] updated: 早于 created（updated={}, created={}）", filename, updated, created
                ));
            }
        }
    }

    // revision
    match fm.get(&YamlValue::String("revision".into())) {
        Some(YamlValue::Number(n)) if n.as_u64().map(|v| v > 0).unwrap_or(false) => {}
        Some(YamlValue::Number(_)) => {
            errors.push(format!("[{}] revision: 必须为正整数", filename));
        }
        Some(_) => {
            errors.push(format!("[{}] revision: 必须为正整数", filename));
        }
        None => {
            errors.push(format!("[{}] revision: 字段缺失", filename));
        }
    }

    // tags
    match fm.get(&YamlValue::String("tags".into())) {
        Some(YamlValue::Sequence(_)) | Some(YamlValue::Null) | None => {}
        Some(_) => {
            errors.push(format!("[{}] tags: 必须为数组", filename));
        }
    }

    // evidence（可选字段）：填了必须是字符串数组
    match fm.get(&YamlValue::String("evidence".into())) {
        Some(YamlValue::Sequence(seq)) => {
            for (i, item) in seq.iter().enumerate() {
                if !matches!(item, YamlValue::String(_)) {
                    errors.push(format!(
                        "[{}] evidence: 第 {} 项必须为字符串（相对路径）", filename, i + 1
                    ));
                }
            }
        }
        Some(YamlValue::Null) | None => {}
        Some(_) => {
            errors.push(format!("[{}] evidence: 必须为数组（字符串路径列表）", filename));
        }
    }

    // parent
    match fm.get(&YamlValue::String("parent".into())) {
        Some(YamlValue::Null) | Some(YamlValue::String(_)) => {}
        None => {
            errors.push(format!("[{}] parent: 字段缺失", filename));
        }
        Some(_) => {
            errors.push(format!("[{}] parent: 必须为 null 或字符串", filename));
        }
    }
}

/// 结构级校验：id 唯一 / parent 悬空 / 环检测 / root 唯一 / parent 类型约束
pub fn validate_structure(
    nodes: &[(&str, &Node)],
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    let mut id_set: HashSet<&str> = HashSet::new();
    let mut root_count = 0;

    for (filename, node) in nodes {
        if !id_set.insert(node.id.as_str()) {
            errors.push(format!("[{}] id: 重复 id '{}'", filename, node.id));
        }
        if node.parent.is_none() {
            root_count += 1;
        }
    }

    // root 唯一
    if nodes.is_empty() {
        // 空目录不算错误，跳过结构校验
    } else if root_count == 0 {
        errors.push("结构: 缺少根节点（没有 parent 为 null 的节点）".into());
    } else if root_count > 1 {
        errors.push(format!("结构: 存在 {} 个根节点（应仅有 1 个）", root_count));
    }

    // parent 引用存在 + parent 类型约束
    let ids: HashSet<&str> = nodes.iter().map(|(_, n)| n.id.as_str()).collect();
    for (filename, node) in nodes {
        if let Some(parent_id) = &node.parent {
            if !ids.contains(parent_id.as_str()) {
                errors.push(format!("[{}] parent: 悬空引用 '{}'", filename, parent_id));
            }
        }

        // parent 类型约束（warning）
        // M10 起：子 goal 挂在失败节点下是标准用法，goal 有 parent 不再告警
        if !matches!(node.node_type, NodeType::Goal) && node.parent.is_none() {
            warnings.push(format!("[{}] parent: 非 goal 类型节点应有 parent", filename));
        }
    }

    // 环检测（DFS following parent links）
    let parent_map: std::collections::HashMap<&str, Option<&str>> = nodes
        .iter()
        .map(|(_, n)| (n.id.as_str(), n.parent.as_deref()))
        .collect();
    let mut reported_cycles: HashSet<Vec<String>> = HashSet::new();
    for (filename, node) in nodes {
        let mut visited = HashSet::new();
        let mut path = Vec::new();
        let mut current = node.id.as_str();
        loop {
            if visited.contains(current) {
                // 找到环：提取环中的节点
                let cycle_start = path.iter().position(|p: &String| p == current).unwrap();
                let mut cycle: Vec<String> = path[cycle_start..].to_vec();
                cycle.sort();
                if reported_cycles.insert(cycle) {
                    errors.push(format!(
                        "[{}] parent: 检测到环（包含节点 {}）", filename, current
                    ));
                }
                break;
            }
            visited.insert(current);
            path.push(current.to_string());
            match parent_map.get(current) {
                Some(Some(parent_id)) => current = parent_id,
                _ => break,
            }
        }
    }
}

/// 组合校验：字段级 + 结构级，返回 ValidationReport
pub fn validate(nodes_with_files: &[(&str, &Node)]) -> ValidationReport {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    validate_structure(nodes_with_files, &mut errors, &mut warnings);

    ValidationReport {
        valid: errors.is_empty(),
        errors,
        warnings,
    }
}

// ── Helpers ─────────────────────────────────────────────────

fn get_str(fm: &serde_yaml::Mapping, key: &str) -> Option<String> {
    fm.get(&YamlValue::String(key.into()))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// id 格式校验：^[a-z]+-\d{3}$
fn is_valid_id_format(id: &str) -> bool {
    let dash_pos = match id.rfind('-') {
        Some(pos) => pos,
        None => return false,
    };
    let prefix = &id[..dash_pos];
    let num = &id[dash_pos + 1..];
    !prefix.is_empty()
        && prefix.chars().all(|c| c.is_ascii_lowercase())
        && num.len() == 3
        && num.chars().all(|c| c.is_ascii_digit())
}

/// RFC3339 格式校验：YYYY-MM-DDTHH:MM:SS(+HH:MM|Z)
fn is_valid_rfc3339(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() < 20 {
        return false;
    }
    // 检查固定位置
    for i in [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18] {
        if !b[i].is_ascii_digit() {
            return false;
        }
    }
    if b[4] != b'-'
        || b[7] != b'-'
        || (b[10] != b'T' && b[10] != b't')
        || b[13] != b':'
        || b[16] != b':'
    {
        return false;
    }
    // 时区
    let tz_ok = if b.len() == 20 && b[19] == b'Z' {
        true
    } else if b.len() == 25
        && (b[19] == b'+' || b[19] == b'-')
        && b[22] == b':'
        && b[20].is_ascii_digit()
        && b[21].is_ascii_digit()
        && b[23].is_ascii_digit()
        && b[24].is_ascii_digit()
    {
        true
    } else {
        false
    };
    if !tz_ok {
        return false;
    }
    // 值域
    let month: u32 = s[5..7].parse().unwrap_or(0);
    let day: u32 = s[8..10].parse().unwrap_or(0);
    let hour: u32 = s[11..13].parse().unwrap_or(0);
    let minute: u32 = s[14..16].parse().unwrap_or(0);
    let second: u32 = s[17..19].parse().unwrap_or(0);
    (1..=12).contains(&month)
        && (1..=31).contains(&day)
        && hour <= 23
        && minute <= 59
        && second <= 59
}

/// RFC3339 → epoch seconds（用于比较 updated >= created）
fn parse_rfc3339_to_epoch(s: &str) -> Option<i64> {
    if !is_valid_rfc3339(s) {
        return None;
    }
    let year: i64 = s[0..4].parse().ok()?;
    let month: i64 = s[5..7].parse().ok()?;
    let day: i64 = s[8..10].parse().ok()?;
    let hour: i64 = s[11..13].parse().ok()?;
    let minute: i64 = s[14..16].parse().ok()?;
    let second: i64 = s[17..19].parse().ok()?;

    let days = days_from_civil(year, month, day);
    let local_secs = days * 86400 + hour * 3600 + minute * 60 + second;

    // 时区偏移：减去偏移得到 UTC
    let offset = if s.ends_with('Z') {
        0i64
    } else {
        let sign: i64 = if s.as_bytes()[19] == b'-' { 1 } else { -1 };
        let oh: i64 = s[20..22].parse().ok()?;
        let om: i64 = s[23..25].parse().ok()?;
        sign * (oh * 3600 + om * 60)
    };
    Some(local_secs + offset)
}

/// Howard Hinnant days_from_civil：年月日 → Unix 天数
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// 构造一个全合法的临时 chain 目录（3 节点 2 边）
    fn setup_valid_chain() -> TempDir {
        let tmp = TempDir::new().unwrap();
        let nodes_dir = tmp.path().join(".chain").join("nodes");
        fs::create_dir_all(&nodes_dir).unwrap();
        fs::write(
            nodes_dir.join("g-001.md"),
            "---\nid: g-001\ntype: goal\ntitle: 顶层目标\nparent: null\nstatus: pending\ncreated: 2026-08-13T10:00:00+08:00\nupdated: 2026-08-13T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# 顶层目标\n",
        ).unwrap();
        fs::write(
            nodes_dir.join("d-001.md"),
            "---\nid: d-001\ntype: design\ntitle: 设计1\nparent: g-001\nstatus: in_progress\ncreated: 2026-08-13T10:00:00+08:00\nupdated: 2026-08-13T11:00:00+08:00\nrevision: 1\ntags: [arch]\n---\n\n# 设计1\n",
        ).unwrap();
        fs::write(
            nodes_dir.join("t-001.md"),
            "---\nid: t-001\ntype: task\ntitle: 任务1\nparent: d-001\nstatus: pending\ncreated: 2026-08-13T10:00:00+08:00\nupdated: 2026-08-13T12:00:00+08:00\nrevision: 2\ntags: []\n---\n\n# 任务1\n",
        ).unwrap();
        tmp
    }

    fn scan_and_get_errors(tmp: &TempDir) -> Vec<String> {
        use crate::scanner::walker::scan_chain_dir;
        let snap = scan_chain_dir(tmp.path()).unwrap();
        snap.validation.errors
    }

    fn scan_and_get_warnings(tmp: &TempDir) -> Vec<String> {
        use crate::scanner::walker::scan_chain_dir;
        let snap = scan_chain_dir(tmp.path()).unwrap();
        snap.validation.warnings
    }

    // 1. 全合法样本
    #[test]
    fn test_valid_chain_no_errors() {
        let tmp = setup_valid_chain();
        let errors = scan_and_get_errors(&tmp);
        assert!(errors.is_empty(), "合法数据不应有错误: {:?}", errors);
    }

    // 2. id 格式非法
    #[test]
    fn test_invalid_id_format() {
        let tmp = setup_valid_chain();
        let path = tmp.path().join(".chain/nodes/g-001.md");
        let content = fs::read_to_string(&path).unwrap();
        let bad = content.replace("id: g-001", "id: G-001");
        fs::write(&path, bad).unwrap();
        let errors = scan_and_get_errors(&tmp);
        assert!(errors.iter().any(|e| e.contains("id: 格式非法")), "应报 id 格式错误: {:?}", errors);
    }

    // 3. id 与文件名不匹配
    #[test]
    fn test_id_filename_mismatch() {
        let tmp = setup_valid_chain();
        let path = tmp.path().join(".chain/nodes/g-001.md");
        let content = fs::read_to_string(&path).unwrap();
        let bad = content.replace("id: g-001", "id: g-002");
        fs::write(&path, bad).unwrap();
        let errors = scan_and_get_errors(&tmp);
        assert!(errors.iter().any(|e| e.contains("与文件名不匹配")), "应报 id 文件名不匹配: {:?}", errors);
    }

    // 4. type 非法枚举
    #[test]
    fn test_invalid_type_enum() {
        let tmp = setup_valid_chain();
        let path = tmp.path().join(".chain/nodes/d-001.md");
        let content = fs::read_to_string(&path).unwrap();
        let bad = content.replace("type: design", "type: decision");
        fs::write(&path, bad).unwrap();
        let errors = scan_and_get_errors(&tmp);
        assert!(errors.iter().any(|e| e.contains("type: 非法枚举值")), "应报 type 枚举错误: {:?}", errors);
    }

    // 5. status 非法枚举
    #[test]
    fn test_invalid_status_enum() {
        let tmp = setup_valid_chain();
        let path = tmp.path().join(".chain/nodes/g-001.md");
        let content = fs::read_to_string(&path).unwrap();
        let bad = content.replace("status: pending", "status: done");
        fs::write(&path, bad).unwrap();
        let errors = scan_and_get_errors(&tmp);
        assert!(errors.iter().any(|e| e.contains("status: 非法枚举值")), "应报 status 枚举错误: {:?}", errors);
    }

    // 6. title 为空
    #[test]
    fn test_empty_title() {
        let tmp = setup_valid_chain();
        let path = tmp.path().join(".chain/nodes/t-001.md");
        let content = fs::read_to_string(&path).unwrap();
        let bad = content.replace("title: 任务1", "title: ''");
        fs::write(&path, bad).unwrap();
        let errors = scan_and_get_errors(&tmp);
        assert!(errors.iter().any(|e| e.contains("title: 不能为空")), "应报 title 空错误: {:?}", errors);
    }

    // 7. created 不符合 RFC3339
    #[test]
    fn test_invalid_created_format() {
        let tmp = setup_valid_chain();
        let path = tmp.path().join(".chain/nodes/d-001.md");
        let content = fs::read_to_string(&path).unwrap();
        let bad = content.replace("created: 2026-08-13T10:00:00+08:00", "created: 2026-8-3");
        fs::write(&path, bad).unwrap();
        let errors = scan_and_get_errors(&tmp);
        assert!(errors.iter().any(|e| e.contains("created: 不符合 RFC3339")), "应报 created 格式错误: {:?}", errors);
    }

    // 8. revision 非正整数
    #[test]
    fn test_revision_zero() {
        let tmp = setup_valid_chain();
        let path = tmp.path().join(".chain/nodes/t-001.md");
        let content = fs::read_to_string(&path).unwrap();
        let bad = content.replace("revision: 2", "revision: 0");
        fs::write(&path, bad).unwrap();
        let errors = scan_and_get_errors(&tmp);
        assert!(errors.iter().any(|e| e.contains("revision: 必须为正整数")), "应报 revision 非正整数: {:?}", errors);
    }

    // 9. tags 非数组
    #[test]
    fn test_tags_not_array() {
        let tmp = setup_valid_chain();
        let path = tmp.path().join(".chain/nodes/d-001.md");
        let content = fs::read_to_string(&path).unwrap();
        let bad = content.replace("tags: [arch]", "tags: arch,ui");
        fs::write(&path, bad).unwrap();
        let errors = scan_and_get_errors(&tmp);
        assert!(errors.iter().any(|e| e.contains("tags: 必须为数组")), "应报 tags 非数组: {:?}", errors);
    }

    // 10. updated 早于 created
    #[test]
    fn test_updated_before_created() {
        let tmp = setup_valid_chain();
        let path = tmp.path().join(".chain/nodes/d-001.md");
        let content = fs::read_to_string(&path).unwrap();
        let bad = content.replace("updated: 2026-08-13T11:00:00+08:00", "updated: 2026-08-13T09:00:00+08:00");
        fs::write(&path, bad).unwrap();
        let errors = scan_and_get_errors(&tmp);
        assert!(errors.iter().any(|e| e.contains("updated: 早于 created")), "应报 updated 早于 created: {:?}", errors);
    }

    // 11. id 重复
    #[test]
    fn test_duplicate_id() {
        let tmp = setup_valid_chain();
        let path = tmp.path().join(".chain/nodes/t-001.md");
        let content = fs::read_to_string(&path).unwrap();
        let bad = content.replace("id: t-001", "id: d-001");
        fs::write(&path, bad).unwrap();
        let errors = scan_and_get_errors(&tmp);
        assert!(errors.iter().any(|e| e.contains("id: 重复")), "应报 id 重复: {:?}", errors);
    }

    // 12. parent 悬空引用
    #[test]
    fn test_dangling_parent() {
        let tmp = setup_valid_chain();
        let path = tmp.path().join(".chain/nodes/t-001.md");
        let content = fs::read_to_string(&path).unwrap();
        let bad = content.replace("parent: d-001", "parent: x-999");
        fs::write(&path, bad).unwrap();
        let errors = scan_and_get_errors(&tmp);
        assert!(errors.iter().any(|e| e.contains("parent: 悬空引用")), "应报 parent 悬空: {:?}", errors);
    }

    // 13. 环检测
    #[test]
    fn test_cycle_detection() {
        let tmp = TempDir::new().unwrap();
        let nodes_dir = tmp.path().join(".chain").join("nodes");
        fs::create_dir_all(&nodes_dir).unwrap();
        // a→b→a 形成环
        fs::write(
            nodes_dir.join("g-001.md"),
            "---\nid: g-001\ntype: goal\ntitle: A\nparent: d-001\nstatus: pending\ncreated: 2026-08-13T10:00:00+08:00\nupdated: 2026-08-13T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# A\n",
        ).unwrap();
        fs::write(
            nodes_dir.join("d-001.md"),
            "---\nid: d-001\ntype: design\ntitle: B\nparent: g-001\nstatus: pending\ncreated: 2026-08-13T10:00:00+08:00\nupdated: 2026-08-13T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# B\n",
        ).unwrap();
        let errors = scan_and_get_errors(&tmp);
        assert!(errors.iter().any(|e| e.contains("检测到环")), "应报环检测: {:?}", errors);
    }

    // 14. 多个根节点
    #[test]
    fn test_multiple_roots() {
        let tmp = setup_valid_chain();
        // 把 t-001 的 parent 改成 null → 两个根节点
        let path = tmp.path().join(".chain/nodes/t-001.md");
        let content = fs::read_to_string(&path).unwrap();
        let bad = content.replace("parent: d-001", "parent: null");
        fs::write(&path, bad).unwrap();
        let errors = scan_and_get_errors(&tmp);
        assert!(errors.iter().any(|e| e.contains("根节点")), "应报多个根节点: {:?}", errors);
    }

    // 15. 子 goal 合法化（M10）：goal 有 parent 不再产生 warning（挂失败节点下是标准用法）
    #[test]
    fn test_sub_goal_with_parent_no_warning() {
        let tmp = setup_valid_chain();
        // g-001 是 goal 但有 parent（子 goal 场景）
        let path = tmp.path().join(".chain/nodes/g-001.md");
        let content = fs::read_to_string(&path).unwrap();
        let bad = content.replace("parent: null", "parent: d-001");
        fs::write(&path, bad).unwrap();
        let warnings = scan_and_get_warnings(&tmp);
        assert!(!warnings.iter().any(|w| w.contains("goal 类型节点不应有 parent")), "子 goal 不应报 parent warning: {:?}", warnings);
    }

    // 16. 辅助函数测试：is_valid_id_format
    #[test]
    fn test_is_valid_id_format() {
        assert!(is_valid_id_format("g-001"));
        assert!(is_valid_id_format("task-001"));
        assert!(is_valid_id_format("v-999"));
        assert!(!is_valid_id_format("G-001"));
        assert!(!is_valid_id_format("g-1"));
        assert!(!is_valid_id_format("g001"));
        assert!(!is_valid_id_format("g-001-002"));
    }

    // 17. 辅助函数测试：is_valid_rfc3339
    #[test]
    fn test_is_valid_rfc3339() {
        assert!(is_valid_rfc3339("2026-08-13T10:00:00+08:00"));
        assert!(is_valid_rfc3339("2026-08-13T10:00:00Z"));
        assert!(!is_valid_rfc3339("2026-8-3"));
        assert!(!is_valid_rfc3339("2026-08-13 10:00:00+08:00"));
        assert!(!is_valid_rfc3339("2026-13-01T10:00:00+08:00"));
        assert!(!is_valid_rfc3339("2026-08-13T25:00:00+08:00"));
    }

    // 18. 辅助函数测试：days_from_civil
    #[test]
    fn test_days_from_civil() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 2, 29), 11016);
        assert_eq!(days_from_civil(2026, 8, 13), 20678);
    }
}
