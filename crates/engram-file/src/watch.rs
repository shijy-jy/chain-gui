//! watcher 纯逻辑（与 Tauri 解耦，可测试）：
//! 事件循环回调构造（build_watch_callback）+ 重扫发射（rescan_and_emit）。
//! WatchState 管理与 AppHandle 发射留在 GUI 侧。

use crate::model::ScanMode;
use crate::scanner::walker::scan_chain_dir_mode;
use notify::{EventKind, Watcher};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub enum RescanResult {
    Ok(Box<crate::model::chain::ChainSnapshot>),
    Err(String),
}

/// 重扫并 emit 的纯逻辑（可测试，不依赖 AppHandle）
pub fn rescan_and_emit<F>(dir: &std::path::Path, mode: ScanMode, emit_fn: F)
where
    F: Fn(RescanResult),
{
    match scan_chain_dir_mode(dir, mode) {
        Ok(snapshot) => emit_fn(RescanResult::Ok(Box::new(snapshot))),
        Err(e) => emit_fn(RescanResult::Err(e.to_string())),
    }
}

/// 事件循环回调（与 Tauri 解耦，可测试）：只关心 nodes/ 下 .md 的 Modify/Create/Remove，
/// 300ms 去抖后按当前模式重扫并交给 emit 回调。
pub fn build_watch_callback<F>(
    scan_dir: PathBuf,
    mode: Arc<Mutex<ScanMode>>,
    emit: F,
) -> impl FnMut(notify::Result<notify::Event>)
where
    F: Fn(RescanResult) + Send + 'static,
{
    let last_fire = Arc::new(Mutex::new(Instant::now() - Duration::from_secs(1)));
    move |res: Result<notify::Event, notify::Error>| {
        let Ok(event) = res else { return };
        if !matches!(
            event.kind,
            EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
        ) {
            return;
        }
        if !event
            .paths
            .iter()
            .any(|p| p.extension().is_some_and(|e| e == "md"))
        {
            return;
        }
        // 300ms 去抖
        {
            let Ok(mut last) = last_fire.lock() else {
                return;
            };
            if last.elapsed() < Duration::from_millis(300) {
                return;
            }
            *last = Instant::now();
        }
        // 按当前模式重扫并推送
        let Ok(mode_guard) = mode.lock() else { return };
        let scan_mode = *mode_guard;
        drop(mode_guard);
        rescan_and_emit(&scan_dir, scan_mode, &emit);
    }
}

/// 创建链目录的 notify watcher（与发射解耦，GUI/测试共用）：
/// - `.chain/nodes/` 非递归（节点文件平铺）
/// - `.chain/archive/` 递归（M7'：直接归档 <id>.md + fold 子目录，GUI 需感知 MCP 侧归档动作）
/// 不监听 .chain 根——stats.json/index 等派生物写盘不触发重扫风暴。
pub fn create_nodes_watcher<F>(
    root: &std::path::Path,
    callback: F,
) -> Result<notify::RecommendedWatcher, String>
where
    F: FnMut(notify::Result<notify::Event>) + Send + 'static,
{
    let nodes_dir = root.join(".chain").join("nodes");
    let archive_dir = root.join(".chain").join("archive");
    std::fs::create_dir_all(&archive_dir).map_err(|e| format!("创建归档目录失败：{e}"))?;
    let mut watcher =
        notify::recommended_watcher(callback).map_err(|e| format!("创建 watcher 失败：{e}"))?;
    watcher
        .watch(&nodes_dir, notify::RecursiveMode::NonRecursive)
        .map_err(|e| format!("监听 nodes 失败：{e}"))?;
    watcher
        .watch(&archive_dir, notify::RecursiveMode::Recursive)
        .map_err(|e| format!("监听 archive 失败：{e}"))?;
    Ok(watcher)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};
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
        rescan_and_emit(tmp.path(), ScanMode::Analysis, move |result| match result {
            RescanResult::Ok(snap) => {
                assert_eq!(snap.nodes.len(), 1);
                fc.fetch_add(1, Ordering::SeqCst);
            }
            RescanResult::Err(e) => panic!("重扫失败: {}", e),
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

    /// 事件循环真实测试（补盲区）：真实 notify watcher + 真实文件写入 →
    /// build_watch_callback 应在 2s 内触发重扫（验收判据「GUI 2 秒内刷新」的自动化等价）。
    #[test]
    fn test_watch_event_loop_picks_up_new_md_file() {
        let tmp = setup_chain();
        let nodes_dir = tmp.path().join(".chain").join("nodes");
        let (tx, rx) = std::sync::mpsc::channel::<usize>();

        let mode = Arc::new(Mutex::new(ScanMode::Analysis));
        let callback = build_watch_callback(tmp.path().to_path_buf(), mode, move |result| {
            if let RescanResult::Ok(snap) = result {
                let _ = tx.send(snap.nodes.len());
            }
        });
        let _watcher = create_nodes_watcher(tmp.path(), callback).expect("watcher 创建失败");

        // 写入新节点文件（真实文件事件）
        fs::write(
            nodes_dir.join("t-001.md"),
            "---\nid: t-001\ntype: task\ntitle: 任务1\nparent: g-001\nstatus: pending\ncreated: 2026-08-13T10:00:00+08:00\nupdated: 2026-08-13T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# 任务1\n",
        )
        .unwrap();

        let got = rx
            .recv_timeout(Duration::from_secs(2))
            .expect("2 秒内应收到 watcher 事件（事件循环盲区回归）");
        assert_eq!(got, 2, "重扫应发现 2 个节点（初始 g-001 + 新增 t-001）");
    }

    /// M7'：archive 目录事件同样触发重扫（GUI 感知 MCP 侧 archive_node 动作）
    #[test]
    fn test_watch_event_loop_picks_up_archive_file() {
        let tmp = setup_chain();
        let archive_dir = tmp.path().join(".chain").join("archive");
        fs::create_dir_all(&archive_dir).unwrap();
        let (tx, rx) = std::sync::mpsc::channel::<usize>();

        let mode = Arc::new(Mutex::new(ScanMode::Analysis));
        let callback = build_watch_callback(tmp.path().to_path_buf(), mode, move |result| {
            if let RescanResult::Ok(snap) = result {
                let _ = tx.send(snap.archived.len());
            }
        });
        let _watcher = create_nodes_watcher(tmp.path(), callback).expect("watcher 创建失败");

        // 写入归档节点文件（真实文件事件，位于 archive/ 子目录）
        let sub = archive_dir.join("fold_x");
        fs::create_dir_all(&sub).unwrap();
        fs::write(
            sub.join("arch-1.md"),
            "---\nid: arch-1\ntype: note\ntitle: '[归档]旧节点'\nparent: null\nstatus: none\ncreated: 2026-08-13T10:00:00+08:00\nupdated: 2026-08-13T10:00:00+08:00\nrevision: 1\ntags: []\narchived: true\n---\n\n# 旧节点\n",
        )
        .unwrap();

        let got = rx
            .recv_timeout(Duration::from_secs(2))
            .expect("archive 目录事件应在 2 秒内触发重扫");
        assert_eq!(got, 1, "归档列表应含新增归档节点");
    }
}
