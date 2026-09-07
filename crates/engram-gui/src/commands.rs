//! Tauri 命令薄壳（v2.8 重构）：全部委托 engram-core。
//! 本层仅保留窗口副作用：watcher 启动、证据文件系统打开、工作区配置目录（AppHandle）。
//! 命令签名与旧版完全一致（前端 invoke 契约不变，宪法第 8 条：工具即契约）。

use engram_core::model::chain::ChainSnapshot;
use engram_core::model::{ScanMode, UpdateFields};
use engram_core::ops::node_edit::CreateNodeInput;
use engram_core::scanner::walker::scan_chain_dir_mode;
use engram_core::workspace::WorkspaceInfo;
use std::path::{Path, PathBuf};
use tauri::{command, AppHandle, Manager, State};

// ── 扫描与 watcher ────────────────────────────────────────

#[command]
pub fn scan_chain(
    dir: String,
    mode: Option<String>,
    app: AppHandle,
    state: State<'_, crate::watcher::WatchState>,
) -> Result<ChainSnapshot, String> {
    let scan_mode = mode
        .as_deref()
        .map(ScanMode::parse_lenient)
        .unwrap_or(ScanMode::Analysis);
    let path = PathBuf::from(&dir);
    // v2.1 模式强绑定：文件夹标签与请求模式不符 → 拒绝（防混用）
    engram_core::workspace::check_mode(&path, scan_mode)?;
    let snapshot = scan_chain_dir_mode(&path, scan_mode).map_err(|e| e.to_string())?;
    // 启动/切换监听；失败不阻塞扫描结果（只告警）。
    // 监听回调重扫使用 state.mode 的当前值（v2.0 模式切换后文件变化按新模式解析）
    {
        let mut mode_guard = state.mode.lock().map_err(|e| e.to_string())?;
        *mode_guard = scan_mode;
    }
    let mode_arc = state.mode.clone();
    if let Err(e) = crate::watcher::start_watch(path, app, &state, mode_arc) {
        eprintln!("[engram] watcher 启动失败：{e}");
    }
    Ok(snapshot)
}

// ── 节点编辑 ──────────────────────────────────────────────

#[command]
pub fn update_node(
    dir: String,
    node_id: String,
    fields: UpdateFields,
    mode: Option<String>,
) -> Result<ChainSnapshot, String> {
    let scan_mode = mode
        .as_deref()
        .map(ScanMode::parse_lenient)
        .unwrap_or(ScanMode::Analysis);
    engram_core::ops::node_edit::update_node_fields(Path::new(&dir), &node_id, &fields, scan_mode)
}

#[command]
pub fn create_node(
    dir: String,
    input: CreateNodeInput,
    mode: Option<String>,
) -> Result<ChainSnapshot, String> {
    let scan_mode = mode
        .as_deref()
        .map(ScanMode::parse_lenient)
        .unwrap_or(ScanMode::Analysis);
    engram_core::ops::node_edit::create_node(Path::new(&dir), &input, scan_mode)
}

#[command]
pub fn delete_node(
    dir: String,
    node_id: String,
    mode: Option<String>,
) -> Result<ChainSnapshot, String> {
    let scan_mode = mode
        .as_deref()
        .map(ScanMode::parse_lenient)
        .unwrap_or(ScanMode::Analysis);
    engram_core::ops::node_edit::delete_node(Path::new(&dir), &node_id, scan_mode)
}

#[command]
pub fn set_parent(
    dir: String,
    node_id: String,
    parent: Option<String>,
    mode: Option<String>,
    rel: Option<String>,
) -> Result<ChainSnapshot, String> {
    let scan_mode = mode
        .as_deref()
        .map(ScanMode::parse_lenient)
        .unwrap_or(ScanMode::Analysis);
    engram_core::ops::node_edit::set_parent(Path::new(&dir), &node_id, parent, scan_mode, rel)
}

// ── 链级操作 ──────────────────────────────────────────────

#[command]
pub fn init_chain(dir: String, mode: Option<String>) -> Result<ChainSnapshot, String> {
    let scan_mode = mode
        .as_deref()
        .map(ScanMode::parse_lenient)
        .unwrap_or(ScanMode::Analysis);
    engram_core::ops::chain::init_chain(Path::new(&dir), scan_mode)
}

#[command]
pub fn fold_chain(
    dir: String,
    node_id: String,
    mode: Option<String>,
) -> Result<ChainSnapshot, String> {
    let scan_mode = mode
        .as_deref()
        .map(ScanMode::parse_lenient)
        .unwrap_or(ScanMode::Analysis);
    engram_core::ops::chain::fold_chain(Path::new(&dir), &node_id, scan_mode)
}

#[command]
pub fn snapshot_chain(dir: String, tag: String) -> Result<String, String> {
    engram_core::ops::chain::snapshot_chain(Path::new(&dir), &tag)
}

#[command]
pub fn list_snapshots(dir: String) -> Result<Vec<engram_core::model::chain::SnapshotMeta>, String> {
    engram_core::ops::chain::list_snapshots(Path::new(&dir))
}

