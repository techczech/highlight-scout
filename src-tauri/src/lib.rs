mod commands;
mod config;
mod import;
mod import_log;
mod index;
mod models;
mod qmd;

use std::sync::{Mutex, RwLock};
use rusqlite::Connection;
use tauri::{Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

pub struct AppState {
    pub db: Mutex<Connection>,
    /// Live config so settings changes take effect without a restart.
    pub config: RwLock<config::Config>,
}

impl AppState {
    /// Snapshot the current config for read-only use inside a command.
    pub fn config(&self) -> config::Config {
        self.config.read().expect("config lock poisoned").clone()
    }
}

/// Headless import: `highlight-scout --import-x <saved.jsonl>` imports X saved
/// tweets into the configured archive + index without launching the GUI. Reuses
/// the same import/archive/index code paths as the in-app importer.
fn headless_import_x(path: &str) {
    use std::collections::HashMap;
    let cfg = config::load();
    let index_path = config::index_path();
    if let Some(parent) = index_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = index::sqlite::open(&index_path).expect("open index");
    index::sqlite::init_schema(&conn).expect("init schema");
    let (works, hls) = import::x::import(path).expect("parse saved.jsonl");
    let mut by_work: HashMap<String, Vec<&models::Highlight>> = HashMap::new();
    for (h, _, _) in &hls {
        by_work.entry(h.work_id.clone()).or_default().push(h);
    }
    import::archive::write_archive(&cfg.archive_path, &works, &by_work).expect("write archive");
    for w in &works {
        index::sqlite::upsert_work(&conn, w).expect("upsert work");
    }
    for (h, title, author) in &hls {
        index::sqlite::upsert_highlight(&conn, h, title, author.as_deref()).expect("upsert highlight");
    }
    println!("Imported {} tweet works, {} highlights from {}", works.len(), hls.len(), path);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--import-x") {
        headless_import_x(args.get(i + 1).map(|s| s.as_str()).unwrap_or(""));
        return;
    }

    let cfg = config::load();
    let index_path = config::index_path();

    // Ensure index directory exists
    if let Some(parent) = index_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let conn = index::sqlite::open(&index_path).expect("Failed to open SQLite index");
    index::sqlite::init_schema(&conn).expect("Failed to initialise schema");

    let shortcut = cfg.shortcut.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            db: Mutex::new(conn),
            config: RwLock::new(cfg),
        })
        .invoke_handler(tauri::generate_handler![
            commands::search::search_query,
            commands::search::semantic_search,
            commands::search::find_related,
            commands::search::get_highlight,
            commands::search::qmd_available,
            commands::search::qmd_reindex,
            commands::search::work_highlights,
            commands::search::highlight_position,
            commands::search::list_tags,
            commands::search::get_facets,
            commands::search::get_stats,
            commands::import::run_import,
            commands::import::run_readwise_seed,
            commands::import::run_zotero_import,
            commands::import::inspect_csv,
            commands::import::import_csv,
            commands::import::import_kindle,
            commands::import::import_x,
            commands::import::import_json,
            commands::import::export_json,
            commands::import::get_import_log,
            commands::import::get_config,
            commands::settings::get_settings,
            commands::settings::save_settings,
        ])
        .setup(move |app| {
            let app_handle = app.handle().clone();
            let shortcut_str = shortcut.clone();

            use tauri_plugin_global_shortcut::GlobalShortcutExt;
            app_handle
                .global_shortcut()
                .on_shortcut(shortcut_str.as_str(), move |app, _shortcut, _event| {
                    if let Some(window) = app.get_webview_window("main") {
                        let visible = window.is_visible().unwrap_or(false);
                        if visible {
                            let _ = window.hide();
                        } else {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .unwrap_or_else(|e| eprintln!("Failed to register shortcut: {}", e));

            // Enable launch-at-login (ADR-0007 MVP: macOS Login Item).
            use tauri_plugin_autostart::ManagerExt;
            let autostart = app.autolaunch();
            if let Ok(false) = autostart.is_enabled() {
                let _ = autostart.enable();
            }

            // Show the main window on first launch (config has visible:false).
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            // Intercept the close button: hide instead of quitting, so the
            // daemon stays resident for the global hotkey. (No hide-on-blur —
            // the window stays put until the hotkey toggles or it is closed.)
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Highlight Scout");
}
