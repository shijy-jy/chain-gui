//! 四版本矩阵（ADR 0011）：GUI/工具契约/指南/索引格式四版本独立管理 + git 短哈希。
//! 矩阵装配在 core；各入口注入自己的 app 版本号（GUI 取 tauri.conf，MCP 取 CARGO_PKG_VERSION）。

use crate::guide::{AI_GUIDE_DEV_VERSION, AI_GUIDE_VERSION};

/// MCP 工具契约版本：golden 契约测试覆盖的工具集发生增删改时必须 +1（宪法第 8 条③），
/// 并同步更新 docs/test-golden/engram-mcp-golden.json。
/// v3（M7'）：新增 archive_node / unlink_nodes（9→12 工具），golden 16 条。
pub const TOOL_CONTRACT_VERSION: u32 = 3;

/// 索引/schema 格式版本（宪法第 9 条；§10⑤ 起已实现，见 crate::schema）
pub const SCHEMA_SPEC_VERSION: Option<&str> = Some("1.1");

/// 四版本矩阵 + 漂移锚点
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct VersionInfo {
    /// 软件版本（GUI 侧取自 tauri.conf.json，MCP 侧取自 crate 版本；两者随 tag 同步）
    pub app: String,
    /// 编译期注入的 git 短哈希（build.rs）
    pub git_hash: String,
    /// MCP 工具契约版本
    pub tool_contract: u32,
    /// 分析模式指南版本
    pub guide_analysis: u32,
    /// 开发模式指南版本
    pub guide_dev: u32,
    /// 数据 schema 格式版本（None = 未实现；当前已实现，见 crate::schema）
    pub schema: Option<String>,
}

impl VersionInfo {
    pub fn new(app_version: &str) -> Self {
        Self {
            app: app_version.to_string(),
            git_hash: option_env!("GIT_SHORT_HASH")
                .unwrap_or("unknown")
                .to_string(),
            tool_contract: TOOL_CONTRACT_VERSION,
            guide_analysis: AI_GUIDE_VERSION,
            guide_dev: AI_GUIDE_DEV_VERSION,
            schema: SCHEMA_SPEC_VERSION.map(|s| s.to_string()),
        }
    }

    /// --version 单行人类可读输出（发布管线校验锚点）
    pub fn display_line(&self) -> String {
        format!(
            "Engram {} (git {}) tool-contract v{} guide analysis v{}/dev v{} schema v{}",
            self.app,
            self.git_hash,
            self.tool_contract,
            self.guide_analysis,
            self.guide_dev,
            self.schema.as_deref().unwrap_or("n/a（未实现）"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_info_display_line_shape() {
        let v = VersionInfo::new("2.9.0");
        let line = v.display_line();
        assert!(
            line.starts_with("Engram 2.9.0 (git "),
            "格式应为 app+hash 开头：{line}"
        );
        assert!(
            line.contains("tool-contract v3"),
            "应含工具契约版本：{line}"
        );
        assert!(
            line.contains(&format!(
                "guide analysis v{AI_GUIDE_VERSION}/dev v{AI_GUIDE_DEV_VERSION}"
            )),
            "应含双指南版本：{line}"
        );
        assert!(
            line.contains("schema v1.1"),
            "schema 已实现应标注版本：{line}"
        );
        assert_eq!(v.schema.as_deref(), Some("1.1"));
    }

    #[test]
    fn test_git_hash_injected_or_unknown() {
        let v = VersionInfo::new("x");
        // 无 .git 环境（源码归档）回退 unknown 而非 panic
        assert!(!v.git_hash.is_empty());
    }
}
