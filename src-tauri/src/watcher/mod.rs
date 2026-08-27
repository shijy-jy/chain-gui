use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use crate::model::ScanMode;

/// 全局 watcher 状态（同一时间只监听一个目录）+ 当前扫描模式（v2.0 双模式）
pub struct WatchState {
    pub watcher: Mutex<Option<RecommendedWatcher>>,
    pub mode: Arc<Mutex<ScanMode>>,
    pub dir: Mutex<Option<PathBuf>>,
}

/// 重扫并 emit 的纯逻辑（可测试，不依赖 AppHandle）
pub fn rescan_and_emit<F>(dir: &std::path::Path, mode: ScanMode, emit_fn: F)
where
    F: Fn(RescanResult),
{
    use crate::scanner::walker::scan_chain_dir_mode;
    match scan_chain_dir_mode(dir, mode) {
        Ok(snapshot) => emit_fn(RescanResult::Ok(snapshot)),
        Err(e) => emit_fn(RescanResult::Err(e.to_string())),
    }
}

pub enum RescanResult {
    Ok(crate::model::chain::ChainSnapshot),
    Err(String),
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

    let last_fire = std::sync::Arc::new(Mutex::new(Instant::now() - Duration::from_secs(1)));
    let scan_dir = dir.clone();

    let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
        let Ok(event) = res else { return };
        if !matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)) {
            return;
        }
        if !event.paths.iter().any(|p| p.extension().is_some_and(|e| e == "md")) {
            return;
        }
        // 300ms 去抖
        {
            let Ok(mut last) = last_fire.lock() else { return };
            if last.elapsed() < Duration::from_millis(300) { return; }
            *last = Instant::now();
        }
        // 按当前模式重扫并推送
        let Ok(mode_guard) = mode.lock() else { return };
        let scan_mode = *mode_guard;
        drop(mode_guard);
        rescan_and_emit(&scan_dir, scan_mode, |result| match result {
            RescanResult::Ok(snapshot) => { let _ = app.emit("chain-changed", &snapshot); }
            RescanResult::Err(e) => { let _ = app.emit("chain-error", e); }
        });
    }).map_err(|e| format!("创建 watcher 失败：{e}"))?;

    watcher.watch(&nodes_dir, RecursiveMode::NonRecursive)
        .map_err(|e| format!("监听失败：{e}"))?;

    let mut guard = state.watcher.lock().map_err(|e| e.to_string())?;
    *guard = Some(watcher);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tempfile::TempDir;

    fn setup_chain() -> TempDir {
        let tmp = TempDir::new().unwrap();
        let nodes_dir = tmp.path().join(".chain").join("nodes");
        fs::create_dir_all(&nodes_dir).unwrap();
        fs::write(
            nodes_dir.join("g-001.md"),
            "---\nid: g-001\ntype: goal\ntitle: 顶层目标\nparent: null\nstatus: pending\ncreated: 2026-08-13T10:00:00+08:00\nupdated: 2026-08-13T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# 顶层目标\n",
        ).unwrap();
        tmp
    }

    #[test]
    fn test_rescan_and_emit_on_md_change() {
        let tmp = setup_chain();
        let fire_count = Arc::new(AtomicUsize::new(0));
        let fc = fire_count.clone();

        // 直接测试 rescan_and_emit 纯函数
        rescan_and_emit(tmp.path(), ScanMode::Analysis, move |result| {
            match result {
                RescanResult::Ok(snap) => {
                    assert_eq!(snap.nodes.len(), 1);
                    fc.fetch_add(1, Ordering::SeqCst);
                }
                RescanResult::Err(e) => panic!("重扫失败: {}", e),
            }
        });
        assert_eq!(fire_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_rescan_picks_up_new_file() {
        let tmp = setup_chain();
        // 加一个新节点
        let nodes_dir = tmp.path().join(".chain").join("nodes");
        fs::write(
            nodes_dir.join("d-001.md"),
            "---\nid: d-001\ntype: design\ntitle: 设计1\nparent: g-001\nstatus: in_progress\ncreated: 2026-08-13T10:00:00+08:00\nupdated: 2026-08-13T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# 设计1\n",
        ).unwrap();

        let got_nodes = std::cell::Cell::new(0usize);
        rescan_and_emit(tmp.path(), ScanMode::Analysis, |result| {
            if let RescanResult::Ok(snap) = result {
                got_nodes.set(snap.nodes.len());
            }
        });
        assert_eq!(got_nodes.get(), 2, "重扫应发现 2 个节点");
    }

    #[test]
    fn test_rescan_error_on_no_chain_dir() {
        let tmp = TempDir::new().unwrap();
        let got_err = std::cell::Cell::new(false);
        rescan_and_emit(tmp.path(), ScanMode::Analysis, |result| {
            if let RescanResult::Err(_) = result {
                got_err.set(true);
            }
        });
        assert!(got_err.get(), "无 .chain 目录应报错");
    }
}
