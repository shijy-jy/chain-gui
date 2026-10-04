//! 证据产物辅助（v1.8，纯逻辑）：
//! - `evidence_rel_path`：把文件选择器返回的绝对路径转成相对工程根的 evidence 路径（协议要求相对路径，统一 `/` 分隔）
//! - `resolve_evidence`：解析并校验证据相对路径，防路径穿越
//! - `is_view_only`：危险扩展名判断（只读查看徽标）
//!
//! 打开文件（ShellExecute/notepad）是平台副作用，留在 GUI 侧。
//! 协议不变：节点 evidence 字段仍存相对路径（见 AI_GUIDE §4.8）。

use std::path::{Path, PathBuf};

/// 把绝对路径转成相对工程根的 evidence 相对路径（统一用 `/` 分隔，与协议 §4.8 一致）。
/// 文件必须在工程目录内——evidence 是"相对工程根"的路径，选工程外的文件会被拒绝。
pub fn evidence_rel_path(dir: &str, abs: &str) -> Result<String, String> {
    let root = PathBuf::from(dir)
        .canonicalize()
        .map_err(|e| format!("工程目录不可访问：{e}"))?;
    let file = PathBuf::from(abs)
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

/// 双击会"执行/导入"的危险扩展名（Windows）：点击证据时改为记事本打开查看。
/// 与前端 Sidebar.svelte 的 VIEW_ONLY_EXTS 保持同步（前端仅用于显示"只读"徽标）。
const VIEW_ONLY_EXTS: &[&str] = &[
    "exe", "bat", "cmd", "com", "msi", "msp", "mst", "scr", "pif", "cpl", "msc", "reg", "vbs",
    "vbe", "js", "jse", "wsf", "wsh", "hta", "ps1", "psm1", "psd1", "py", "pyw", "pyc", "jar",
    "rb", "sh", "lnk", "chm", "dll", "sys", "ocx", "drv",
];

/// 该文件是否属于"只能看不能跑"的类型（按扩展名，不区分大小写）
pub fn is_view_only(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| VIEW_ONLY_EXTS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup() -> TempDir {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join("artifacts")).unwrap();
        tmp
    }

    #[test]
    fn test_evidence_rel_path_roundtrip() {
        let tmp = setup();
        let file = tmp.path().join("artifacts").join("a.png");
        fs::write(&file, "x").unwrap();
        let rel = evidence_rel_path(tmp.path().to_str().unwrap(), file.to_str().unwrap()).unwrap();
        assert_eq!(rel, "artifacts/a.png");
    }

    #[test]
    fn test_evidence_rel_path_rejects_outside() {
        let tmp = setup();
        let outside = TempDir::new().unwrap();
        let file = outside.path().join("secret.txt");
        fs::write(&file, "x").unwrap();
        let res = evidence_rel_path(tmp.path().to_str().unwrap(), file.to_str().unwrap());
        assert!(res.is_err(), "工程外文件应被拒绝");
    }

    #[test]
    fn test_resolve_evidence_rejects_traversal() {
        let root = TempDir::new().unwrap();
        let secret = TempDir::new().unwrap();
        fs::write(secret.path().join("secret.txt"), "x").unwrap();
        // 相对路径穿越（先构造一个绝对越界路径再改相对）——直接传越界的相对路径无法构造于临时目录间，
        // 用绝对路径拼接测试 resolve 的越界拒绝
        let joined = root
            .path()
            .join(secret.path().join("secret.txt").to_str().unwrap());
        let res = resolve_evidence(
            root.path(),
            secret.path().join("secret.txt").to_str().unwrap(),
        );
        assert!(res.is_err(), "路径穿越应被拒绝: {:?}", joined);
        // 不存在文件
        assert!(resolve_evidence(root.path(), "artifacts/none.png").is_err());
    }

    #[test]
    fn test_is_view_only() {
        assert!(is_view_only(Path::new("x.ps1")));
        assert!(is_view_only(Path::new("x.PY")));
        assert!(!is_view_only(Path::new("x.png")));
        assert!(!is_view_only(Path::new("x.md")));
    }
}
