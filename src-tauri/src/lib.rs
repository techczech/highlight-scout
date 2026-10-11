mod app_lock;
mod archive_meta;
mod busy;
mod commands;
mod config;
mod corpus;
mod corpus_copy;
mod frontmost;
mod http;
mod import;
mod import_log;
mod meaning;
mod models;
mod ocr;
mod r2;
mod sync;

use rusqlite::Connection;
use std::sync::{Mutex, RwLock};
use tauri::{Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

pub struct AppState {
    pub db: Mutex<Connection>,
    /// Live config so settings changes take effect without a restart.
    pub config: RwLock<config::Config>,
    /// One writer at a time: imports, the scheduled pass and the duplicate
    /// merge each hold a claim for their whole run (see `busy`).
    pub busy: busy::BusyLock,
    /// Guard: true while an OCR batch run is in progress. Prevents overlapping OCR runs.
    pub is_ocring: std::sync::atomic::AtomicBool,
    /// Guard: true while an all-sources sync pass (launch / interval / Sync now) runs.
    pub sync_pass_running: std::sync::atomic::AtomicBool,
    /// The latest finished sync pass, for a window that opens after it ended.
    pub last_sync_report: Mutex<Option<sync::orchestrator::SyncReport>>,
    pub sync_seq: std::sync::atomic::AtomicU64,
    /// Keeps the Scout corpus indexes (writing, tweets, highlights) current
    /// for archive search, one background build at a time.
    pub corpus_index: corpus::IndexKeeper,
    /// The meaning index (semantic search): the session's one embedding
    /// model, loaded on first use, and its one background build.
    pub meaning: meaning::Meaning,
}

impl AppState {
    /// Snapshot the current config for read-only use inside a command.
    pub fn config(&self) -> config::Config {
        self.config.read().expect("config lock poisoned").clone()
    }
}

/// Headless import: `highlight-scout --import-x <saved.jsonl>` imports X saved
/// tweets into the configured archive + index without launching the GUI.
/// Refuses (exit 2) while the app is running; exit 1 on any other failure.
fn headless_import_x(path: &str) -> i32 {
    let cfg = config::load();
    match import_x_locked(
        path,
        &cfg.archive_path,
        &config::index_path(),
        &config::lock_path(),
    ) {
        Ok((status, tweets)) => {
            commands::import::log_tweet_report(&tweets);
            println!("{} (from {})", status.message, path);
            if let Some(m) = tweets.log_message() {
                println!("{m}");
            }
            0
        }
        Err(HeadlessError::Refused(e)) => {
            eprintln!("import refused: {e}");
            2
        }
        Err(HeadlessError::Failed(e)) => {
            eprintln!("import {path}: {e}");
            1
        }
    }
}

#[derive(Debug)]
enum HeadlessError {
    /// The app holds its lock: it could be writing the same files.
    Refused(String),
    Failed(String),
}

/// `import_x_into` under the app lock, held for the whole import, so it
/// never runs beside the app or beside another headless run.
fn import_x_locked(
    path: &str,
    archive_path: &str,
    index_path: &std::path::Path,
    lock_path: &std::path::Path,
) -> Result<(models::ImportStatus, import::tweet_upsert::TweetReport), HeadlessError> {
    let _lock = app_lock::AppLock::acquire(lock_path).map_err(HeadlessError::Refused)?;
    import_x_into(path, archive_path, index_path).map_err(HeadlessError::Failed)
}

/// The headless X import against a named archive and index, through the
/// same persistence step as the in-app importers (so a tweet already in the
/// archive is updated in its own file, found by key).
fn import_x_into(
    path: &str,
    archive_path: &str,
    index_path: &std::path::Path,
) -> Result<(models::ImportStatus, import::tweet_upsert::TweetReport), String> {
    if let Some(parent) = index_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = scout_index::sqlite::open(index_path).map_err(|e| format!("open index: {e}"))?;
    scout_index::sqlite::init_schema(&conn).map_err(|e| format!("init schema: {e}"))?;
    let (works, hls) = import::x::import(path).map_err(|e| e.to_string())?;
    commands::import::persist_core_reported(
        &Mutex::new(conn),
        archive_path,
        commands::import::PersistBatch {
            source: "x",
            works: &works,
            highlights: &hls,
            archive_records: None,
            deleted_ids: &std::collections::HashSet::new(),
            raw_json: None,
        },
        &|_, _, _| {},
    )
}

/// What a GUI start that cannot take the app lock says before it exits.
const ALREADY_RUNNING: &str = "Highlight Scout is already running, or an import is in progress. Close it or wait, then open Highlight Scout again.";

/// The app lock, then the main index, in that order: the index file is not
/// created, opened or initialised by a process that does not hold the lock.
/// An `Err(StartError::Locked)` start opens nothing.
fn lock_then_open_index(
    lock_path: &std::path::Path,
    index_path: &std::path::Path,
) -> Result<(app_lock::AppLock, Connection), StartError> {
    let lock = app_lock::AppLock::acquire(lock_path).map_err(StartError::Locked)?;
    if let Some(parent) = index_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = scout_index::sqlite::open(index_path)
        .map_err(|e| StartError::Index(format!("Failed to open SQLite index: {e}")))?;
    scout_index::sqlite::init_schema(&conn)
        .map_err(|e| StartError::Index(format!("Failed to initialise schema: {e}")))?;
    Ok((lock, conn))
}

#[derive(Debug)]
enum StartError {
    /// The app lock was not taken; the reason, for stderr.
    Locked(String),
    Index(String),
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--import-x") {
        std::process::exit(headless_import_x(
            args.get(i + 1).map(|s| s.as_str()).unwrap_or(""),
        ));
    }
    if args.iter().any(|a| a == "--merge-duplicate-readwise-works") {
        std::process::exit(import::readwise_identity::run_cli(
            &args,
            &config::lock_path(),
        ));
    }

    // The one writing process holds the app lock for the life of run(). A
    // start that cannot take it does not open: it says so and exits, having
    // touched neither the index nor the archive.
    let (_app_lock, conn) = match lock_then_open_index(&config::lock_path(), &config::index_path())
    {
        Ok(held) => held,
        Err(StartError::Locked(reason)) => {
            eprintln!("{ALREADY_RUNNING} ({reason})");
            std::process::exit(2);
        }
        Err(StartError::Index(e)) => panic!("{e}"),
    };

    let cfg = config::load();
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
            busy: busy::BusyLock::default(),
            is_ocring: std::sync::atomic::AtomicBool::new(false),
            sync_pass_running: std::sync::atomic::AtomicBool::new(false),
            last_sync_report: Mutex::new(None),
            sync_seq: std::sync::atomic::AtomicU64::new(0),
            corpus_index: corpus::IndexKeeper::default(),
            meaning: meaning::Meaning::new(meaning::embedder_from_env()),
        })
        .invoke_handler(tauri::generate_handler![
            commands::search::search_query,
            commands::search::search_counts,
            commands::search::find_related,
            commands::search::get_highlight,
            commands::search::ocr_images,
            commands::search::work_highlights,
            commands::search::highlight_position,
            commands::search::list_tags,
            commands::search::get_facets,
            commands::search::get_stats,
            commands::corpus::corpus_search,
            commands::corpus::corpus_cite,
            commands::corpus::corpus_index_status,
            commands::corpus::corpus_index_refresh,
            commands::corpus::corpus_meaning_state,
            commands::corpus::corpus_meaning_build,
            commands::corpus::corpus_passage,
            commands::corpus::corpus_counts,
            commands::corpus::frontmost_other_app,
            commands::corpus::quick_finder_hide,
            commands::import::run_import,
            commands::import::merge_duplicate_readwise_works,
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
            commands::settings::save_r2_credentials,
            commands::settings::test_r2_connection,
            commands::settings::r2_backup_now,
            commands::settings::r2_restore_now,
            commands::settings::set_autostart,
            commands::clipboard::copy_image,
            commands::sync::sync_now,
            commands::sync::get_sync_status,
            commands::sync::get_sync_error_times,
        ])
        .setup(move |app| {
            let app_handle = app.handle().clone();

            // A hard crash mid-write can leave `.<name>.tmp-…` files in the
            // archive; clear any older than an hour, off the launch path.
            let archive_path = app.state::<AppState>().config().archive_path;
            std::thread::spawn(move || {
                archive_meta::sweep_stale_temp_files(
                    &archive_path,
                    std::time::Duration::from_secs(3600),
                );
            });
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
            if want && !is_on {
                let _ = autostart.enable();
            }
            if !want && is_on {
                let _ = autostart.disable();
            }

            // Show the main window on first launch (config has visible:false).
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }

            // Background sync: every configured source once at launch, then
            // every `sync_interval_hours` while the app runs (see crate::sync).
            crate::sync::spawn_scheduler(app.handle().clone());

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_start_without_the_app_lock_never_touches_the_index() {
        let dir = std::env::temp_dir().join(format!("hs-start-order-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let lock_path = dir.join("lock/highlight-scout.lock");
        // The index's directory does not exist yet: opening would make it.
        let index_path = dir.join("index/highlights.db");

        let other_process = app_lock::AppLock::acquire(&lock_path).unwrap();
        let refused = lock_then_open_index(&lock_path, &index_path).unwrap_err();
        assert!(matches!(refused, StartError::Locked(_)), "{refused:?}");
        assert!(!index_path.exists());
        assert!(!index_path.parent().unwrap().exists());

        // With the lock free, the same call opens and initialises the index
        // and holds the lock while it does.
        drop(other_process);
        let (_lock, conn) = lock_then_open_index(&lock_path, &index_path).unwrap();
        assert!(app_lock::AppLock::acquire(&lock_path).is_err());
        let tables: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name = 'works'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(tables, 1);
        drop(conn);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
