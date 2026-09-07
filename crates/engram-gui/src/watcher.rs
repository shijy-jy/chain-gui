//! 全局 watcher 状态（同一时间只监听一个目录）+ 当前扫描模式（v2.0 双模式）。
//! 回调构造与重扫逻辑在 core::watch；本模块只持有句柄并做 Tauri 发射。

use engram_core::model::ScanMode;
use engram_core::watch::{build_watch_callback, create_nodes_watcher, RescanResult};
use notify::RecommendedWatcher;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

pub struct WatchState {
    pub watcher: Mutex<Option<RecommendedWatcher>>,
    pub mode: Arc<Mutex<ScanMode>>,
    pub dir: Mutex<Option<PathBuf>>,
}

/// 启动/重启对 dir\.chain\nodes 的监听。重复调用安全：旧 watcher 被 drop 后重建。
/// 回调里的重扫使用 state.mode 的当前值（模式切换后文件变化按新模式解析）。
pub fn start_watch(
    dir: PathBuf,
    app: AppHandle,
    state: &WatchState,
    mode: Arc<Mutex<ScanMode>>,
) -> Result<(), String> {
    let nodes_dir = dir.join(".chain").join("nodes");
    if !nodes_dir.is_dir() {
        return Err(format!("nodes 目录不存在：{}", nodes_dir.display()));
    }

    {
        let mut dir_guard = state.dir.lock().map_err(|e| e.to_string())?;
        *dir_guard = Some(dir.clone());
    }

    // 事件回调与 Tauri 解耦：build_watch_callback 纯逻辑可脱离 AppHandle 测试（补事件循环盲区）
    let callback = build_watch_callback(dir.clone(), mode, move |result| match result {
        RescanResult::Ok(snapshot) => {
            let _ = app.emit("chain-changed", &*snapshot);
        }
        RescanResult::Err(e) => {
            let _ = app.emit("chain-error", e);
        }
    });

    let watcher = create_nodes_watcher(&nodes_dir, callback)?;

    let mut guard = state.watcher.lock().map_err(|e| e.to_string())?;
    *guard = Some(watcher);
    Ok(())
}
