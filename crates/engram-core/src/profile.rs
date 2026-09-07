//! 双模式 profile 配置包（终版 §1.4）：
//! 开发模式与链协议模式的差异收为两个 profile 配置（rel 词表 + 指南指针 + type/status 词表 + 校验开关）。
//! 模式是配置，不是分支——入口只传 profile 名（ScanMode），core 内部取配置。

use crate::guide::{AI_GUIDE, AI_GUIDE_DEV, AI_GUIDE_DEV_VERSION, AI_GUIDE_VERSION};
use crate::model::ScanMode;

/// rel 三类型词表（D2，ADR 0002：不扩张）
pub const REL_TYPES: &[&str] = &["contains", "solves", "alternative"];

/// 分析模式四类型词表（链协议）
pub const ANALYSIS_TYPES: &[&str] = &["goal", "design", "task", "verification"];

/// 分析模式五状态词表（链协议）
pub const ANALYSIS_STATUSES: &[&str] = &["pending", "in_progress", "success", "failed", "blocked"];

/// 双模式 profile 配置包
pub struct Profile {
    pub mode: ScanMode,
    /// rel 词表（D2 校验）
    pub rel_vocab: &'static [&'static str],
    /// 指南指针（D4 提示、init 写盘）
    pub guide: &'static str,
    pub guide_version: u32,
    /// type 词表（分析四类型；开发含 note）
    pub type_vocab: &'static [&'static str],
    /// status 词表（分析五状态；开发含 none）
    pub status_vocab: &'static [&'static str],
    /// 严格链协议校验（分析模式）。
    /// 声明式开关：当前由 walker（build_dev_node vs 严格解析）/ops 的模式分支实际执行，
    /// 生产代码按 mode 分支等价于此开关——二者语义必须保持同步（词表是唯一数据源）。
    pub strict: bool,
}

/// 分析模式 profile：严格链协议（AI 按协议维护链，GUI 不可自由增删）
pub const ANALYSIS: Profile = Profile {
    mode: ScanMode::Analysis,
    rel_vocab: REL_TYPES,
    guide: AI_GUIDE,
    guide_version: AI_GUIDE_VERSION,
    type_vocab: ANALYSIS_TYPES,
    status_vocab: ANALYSIS_STATUSES,
    strict: true,
};

/// 开发模式 profile：自由知识图谱（宽松校验、自由增删链）
pub const DEV: Profile = Profile {
    mode: ScanMode::Dev,
    rel_vocab: REL_TYPES,
    guide: AI_GUIDE_DEV,
    guide_version: AI_GUIDE_DEV_VERSION,
    type_vocab: &["goal", "design", "task", "verification", "note"],
    status_vocab: &[
        "pending",
        "in_progress",
        "success",
        "failed",
        "blocked",
        "none",
    ],
    strict: false,
};

/// 按模式取 profile 配置
pub fn profile_for(mode: ScanMode) -> &'static Profile {
    if mode.is_dev() {
        &DEV
    } else {
        &ANALYSIS
    }
}

/// 模式 → 标签字符串（`.chain/.mode` 值）
pub fn mode_str(m: ScanMode) -> &'static str {
    if m.is_dev() {
        "dev"
    } else {
        "analysis"
    }
}

/// 按模式字符串取 profile（None/未知 → 分析模式，与历史入口语义一致）
pub fn profile_for_str(mode: Option<&str>) -> &'static Profile {
    if mode.is_some_and(|m| m == "dev") {
        &DEV
    } else {
        &ANALYSIS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_dispatch() {
        assert_eq!(profile_for(ScanMode::Dev).mode, ScanMode::Dev);
        assert_eq!(profile_for(ScanMode::Analysis).mode, ScanMode::Analysis);
        assert!(!profile_for(ScanMode::Dev).strict);
        assert!(profile_for(ScanMode::Analysis).strict);
    }

    #[test]
    fn test_vocab_contains_rel_types() {
        assert!(DEV.rel_vocab.contains(&"solves"));
        assert!(ANALYSIS.rel_vocab.contains(&"alternative"));
        assert_eq!(ANALYSIS.rel_vocab, REL_TYPES);
        assert_eq!(DEV.rel_vocab, REL_TYPES);
    }

    #[test]
    fn test_mode_str() {
        assert_eq!(mode_str(ScanMode::Dev), "dev");
        assert_eq!(mode_str(ScanMode::Analysis), "analysis");
    }

    #[test]
    fn test_profile_for_str_defaults_analysis() {
        assert_eq!(profile_for_str(Some("dev")).mode, ScanMode::Dev);
        assert_eq!(profile_for_str(Some("analysis")).mode, ScanMode::Analysis);
        assert_eq!(profile_for_str(Some("bogus")).mode, ScanMode::Analysis);
        assert_eq!(profile_for_str(None).mode, ScanMode::Analysis);
    }
}
