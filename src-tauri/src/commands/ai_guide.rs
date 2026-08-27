//! AI 使用指南模块：单源资源（仓库根 resources/AI_GUIDE.md），编译期内嵌进二进制。
//! 两个出口：init_chain 初始化时写盘 `.chain/AI_GUIDE.md`；get_ai_guide 供前端「复制 AI 指南」按钮。
//! v1.2：指南带版本标记（首行 <!-- CHAIN_GUIDE_VERSION: N -->），用于检测陈旧副本并自动刷新。
use tauri::command;

/// 指南全文（编译期内嵌，与仓库 resources/AI_GUIDE.md 单源同步，改文档后重编译即生效）
pub const AI_GUIDE: &str = include_str!("../../../resources/AI_GUIDE.md");

/// 内嵌指南的版本号（与 resources/AI_GUIDE.md 首行版本标记一致；改指南时必须同步 +1）
pub const AI_GUIDE_VERSION: u32 = 5;

/// 从指南文本解析版本标记（首行 `<!-- CHAIN_GUIDE_VERSION: N -->`）。
/// 返回 None = 无标记（旧版指南或人工编辑过）。
pub fn parse_guide_version(content: &str) -> Option<u32> {
    let first_line = content.lines().next()?.trim();
    let inner = first_line
        .strip_prefix("<!--")?
        .strip_suffix("-->")?
        .trim();
    let num_str = inner
        .split(':')
        .last()?
        .trim();
    num_str.parse::<u32>().ok()
}

/// 返回 AI 使用指南全文
#[command]
pub fn get_ai_guide() -> String {
    AI_GUIDE.to_string()
}

/// 返回内嵌指南版本号
#[command]
pub fn get_guide_version() -> u32 {
    AI_GUIDE_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_guide_embedded() {
        assert!(AI_GUIDE.len() > 1000, "指南应已内嵌且非空");
        assert!(AI_GUIDE.contains("Chain Protocol"), "指南应含标题");
        assert!(AI_GUIDE.contains("success"), "指南应含 status 枚举");
        assert!(AI_GUIDE.contains("子 goal"), "指南应含子 goal 规则");
        assert!(AI_GUIDE.contains("title 命名规范"), "指南应含 v4 title 命名规范（图谱显示名）");
        assert!(AI_GUIDE.contains("LaTeX"), "指南应含 v5 数学公式 LaTeX 书写约定");
    }

    #[test]
    fn test_ai_guide_has_nine_rules() {
        // v2 起守则 9 条：第 9 条「不拍脑袋实现」必须有
        assert!(AI_GUIDE.contains("不拍脑袋实现"), "指南应含第 9 条守则「不拍脑袋实现」");
        assert!(AI_GUIDE.contains("不伪造结果"), "指南应含第 1 条守则");
        assert!(AI_GUIDE.contains("不跳验证"), "指南应含第 8 条守则");
    }

    #[test]
    fn test_ai_guide_version_marker() {
        // 首行版本标记必须存在且与 AI_GUIDE_VERSION 常量一致（同步测试，防改文档忘改常量）
        let parsed = parse_guide_version(AI_GUIDE).expect("指南首行应有版本标记");
        assert_eq!(parsed, AI_GUIDE_VERSION, "指南标记版本与常量不一致");
    }

    #[test]
    fn test_parse_guide_version_old_style() {
        // 无标记的旧版指南 → None（触发刷新）
        assert_eq!(parse_guide_version("# 无标记的旧指南"), None);
        assert_eq!(parse_guide_version(""), None);
        // 非法数字 → None
        assert_eq!(parse_guide_version("<!-- CHAIN_GUIDE_VERSION: abc -->"), None);
    }

    #[test]
    fn test_parse_guide_version_v1() {
        assert_eq!(parse_guide_version("<!-- CHAIN_GUIDE_VERSION: 1 -->"), Some(1));
        assert_eq!(parse_guide_version("<!-- CHAIN_GUIDE_VERSION: 3 -->"), Some(3));
    }
}
