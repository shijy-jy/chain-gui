use tauri::command;
use std::path::PathBuf;
use crate::model::ScanMode;
use crate::scanner::walker::scan_chain_dir_mode;
use crate::model::chain::ChainSnapshot;

#[command]
pub fn scan_chain(
    dir: String,
    mode: Option<String>,
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::watcher::WatchState>,
) -> Result<ChainSnapshot, String> {
    let scan_mode = mode.as_deref().map(ScanMode::from_str).unwrap_or(ScanMode::Analysis);
    let path = PathBuf::from(&dir);
    // v2.1 模式强绑定：文件夹标签与请求模式不符 → 拒绝（防混用）
    crate::commands::workspace::check_mode(&path, scan_mode)?;
    let snapshot = scan_chain_dir_mode(&path, scan_mode).map_err(|e| e.to_string())?;
    // 启动/切换监听；失败不阻塞扫描结果（只告警）。
    // 监听回调重扫使用 state.mode 的当前值（v2.0 模式切换后文件变化按新模式解析）
    {
        let mut mode_guard = state.mode.lock().map_err(|e| e.to_string())?;
        *mode_guard = scan_mode;
    }
    let mode_arc = state.mode.clone();
    if let Err(e) = crate::watcher::start_watch(path, app, &state, mode_arc) {
        eprintln!("[chain-gui] watcher 启动失败：{e}");
    }
    Ok(snapshot)
}
