//! Engram GUI（薄壳适配器）：Tauri commands 全部委托 engram-core，
//! 本 crate 只做协议翻译与窗口副作用（watcher 发射、证据打开、工作区配置目录）。

mod commands;
mod watcher;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(watcher::WatchState {
            watcher: std::sync::Mutex::new(None),
            mode: std::sync::Arc::new(std::sync::Mutex::new(
                engram_core::model::ScanMode::Analysis,
            )),
            dir: std::sync::Mutex::new(None),
        })
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::scan_chain,
            commands::update_node,
            commands::init_chain,
            commands::get_ai_guide,
            commands::get_guide_version,
            commands::get_version_info,
            commands::append_log,
            commands::get_process_log,
            commands::snapshot_chain,
            commands::list_snapshots,
            commands::read_snapshot,
            commands::fold_chain,
            commands::open_evidence,
            commands::evidence_rel_path,
            commands::create_node,
            commands::delete_node,
            commands::set_parent,
            commands::list_workspaces,
            commands::add_workspace,
            commands::remove_workspace,
            commands::get_code_map,
            commands::reindex_embeddings,
            commands::attach_code_map,
            commands::sync_code_map,
            commands::detach_code_map,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
