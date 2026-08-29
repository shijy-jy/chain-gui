use anyhow::{Context, Result};
use crate::model::node::Node;

/// 当前时间的 RFC3339 字符串（固定 UTC+8，手写 civil 算法，不引 chrono 依赖）
pub fn now_iso8601() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let cst = secs + 8 * 3600; // UTC+8
    let days = cst.div_euclid(86400);
    let secs_of_day = cst.rem_euclid(86400);
    let (y, m, d) = civil_from_days(days);
    let hh = secs_of_day / 3600;
    let mm = (secs_of_day % 3600) / 60;
    let ss = secs_of_day % 60;
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}+08:00", y, m, d, hh, mm, ss)
}

/// Howard Hinnant 的 civil_from_days 算法：Unix 天数 → (年, 月, 日)
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// 解析节点文件，返回 frontmatter Mapping 和 body 正文
pub fn parse(content: &str) -> Result<(serde_yaml::Mapping, String)> {
    let content = content.trim_start();
    if !content.starts_with("---") {
        anyhow::bail!("节点文件必须以 --- 开头的 frontmatter 起始");
    }
    let rest = &content[3..];
    let end = rest
        .find("\n---")
        .context("frontmatter 未闭合（找不到 ---）")?;
    let yaml_str = rest[..end].trim_start_matches('\n');
    let body = rest[end + 4..].trim_start_matches('\n').to_string();

    let fm: serde_yaml::Mapping = serde_yaml::from_str(yaml_str)
        .context("frontmatter YAML 解析失败")?;

    Ok((fm, body))
}

/// 将 frontmatter Mapping + body 序列化回 .md 文件内容
pub fn serialize(fm: &serde_yaml::Mapping, body: &str) -> Result<String> {
    let yaml_str = serde_yaml::to_string(fm).context("frontmatter 序列化失败")?;
    // serde_yaml::to_string 末尾带 \n，需要 trim
    let yaml_str = yaml_str.trim_end();
    if body.is_empty() {
        Ok(format!("---\n{}\n---\n", yaml_str))
    } else {
        Ok(format!("---\n{}\n---\n\n{}\n", yaml_str, body))
    }
}

