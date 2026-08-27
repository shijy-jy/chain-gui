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

/// 用系统默认程序打开证据文件；危险扩展名强制"只读查看"（记事本），绝不执行。
/// 证据是产物（截图/日志/文档），不是可执行体——误点 .py/.bat/.reg 可能造成破坏（v1.8 安全策略）。
#[command]
pub fn open_evidence(dir: String, rel: String) -> Result<(), String> {
    let root = PathBuf::from(&dir);
    let target = resolve_evidence(&root, &rel)?;
    if is_view_only(&target) {
        open_with_notepad(&target)
    } else {
        open_with_default_app(&target)
    }
}

/// 双击会"执行/导入"的危险扩展名（Windows）：点击证据时改为记事本打开查看。
/// 与前端 Sidebar.svelte 的 VIEW_ONLY_EXTS 保持同步（前端仅用于显示"只读"徽标）。
const VIEW_ONLY_EXTS: &[&str] = &[
    "exe", "bat", "cmd", "com", "msi", "msp", "mst", "scr", "pif", "cpl", "msc",
    "reg", "vbs", "vbe", "js", "jse", "wsf", "wsh", "hta", "ps1", "psm1", "psd1",
    "py", "pyw", "pyc", "jar", "rb", "sh", "lnk", "chm", "dll", "sys", "ocx", "drv",
];

/// 该文件是否属于"只能看不能跑"的类型（按扩展名，不区分大小写）
pub fn is_view_only(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| VIEW_ONLY_EXTS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// 用记事本只读查看（不执行、不导入）
fn open_with_notepad(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("notepad.exe")
            .arg(path)
            .spawn()
            .map_err(|e| format!("用记事本打开失败：{e}"))?;
        Ok(())
    }
    // 非 Windows 平台：脚本/可执行文件由桌面关联处理（Linux/macOS 默认也是编辑器而非执行）
    #[cfg(not(target_os = "windows"))]
    {
        open_with_default_app(path)
    }
}

/// 用系统默认程序打开文件。
/// Windows：ShellExecuteW 直调——不经过 explorer.exe（它会复用已打开的 Explorer 窗口、
/// 有自己的命令行解析怪癖，v1.8 曾出现"跳到文档文件夹却不打开文件"）；
/// macOS：open；Linux：xdg-open。路径必须是普通形态（非 `\\?\` verbatim）。
fn open_with_default_app(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Foundation::HWND;
        use windows_sys::Win32::UI::Shell::ShellExecuteW;

        // UTF-16 宽字符 + 结尾 null
        let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
        wide.push(0);
        let op: Vec<u16> = "open".encode_utf16().chain(std::iter::once(0)).collect();

        // ShellExecuteW 返回值 > 32 = 成功；≤ 32 = SE_ERR_* 错误码
        let ret = unsafe {
            ShellExecuteW(
                0 as HWND,
                op.as_ptr(),
                wide.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1, // SW_SHOWNORMAL
            )
        };
        if (ret as u32) <= 32 {
            return Err(format!(
                "系统打开失败（{se}）：{}",
                path.display(),
                se = se_err_msg(ret as u32),
            ));
        }
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

/// ShellExecute 的 SE_ERR_* 错误码 → 可读说明（https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shellexecutea#return-value）
#[cfg(target_os = "windows")]
fn se_err_msg(code: u32) -> &'static str {
    match code {
        2 => "找不到指定文件（SE_ERR_FNF）",
        3 => "找不到指定路径（SE_ERR_PNF）",
        5 => "拒绝访问（SE_ERR_ACCESSDENIED）",
        8 => "内存不足（SE_ERR_OOM）",
        26 => "没有与该文件类型关联的默认程序（SE_ERR_NOASSOC）",
        27 => "找不到或无法加载关联的动态库（SE_ERR_DDLL）",
        28 => "关联程序未响应（SE_ERR_DDEBUS）",
        29 => "DDE 事务失败（SE_ERR_DDEFAIL）",
        30 => "文件正被其他程序占用（SE_ERR_SHARE）",
        31 => "文件类型关联无效（SE_ERR_ASSOCINCOMPLETE）",
        32 => "关联程序加载失败（SE_ERR_DLLNOTFOUND）",
        _ => "未知系统错误",
    }
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

    #[test]
    fn test_view_only_extensions() {
        // 双击会执行/导入的危险扩展名 → 只读查看（记事本），绝不运行
        for ext in ["py", "PY", "Py", "bat", "exe", "reg", "ps1", "js", "vbs", "lnk", "jar"] {
            assert!(is_view_only(Path::new(&format!("x.{ext}"))), "{ext} 应判定为只读查看");
        }
        // 普通产物 → 桌面双击方式
        for ext in ["md", "png", "txt", "log", "json", "csv", "pdf", "html"] {
            assert!(!is_view_only(Path::new(&format!("x.{ext}"))), "{ext} 应走默认打开");
        }
        // 无扩展名 → 默认打开
        assert!(!is_view_only(Path::new("README")));
    }
}
