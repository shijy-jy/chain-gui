pub mod model;
pub mod scanner;
pub mod commands;
pub mod watcher;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_dialog::init())
    .manage(crate::watcher::WatchState(std::sync::Mutex::new(None)))
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
        commands::scan_chain::scan_chain,
        commands::update_node::update_node,
        commands::init_chain::init_chain,
        commands::ai_guide::get_ai_guide,
        commands::ai_guide::get_guide_version,
        commands::process_log::append_log,
        commands::process_log::get_process_log,
        commands::snapshot::snapshot_chain,
        commands::snapshot::list_snapshots,
        commands::snapshot::read_snapshot,
        commands::fold::fold_chain,
        commands::evidence::open_evidence,
        commands::evidence::evidence_rel_path,
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
