//! 证据产物辅助命令（v1.8）：
//! - `evidence_rel_path`：把文件选择器返回的绝对路径转成相对工程根的 evidence 路径（协议要求相对路径，统一 `/` 分隔）
//! - `open_evidence`：用系统默认程序打开证据文件（界面只显示文件名，点击即打开）
//! 协议不变：节点 evidence 字段仍存相对路径（见 AI_GUIDE §4.8）。

use std::path::{Path, PathBuf};
use tauri::command;

/// 把绝对路径转成相对工程根的 evidence 相对路径（统一用 `/` 分隔，与协议 §4.8 一致）。
/// 文件必须在工程目录内——evidence 是"相对工程根"的路径，选工程外的文件会被拒绝。
#[command]
pub fn evidence_rel_path(dir: String, abs: String) -> Result<String, String> {
    let root = PathBuf::from(&dir)
        .canonicalize()
        .map_err(|e| format!("工程目录不可访问：{e}"))?;
    let file = PathBuf::from(&abs)
        .canonicalize()
        .map_err(|e| format!("文件不可访问：{e}"))?;
    let rel = file
        .strip_prefix(&root)
        .map_err(|_| "证据文件需位于工程目录内（evidence 是相对工程根的路径）".to_string())?;
    Ok(rel.to_string_lossy().replace('\\', "/"))
}

/// 解析并校验证据相对路径，防路径穿越（不允许打开工程目录外的文件）。
/// 注意：校验用 canonicalize（能解符号链接/穿越），但**返回普通形态的绝对路径**——
/// Windows 上 canonicalize 会返回 `\\?\` verbatim 路径，ShellExecute（cmd start / explorer）打不开它，
/// 这是 v1.8 首版"证据文件存在却打不开"的根因。
pub fn resolve_evidence(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let canon_root = root
        .canonicalize()
        .map_err(|e| format!("工程目录不可访问：{e}"))?;
    let joined = root.join(rel);
    let canon = joined
        .canonicalize()
        .map_err(|e| format!("证据文件不存在或不可访问：{rel}（{e}）"))?;
    if !canon.starts_with(&canon_root) {
        return Err("证据路径越界（不允许打开工程目录外的文件）".into());
    }
    Ok(joined)
}

/// 用系统默认程序打开证据文件
#[command]
pub fn open_evidence(dir: String, rel: String) -> Result<(), String> {
    let root = PathBuf::from(&dir);
    let target = resolve_evidence(&root, &rel)?;
    open_with_default_app(&target)
}

/// 用系统默认程序打开文件（Windows: explorer.exe 单参直传，无 cmd 元字符重解析且支持中文/空格路径；
/// macOS: open；Linux: xdg-open）。路径必须是普通形态（非 `\\?\` verbatim——ShellExecute 不接受）。
fn open_with_default_app(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer.exe")
            .arg(path)
            .spawn()
            .map_err(|e| format!("打开失败：{e}"))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|e| format!("打开失败：{e}"))?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|e| format!("打开失败：{e}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_rel_path_inside_normalizes_slashes() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("artifacts/t-001")).unwrap();
        let file = root.join("artifacts/t-001/截图.png");
        fs::write(&file, b"x").unwrap();

        let rel = evidence_rel_path(
            root.to_string_lossy().into_owned(),
            file.to_string_lossy().into_owned(),
        )
        .unwrap();
        assert_eq!(rel, "artifacts/t-001/截图.png");
    }

    #[test]
    fn test_rel_path_outside_rejected() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("proj");
        fs::create_dir_all(&root).unwrap();
        let outside = tmp.path().join("外部文件.png");
        fs::write(&outside, b"x").unwrap();

        let res = evidence_rel_path(
            root.to_string_lossy().into_owned(),
            outside.to_string_lossy().into_owned(),
        );
        assert!(res.is_err(), "工程外文件应被拒绝: {res:?}");
    }

    #[test]
    fn test_resolve_rejects_traversal_and_missing() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("proj");
        fs::create_dir_all(&root).unwrap();
        // 工程外的兄弟文件存在，`../` 穿越必须被拦截
        let outside = tmp.path().join("secret.txt");
        fs::write(&outside, b"s").unwrap();

        assert!(resolve_evidence(&root, "../secret.txt").is_err(), "路径穿越应被拒绝");
        assert!(resolve_evidence(&root, "artifacts/none.png").is_err(), "不存在文件应报错");
    }

    #[test]
    fn test_resolve_accepts_inside() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let file = root.join("artifacts/t-001/报告.png");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, b"x").unwrap();

        let resolved = resolve_evidence(root, "artifacts/t-001/报告.png").unwrap();
        assert!(resolved.ends_with("报告.png"));
    }

    #[test]
    fn test_resolve_returns_normal_path_not_verbatim() {
        // 回归测试（v1.8 证据打不开根因）：canonicalize 在 Windows 返回 \\?\ verbatim 路径，
        // ShellExecute 打不开它；resolve_evidence 必须返回普通形态路径。
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let file = root.join("artifacts/t-001/截图.png");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, b"x").unwrap();

        let resolved = resolve_evidence(root, "artifacts/t-001/截图.png").unwrap();
        let s = resolved.to_string_lossy();
        assert!(
            !s.starts_with(r"\\?\"),
            "不能返回 verbatim 路径（ShellExecute 打不开）: {s}"
        );
        assert!(resolved.exists(), "返回的路径应真实存在: {s}");
    }
}
