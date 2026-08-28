//! 工作区列表与模式标签（v2.1）：
//! - 模式标签写在 `.chain/.mode`（analysis/dev），随工程走；软件扫描时读标签自动归类
//! - 工作区列表持久化在 app 配置目录 workspaces.json（仅存路径+模式，磁盘文件永不删除）
//! - `check_mode`：模式强校验——文件夹标签与期望模式不符时拒绝操作（两模式隔离的硬保证）

use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, command};
use crate::model::ScanMode;

pub const MODE_TAG_FILE: &str = ".mode";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceInfo {
    pub path: String,
    /// "analysis" | "dev"
    pub mode: String,
    pub name: String,
}

pub fn mode_to_str(m: ScanMode) -> &'static str {
    if m.is_dev() { "dev" } else { "analysis" }
}

fn mode_label(m: ScanMode) -> &'static str {
    if m.is_dev() { "开发" } else { "分析" }
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
    std::fs::write(root.join(".chain").join(MODE_TAG_FILE), mode_to_str(mode))
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

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_config_dir()
        .map_err(|e| e.to_string())?
        .join("workspaces.json"))
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

/// 列出所有工作区；每次实时读文件夹标签重新归类（标签变了自动归到对应层）
#[command]
pub fn list_workspaces(app: AppHandle) -> Result<Vec<WorkspaceInfo>, String> {
    let p = config_path(&app)?;
    let mut list = read_workspaces(&p);
    for ws in &mut list {
        if let Some(tag) = read_mode_tag(std::path::Path::new(&ws.path)) {
            ws.mode = mode_to_str(tag).to_string();
        }
    }
    Ok(list)
}

/// 添加工作区：
/// - 文件夹已有标签且与所选模式不符 → 拒绝（不允许混用模式）
/// - 无 .chain → 按所选模式初始化（建 nodes + 示例节点 + 分析模式写 AI 指南 + 写标签）
/// - 已有 .chain 无标签（旧工程）→ 按所选模式补签
#[command]
pub fn add_workspace(dir: String, mode: String, app: AppHandle) -> Result<Vec<WorkspaceInfo>, String> {
    let scan_mode = ScanMode::from_str(&mode);
    let root = PathBuf::from(&dir);
    let canonical = root.canonicalize().map_err(|e| format!("目录不可访问：{e}"))?;
    let chain_dir = canonical.join(".chain");

    if let Some(tag) = read_mode_tag(&canonical) {
        if tag != scan_mode {
            return Err(format!(
                "该文件夹已是「{}模式」工作区，不能添加为「{}模式」（请切换到对应模式后再添加）",
                mode_label(tag),
                mode_label(scan_mode),
            ));
        }
    }

    if !chain_dir.exists() {
        crate::commands::init_chain::init_chain(
            canonical.to_string_lossy().into_owned(),
            Some(mode.clone()),
        )?;
    } else {
        // 旧工作区补签（v2.1 起所有工作区必须有标签）
        write_mode_tag(&canonical, scan_mode)?;
    }

    let p = config_path(&app)?;
    let mut list = read_workspaces(&p);
    let path_str = canonical.to_string_lossy().into_owned();
    if !list.iter().any(|w| w.path == path_str) {
        let name = canonical
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| path_str.clone());
        list.push(WorkspaceInfo {
            path: path_str,
            mode: mode_to_str(scan_mode).to_string(),
            name,
        });
    }
    write_workspaces(&p, &list)?;
    Ok(list)
}

/// 从工作区列表移除（仅移除记录，绝不删除磁盘上的任何文件）
#[command]
pub fn remove_workspace(dir: String, app: AppHandle) -> Result<Vec<WorkspaceInfo>, String> {
    let p = config_path(&app)?;
    let mut list = read_workspaces(&p);
    list.retain(|w| w.path != dir);
    write_workspaces(&p, &list)?;
    Ok(list)
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
        assert!(check_mode(root, ScanMode::Analysis).is_err(), "开发工作区不应允许分析操作");
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
            WorkspaceInfo { path: "G:/a".into(), mode: "analysis".into(), name: "a".into() },
            WorkspaceInfo { path: "G:/b".into(), mode: "dev".into(), name: "b".into() },
        ];
        write_workspaces(&p, &list).unwrap();
        let back = read_workspaces(&p);
        assert_eq!(back.len(), 2);
        assert_eq!(back[1].mode, "dev");
        // 空文件/缺失 → 空列表
        let tmp2 = TempDir::new().unwrap();
        assert!(read_workspaces(&tmp2.path().join("none.json")).is_empty());
    }
}
