//! 工作区列表与模式标签（v2.1，纯逻辑）：
//! - 模式标签写在 `.chain/.mode`（analysis/dev），随工程走；软件扫描时读标签自动归类
//! - 工作区列表持久化为 workspaces.json（仅存路径+模式，磁盘文件永不删除）；
//!   配置目录路径由入口（GUI AppHandle）提供，本模块只做纯 IO
//! - `check_mode`：模式强校验——文件夹标签与期望模式不符时拒绝操作（两模式隔离的硬保证）

use crate::model::ScanMode;
use crate::profile;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const MODE_TAG_FILE: &str = ".mode";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceInfo {
    pub path: String,
    /// "analysis" | "dev"
    pub mode: String,
    pub name: String,
}

fn mode_label(m: ScanMode) -> &'static str {
    if m.is_dev() {
        "开发"
    } else {
        "分析"
    }
}

/// 读 `.chain/.mode` 标签；缺失/非法 → None（旧工作区未打标）
pub fn read_mode_tag(root: &std::path::Path) -> Option<ScanMode> {
    let content = std::fs::read_to_string(root.join(".chain").join(MODE_TAG_FILE)).ok()?;
    match content.trim() {
        "dev" => Some(ScanMode::Dev),
        "analysis" => Some(ScanMode::Analysis),
        _ => None,
    }
}

/// 写 `.chain/.mode` 标签
pub fn write_mode_tag(root: &std::path::Path, mode: ScanMode) -> Result<(), String> {
    std::fs::write(
        root.join(".chain").join(MODE_TAG_FILE),
        profile::mode_str(mode),
    )
    .map_err(|e| format!("写模式标签失败：{e}"))
}

/// 模式强校验（v2.1）：文件夹有标签且与期望不符 → 报错。
/// 无标签（旧工程）放行——补签由 add_workspace 完成。
pub fn check_mode(root: &std::path::Path, expected: ScanMode) -> Result<(), String> {
    if let Some(tag) = read_mode_tag(root) {
        if tag != expected {
            return Err(format!(
                "该工作区是「{}模式」，不能在「{}模式」下操作（请在工作区栏切换模式）",
                mode_label(tag),
                mode_label(expected),
            ));
        }
    }
    Ok(())
}

/// 模式标签的人类可读名（add_workspace 报错文案用）
pub fn mode_label_str(m: ScanMode) -> &'static str {
    mode_label(m)
}

pub fn read_workspaces(path: &std::path::Path) -> Vec<WorkspaceInfo> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn write_workspaces(path: &std::path::Path, list: &[WorkspaceInfo]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败：{e}"))?;
    }
    let json = serde_json::to_string_pretty(list).map_err(|e| format!("序列化失败：{e}"))?;
    std::fs::write(path, json).map_err(|e| format!("写工作区列表失败：{e}"))
}

/// 从字符串路径解析 WorkspaceInfo 路径规范化辅助（add_workspace 用）：
/// canonicalize 在 Windows 返回 `\\?\` verbatim 前缀：剥掉，保证列表路径与用户所见一致。
pub fn strip_verbatim_prefix(path_str: String) -> String {
    match path_str.strip_prefix(r"\\?\") {
        Some(stripped) => stripped.to_string(),
        None => path_str,
    }
}

/// 由目录推导默认工作区名（add_workspace 用）
pub fn workspace_name_from_path(path: &std::path::Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

/// 供 GUI add_workspace 使用的目录规范化
pub fn canonicalize_workspace_dir(dir: &str) -> Result<PathBuf, String> {
    let root = PathBuf::from(dir);
    root.canonicalize()
        .map_err(|e| format!("目录不可访问：{e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_mode_tag_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join(".chain")).unwrap();
        assert_eq!(read_mode_tag(root), None);
        write_mode_tag(root, ScanMode::Dev).unwrap();
        assert_eq!(read_mode_tag(root), Some(ScanMode::Dev));
        write_mode_tag(root, ScanMode::Analysis).unwrap();
        assert_eq!(read_mode_tag(root), Some(ScanMode::Analysis));
    }

    #[test]
    fn test_check_mode_rejects_mismatch() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join(".chain")).unwrap();
        write_mode_tag(root, ScanMode::Dev).unwrap();
        assert!(
            check_mode(root, ScanMode::Analysis).is_err(),
            "开发工作区不应允许分析操作"
        );
        assert!(check_mode(root, ScanMode::Dev).is_ok(), "同模式应放行");
        // 未打标放行（补签由 add_workspace 完成）
        let tmp2 = TempDir::new().unwrap();
        fs::create_dir_all(tmp2.path().join(".chain")).unwrap();
        assert!(check_mode(tmp2.path(), ScanMode::Analysis).is_ok());
    }

    #[test]
    fn test_workspaces_json_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("ws.json");
        let list = vec![
            WorkspaceInfo {
                path: "G:/a".into(),
                mode: "analysis".into(),
                name: "a".into(),
            },
            WorkspaceInfo {
                path: "G:/b".into(),
                mode: "dev".into(),
                name: "b".into(),
            },
        ];
        write_workspaces(&p, &list).unwrap();
        let back = read_workspaces(&p);
        assert_eq!(back.len(), 2);
        assert_eq!(back[1].mode, "dev");
        // 空文件/缺失 → 空列表
        let tmp2 = TempDir::new().unwrap();
        assert!(read_workspaces(&tmp2.path().join("none.json")).is_empty());
    }

    #[test]
    fn test_strip_verbatim_prefix() {
        assert_eq!(strip_verbatim_prefix(r"\\?\G:\a".into()), r"G:\a");
        assert_eq!(strip_verbatim_prefix(r"G:\a".into()), r"G:\a");
    }
}