#[command]
pub fn read_snapshot(dir: String, snap_id: String) -> Result<ChainSnapshot, String> {
    engram_core::ops::chain::read_snapshot(Path::new(&dir), &snap_id)
}

#[command]
pub fn append_log(dir: String, text: String) -> Result<String, String> {
    engram_core::ops::chain::append_log(Path::new(&dir), &text)
}

#[command]
pub fn get_process_log(dir: String) -> Result<String, String> {
    engram_core::ops::chain::get_process_log(Path::new(&dir))
}

// ── 指南 ──────────────────────────────────────────────────

/// 返回当前模式对应的 AI 使用指南全文（v2.1 双指南）
#[command]
pub fn get_ai_guide(mode: Option<String>) -> String {
    engram_core::guide::guide_for(mode.as_deref()).to_string()
}

/// 返回当前模式对应的内嵌指南版本号
#[command]
pub fn get_guide_version(mode: Option<String>) -> u32 {
    engram_core::guide::guide_version_for(mode.as_deref())
}

// ── 证据 ──────────────────────────────────────────────────

/// 把绝对路径转成相对工程根的 evidence 相对路径（协议要求相对路径，统一 `/` 分隔）
#[command]
pub fn evidence_rel_path(dir: String, abs: String) -> Result<String, String> {
    engram_core::evidence::evidence_rel_path(&dir, &abs)
}

/// 用系统默认程序打开证据文件；危险扩展名强制"只读查看"（记事本），绝不执行。
/// 证据是产物（截图/日志/文档），不是可执行体——误点 .py/.bat/.reg 可能造成破坏（v1.8 安全策略）。
#[command]
pub fn open_evidence(dir: String, rel: String) -> Result<(), String> {
    let root = PathBuf::from(&dir);
    let target = engram_core::evidence::resolve_evidence(&root, &rel)?;
    if engram_core::evidence::is_view_only(&target) {
        open_with_notepad(&target)
    } else {
        open_with_default_app(&target)
    }
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

// ── 工作区 ────────────────────────────────────────────────

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_config_dir()
        .map_err(|e| e.to_string())?
        .join("workspaces.json"))
}

/// 列出所有工作区；每次实时读文件夹标签重新归类（标签变了自动归到对应层）
#[command]
pub fn list_workspaces(app: AppHandle) -> Result<Vec<WorkspaceInfo>, String> {
    let p = config_path(&app)?;
    let mut list = engram_core::workspace::read_workspaces(&p);
    for ws in &mut list {
        if let Some(tag) = engram_core::workspace::read_mode_tag(Path::new(&ws.path)) {
            ws.mode = engram_core::profile::mode_str(tag).to_string();
        }
    }
    Ok(list)
}

/// 添加工作区：
/// - 文件夹已有标签且与所选模式不符 → 拒绝（不允许混用模式）
/// - 无 .chain → 按所选模式初始化（建 nodes + 示例节点 + 分析模式写 AI 指南 + 写标签）
/// - 已有 .chain 无标签（旧工程）→ 按所选模式补签
#[command]
pub fn add_workspace(
    dir: String,
    mode: String,
    app: AppHandle,
) -> Result<Vec<WorkspaceInfo>, String> {
    let scan_mode = ScanMode::parse_lenient(&mode);
    let canonical = engram_core::workspace::canonicalize_workspace_dir(&dir)?;
    let chain_dir = canonical.join(".chain");

    if let Some(tag) = engram_core::workspace::read_mode_tag(&canonical) {
        if tag != scan_mode {
            return Err(format!(
                "该文件夹已是「{}模式」工作区，不能添加为「{}模式」（请切换到对应模式后再添加）",
                engram_core::workspace::mode_label_str(tag),
                engram_core::workspace::mode_label_str(scan_mode),
            ));
        }
    }

    if !chain_dir.exists() {
        engram_core::ops::chain::init_chain(&canonical, scan_mode)?;
    } else {
        // 旧工作区补签（v2.1 起所有工作区必须有标签）
        engram_core::workspace::write_mode_tag(&canonical, scan_mode)?;
    }

    let p = config_path(&app)?;
    let mut list = engram_core::workspace::read_workspaces(&p);
    let mut path_str = canonical.to_string_lossy().into_owned();
    // canonicalize 在 Windows 返回 `\\?\` verbatim 前缀：剥掉，保证列表路径与用户所见一致
    path_str = engram_core::workspace::strip_verbatim_prefix(path_str);
    if !list.iter().any(|w| w.path == path_str) {
        let name = engram_core::workspace::workspace_name_from_path(&canonical);
        list.push(WorkspaceInfo {
            path: path_str,
            mode: engram_core::profile::mode_str(scan_mode).to_string(),
            name,
        });
    }
    engram_core::workspace::write_workspaces(&p, &list)?;
    Ok(list)
}

/// 从工作区列表移除（仅移除记录，绝不删除磁盘上的任何文件）
#[command]
pub fn remove_workspace(dir: String, app: AppHandle) -> Result<Vec<WorkspaceInfo>, String> {
    let p = config_path(&app)?;
    let mut list = engram_core::workspace::read_workspaces(&p);
    list.retain(|w| w.path != dir);
    engram_core::workspace::write_workspaces(&p, &list)?;
    Ok(list)
}
