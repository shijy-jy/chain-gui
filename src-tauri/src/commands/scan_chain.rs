use tauri::command;
use std::path::PathBuf;
use crate::scanner::walker::scan_chain_dir;
use crate::model::chain::ChainSnapshot;

#[command]
pub fn scan_chain(
    dir: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::watcher::WatchState>,
) -> Result<ChainSnapshot, String> {
    let path = PathBuf::from(&dir);
    let snapshot = scan_chain_dir(&path).map_err(|e| e.to_string())?;
    // 启动/切换监听；失败不阻塞扫描结果（只告警）
    if let Err(e) = crate::watcher::start_watch(path, app, &state) {
        eprintln!("[chain-gui] watcher 启动失败：{e}");
    }
    Ok(snapshot)
}
