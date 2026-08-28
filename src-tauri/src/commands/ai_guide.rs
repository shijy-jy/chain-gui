//! AI 使用指南模块：单源资源（仓库根 resources/），编译期内嵌进二进制。
//! 两个出口：init_chain 初始化时写盘 `.chain/AI_GUIDE.md`；get_ai_guide 供前端「复制 AI 指南」按钮。
//! v1.2：指南带版本标记（首行 <!-- CHAIN_GUIDE_VERSION: N -->），用于检测陈旧副本并自动刷新。
//! v2.1：双指南——分析模式（链协议，AI_GUIDE）与开发模式（知识库搭建，AI_GUIDE_DEV），
//!        按工作区模式标签（.chain/.mode）区分；两个入口命令按 mode 返回对应指南。
use tauri::command;

/// 分析模式指南全文（编译期内嵌，与仓库 resources/AI_GUIDE.md 单源同步）
pub const AI_GUIDE: &str = include_str!("../../../resources/AI_GUIDE.md");

/// 开发模式指南全文（知识库搭建，与仓库 resources/AI_GUIDE_DEV.md 单源同步）
pub const AI_GUIDE_DEV: &str = include_str!("../../../resources/AI_GUIDE_DEV.md");

/// 分析模式指南版本号（与 resources/AI_GUIDE.md 首行标记一致；改指南时必须同步 +1）
pub const AI_GUIDE_VERSION: u32 = 7;

/// 开发模式指南版本号（与 resources/AI_GUIDE_DEV.md 首行 CHAIN_GUIDE_DEV_VERSION 标记一致）
pub const AI_GUIDE_DEV_VERSION: u32 = 2;

/// 从指南文本解析版本标记（首行 `<!-- CHAIN_GUIDE_VERSION: N -->` 或 `<!-- CHAIN_GUIDE_DEV_VERSION: N -->`）。
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

/// 按模式取指南文本：dev → 开发模式知识库指南；其余 → 分析模式链协议指南
pub fn guide_for(mode: Option<&str>) -> &'static str {
    if mode.is_some_and(|m| m == "dev") {
        AI_GUIDE_DEV
    } else {
        AI_GUIDE
    }
}

/// 按模式取指南版本号
pub fn guide_version_for(mode: Option<&str>) -> u32 {
    if mode.is_some_and(|m| m == "dev") {
        AI_GUIDE_DEV_VERSION
    } else {
        AI_GUIDE_VERSION
    }
}

/// 返回当前模式对应的 AI 使用指南全文（v2.1 双指南）
#[command]
pub fn get_ai_guide(mode: Option<String>) -> String {
    guide_for(mode.as_deref()).to_string()
}

/// 返回当前模式对应的内嵌指南版本号
#[command]
pub fn get_guide_version(mode: Option<String>) -> u32 {
    guide_version_for(mode.as_deref())
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
    fn test_ai_guide_dev_embedded() {
        // v2.1 开发模式知识库指南
        assert!(AI_GUIDE_DEV.len() > 500, "开发模式指南应已内嵌且非空");
        assert!(AI_GUIDE_DEV.contains("知识库搭建"), "应含标题");
        assert!(AI_GUIDE_DEV.contains(".chain/.mode = dev"), "应含模式标签说明");
        assert!(AI_GUIDE_DEV.contains("Zettelkasten"), "应含优秀实践参考（卡片盒）");
        assert!(AI_GUIDE_DEV.contains("不可混用"), "应含两模式不可混用规则");
        // v2：递进关系建模（rel 字段）
        assert!(AI_GUIDE_DEV.contains("递进关系建模"), "应含 v2 递进关系建模章节");
        assert!(AI_GUIDE_DEV.contains("rel"), "应含 rel 关系类型字段说明");
        assert!(AI_GUIDE_DEV.contains("solves"), "应含 solves 关系类型");
    }

    #[test]
    fn test_ai_guide_has_nine_rules() {
        // v2 起守则 9 条：第 9 条「不拍脑袋实现」必须有
        assert!(AI_GUIDE.contains("不拍脑袋实现"), "指南应含第 9 条守则「不拍脑袋实现」");
        assert!(AI_GUIDE.contains("不伪造结果"), "指南应含第 1 条守则");
        assert!(AI_GUIDE.contains("不跳验证"), "指南应含第 8 条守则");
        // v6：守则 9 补强——接到需求先找与需求相关的优秀实现，检索手段不限
        assert!(AI_GUIDE.contains("与需求相关的优秀实现"), "指南应含 v6 参考实现守则");
        assert!(AI_GUIDE.contains("检索手段不限"), "指南应含检索手段不限");
        // v7：工作区模式标签（.chain/.mode）
        assert!(AI_GUIDE.contains(".mode"), "指南应含 v7 工作区模式标签");
        assert!(AI_GUIDE.contains("不可混用"), "指南应含 v7 模式不可混用规则");
    }

    #[test]
    fn test_ai_guide_version_marker() {
        // 首行版本标记必须存在且与 AI_GUIDE_VERSION 常量一致（同步测试，防改文档忘改常量）
        let parsed = parse_guide_version(AI_GUIDE).expect("指南首行应有版本标记");
        assert_eq!(parsed, AI_GUIDE_VERSION, "指南标记版本与常量不一致");
    }

    #[test]
    fn test_ai_guide_dev_version_marker() {
        let parsed = parse_guide_version(AI_GUIDE_DEV).expect("开发指南首行应有版本标记");
        assert_eq!(parsed, AI_GUIDE_DEV_VERSION, "开发指南标记版本与常量不一致");
    }

    #[test]
    fn test_guide_for_mode_dispatch() {
        assert_eq!(guide_for(Some("dev")), AI_GUIDE_DEV);
        assert_eq!(guide_for(Some("analysis")), AI_GUIDE);
        assert_eq!(guide_for(None), AI_GUIDE);
        assert_eq!(guide_version_for(Some("dev")), AI_GUIDE_DEV_VERSION);
        assert_eq!(guide_version_for(Some("analysis")), AI_GUIDE_VERSION);
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
        assert_eq!(parse_guide_version("<!-- CHAIN_GUIDE_DEV_VERSION: 1 -->"), Some(1));
    }
}
