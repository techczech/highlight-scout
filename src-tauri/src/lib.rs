mod commands;
mod config;
mod import;
mod import_log;
mod index;
mod models;
mod ocr;
mod qmd;
mod sync;

use std::sync::{Mutex, RwLock};
use rusqlite::Connection;
use tauri::{Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

pub struct AppState {
    pub db: Mutex<Connection>,
    /// Live config so settings changes take effect without a restart.
    pub config: RwLock<config::Config>,
    /// Guard: true while a scheduled sync run is in progress. Prevents overlapping scheduled runs.
    pub is_syncing: std::sync::atomic::AtomicBool,
    /// Guard: true while an OCR batch run is in progress. Prevents overlapping OCR runs.
    pub is_ocring: std::sync::atomic::AtomicBool,
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
            is_syncing: std::sync::atomic::AtomicBool::new(false),
            is_ocring: std::sync::atomic::AtomicBool::new(false),
        })
        .invoke_handler(tauri::generate_handler![
            commands::search::search_query,
            commands::search::semantic_search,
            commands::search::find_related,
            commands::search::get_highlight,
            commands::search::qmd_available,
            commands::search::qmd_reindex,
            commands::search::ocr_images,
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
            commands::import::import_readwise_tweets,
            commands::import::import_json,
            commands::import::export_json,
            commands::import::get_import_log,
            commands::import::get_config,
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::set_autostart,
            commands::clipboard::copy_image,
        ])
        .setup(move |app| {
            let app_handle = app.handle().clone();
            let shortcut_str = shortcut.clone();

            use tauri_plugin_global_shortcut::GlobalShortcutExt;
            app_handle
                .global_shortcut()
                .on_shortcut(shortcut_str.as_str(), move |app, _shortcut, _event| {
                    // Summon-only: always bring the main window to the front
                    // (never hide — the main window stays open while the app runs).
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.unminimize();
                        let _ = window.set_focus();
                    }
                })
                .unwrap_or_else(|e| eprintln!("Failed to register shortcut: {}", e));

            // Launch-at-login follows the user's setting (default off for new installs).
            use tauri_plugin_autostart::ManagerExt;
            let autostart = app.autolaunch();
            let want = { app.state::<AppState>().config().autostart_enabled };
            let is_on = autostart.is_enabled().unwrap_or(false);
            if want && !is_on { let _ = autostart.enable(); }
            if !want && is_on { let _ = autostart.disable(); }

            // Show the main window on first launch (config has visible:false).
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }

            // In-app sync scheduler: tick every 5 min, run due sources sequentially.
            // Uses AppHandle to avoid holding a State guard across await boundaries.
            let sched_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut tick = tokio::time::interval(std::time::Duration::from_secs(300));
                loop {
                    tick.tick().await;
                    let Some(window) = sched_handle.get_webview_window("main") else { continue };
                    let state = sched_handle.state::<AppState>();
                    if state.is_syncing.load(std::sync::atomic::Ordering::SeqCst) { continue; }
                    // Snapshot config synchronously before any await.
                    let cfg = state.config();
                    let now = chrono::Utc::now();
                    // Drop the state borrow before entering the await loop.
                    drop(state);
                    for id in crate::sync::SCHEDULABLE {
                        if crate::sync::is_due(id, &cfg, now) {
                            // Re-fetch state to set the flag; drop before awaiting.
                            {
                                let state = sched_handle.state::<AppState>();
                                state.is_syncing.store(true, std::sync::atomic::Ordering::SeqCst);
                            }
                            let _ = crate::sync::run_source(id, &sched_handle, window.clone()).await;
                            {
                                let state = sched_handle.state::<AppState>();
                                state.is_syncing.store(false, std::sync::atomic::Ordering::SeqCst);
                            }
                        }
                    }
                }
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the main window quits the app — the main window always
            // stays open while the app runs (never hidden). Secondary windows
            // (work / related pop-outs) close normally via Cmd-W or their close
            // button. Esc never closes any window (handled in the frontend).
            if let WindowEvent::CloseRequested { .. } = event {
                if window.label() == "main" {
                    window.app_handle().exit(0);
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Highlight Scout");
}