/// 按字节数安全截断字符串：截断点落在多字节 UTF-8 字符中间时向前回退到字符边界。
/// 中文每字 3 字节，直接按字节切会 panic——所有摘要截断必须走这里。
pub fn truncate_utf8(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

pub fn parse_node_file(content: &str) -> Result<(Node, String)> {
    let (fm, body) = parse(content)?;

    let mut node: Node = serde_yaml::from_str(&serde_yaml::to_string(&fm).unwrap())
        .context("frontmatter YAML 解析失败")?;
    node.body = body.clone();

    Ok((node, body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::node::{NodeType, NodeStatus};

    #[test]
    fn test_parse_simple_node() {
        let content = "---\nid: t-001\ntype: task\ntitle: 测试节点\nparent: d-001\nstatus: pending\ncreated: 2026-08-13T10:00:00+08:00\nupdated: 2026-08-13T10:00:00+08:00\nrevision: 1\ntags: [test]\n---\n\n# 测试节点正文\n\n正文内容...\n";
        let (node, body) = parse_node_file(content).unwrap();
        assert_eq!(node.id, "t-001");
        assert_eq!(node.title, "测试节点");
        assert_eq!(node.node_type, NodeType::Task);
        assert_eq!(node.status, NodeStatus::Pending);
        assert!(body.contains("测试节点正文"));
        assert!(node.body.contains("测试节点正文"));
    }

    #[test]
    fn test_parse_node_with_unicode_title() {
        let content = "---\nid: g-001\ntype: goal\ntitle: 顶层目标——链协议\nparent: null\nstatus: pending\ncreated: 2026-08-13T10:00:00+08:00\nupdated: 2026-08-13T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# 顶层目标\n";
        let (node, body) = parse_node_file(content).unwrap();
        assert_eq!(node.id, "g-001");
        assert_eq!(node.title, "顶层目标——链协议");
        assert_eq!(node.node_type, NodeType::Goal);
        assert!(body.contains("顶层目标"));
    }

    #[test]
    fn test_parse_node_no_body() {
        let content = "---\nid: d-001\ntype: design\ntitle: 设计1\nparent: g-001\nstatus: in_progress\ncreated: 2026-08-13T10:00:00+08:00\nupdated: 2026-08-13T10:00:00+08:00\nrevision: 1\ntags: []\n---\n";
        let (node, body) = parse_node_file(content).unwrap();
        assert_eq!(node.id, "d-001");
        assert_eq!(node.node_type, NodeType::Design);
        assert_eq!(node.status, NodeStatus::InProgress);
        assert!(body.is_empty());
    }

    #[test]
    fn test_parse_node_no_frontmatter() {
        let content = "# 没有 frontmatter 的文件\n正文内容";
        let result = parse_node_file(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_node_unclosed_frontmatter() {
        let content = "---\nid: t-001\ntype: task\ntitle: 未闭合\n";
        let result = parse_node_file(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_now_iso8601_format() {
        let s = now_iso8601();
        // 必须是 RFC3339 格式：YYYY-MM-DDTHH:MM:SS+08:00（25 字符）
        assert_eq!(s.len(), 25, "now_iso8601 长度应为 25，实际：{}", s);
        assert_eq!(&s[4..5], "-");
        assert_eq!(&s[7..8], "-");
        assert_eq!(&s[10..11], "T");
        assert_eq!(&s[13..14], ":");
        assert_eq!(&s[16..17], ":");
        assert_eq!(&s[19..25], "+08:00");
        let year: i32 = s[0..4].parse().unwrap();
        assert!(year >= 2026 && year < 3000, "年份不合理：{}", year);
    }

    #[test]
    fn test_civil_from_days_known_dates() {
        // 1970-01-01 = 第 0 天
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        // 2000-01-01 = 第 10957 天
        assert_eq!(civil_from_days(10957), (2000, 1, 1));
        // 2000-02-29（闰日）= 第 11016 天
        assert_eq!(civil_from_days(11016), (2000, 2, 29));
        // 2024-01-01 = 第 19723 天
        assert_eq!(civil_from_days(19723), (2024, 1, 1));
        // 2026-08-13 = 第 20678 天
        assert_eq!(civil_from_days(20678), (2026, 8, 13));
        // 2026-12-31 = 第 20818 天（2026 非闰年）
        assert_eq!(civil_from_days(20818), (2026, 12, 31));
    }

    #[test]
    fn test_truncate_utf8_short() {
        // 短于上限：原样返回
        assert_eq!(truncate_utf8("短文本", 100), "短文本");
        // 恰好等于上限
        assert_eq!(truncate_utf8("abc", 3), "abc");
        assert_eq!(truncate_utf8("", 5), "");
    }

    #[test]
    fn test_truncate_utf8_chinese_boundary() {
        // 中文每字 3 字节：10 字 = 30 字节，截断到 16 字节应回退到第 5 字末尾（15 字节）
        let s = "汉字正文汉字正文汉字正文"; // 10 字
        let t = truncate_utf8(s, 16);
        assert!(s.is_char_boundary(t.len()), "截断点必须在字符边界");
        assert_eq!(t, "汉字正文汉"); // 5 字 = 15 字节
    }

    #[test]
    fn test_truncate_utf8_never_panics() {
        // 大量中文，多种截断点，都不能 panic
        let s = "汉字正文".repeat(100); // 400 字 = 1200 字节
        for max in 0..=s.len() {
            let t = truncate_utf8(&s, max);
            assert!(t.len() <= max);
            assert!(s.is_char_boundary(t.len()));
        }
    }
}
