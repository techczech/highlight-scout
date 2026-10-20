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
mod menu_suspend;
mod models;
mod ocr;
mod r2;
mod settings_host;
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
    /// The settings store (`settings.json`, shared settings design); `config` is its runtime view.
    pub settings: std::sync::Arc<app_settings::SettingsStore>,
}

static APP_HANDLE: std::sync::OnceLock<tauri::AppHandle> = std::sync::OnceLock::new();

/// Rebuilds the runtime config from the store, the Keychain and the sync cursors.
pub(crate) fn refresh_runtime_config(state: &AppState) -> (config::Config, config::Config) {
    let new = settings_host::runtime_config(&state.settings.view(), state.settings.schema(), settings_host::secrets().as_ref(), &config::base_dir());
    let mut g = state.config.write().unwrap_or_else(|e| e.into_inner());
    let old = std::mem::replace(&mut *g, new.clone());
    (old, new)
}

/// A setting changed (in the window, by the CLI, by an import): apply what needs applying and tell every window.
fn on_settings_changed() {
    use tauri::Emitter;
    let Some(app) = APP_HANDLE.get() else { return };
    let state = app.state::<AppState>();
    let (old, new) = refresh_runtime_config(&state);
    if old.shortcut != new.shortcut {
        set_summon(app, &new.shortcut);
    }
    if old.autostart_enabled != new.autostart_enabled {
        use tauri_plugin_autostart::ManagerExt;
        let a = app.autolaunch();
        let _ = if new.autostart_enabled { a.enable() } else { a.disable() };
    }
    let _ = app.emit("settings:changed", commands::app_settings::view_json(&state));
}

/// The summon shortcut as registered now ("" none): a change unregisters this one only after the new one registered.
static SUMMON: Mutex<String> = Mutex::new(String::new());

/// Registers `acc` as the summon shortcut. The new one is registered first; only when that worked is the old one removed, so a
/// shortcut another app holds leaves the previous one working (and says so on stderr).
fn set_summon(app: &tauri::AppHandle, acc: &str) {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    let gs = app.global_shortcut();
    let mut cur = SUMMON.lock().unwrap_or_else(|e| e.into_inner());
    if *cur == acc {
        return;
    }
    if !acc.is_empty() {
        if let Err(e) = gs.on_shortcut(acc, summon) {
            eprintln!("Failed to register shortcut {acc}: {e}; {} stays", if cur.is_empty() { "no shortcut".to_string() } else { cur.clone() });
            return;
        }
    }
    if !cur.is_empty() {
        let _ = gs.unregister(cur.as_str());
    }
    *cur = acc.to_string();
}

/// The summon shortcut: bring the main window to the front (never hide it; it stays open while the app runs).
fn summon(app: &tauri::AppHandle, _s: &tauri_plugin_global_shortcut::Shortcut, _e: tauri_plugin_global_shortcut::ShortcutEvent) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
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
    // `highlight-scout settings …`: the agent CLI. Never takes the app lock (the app may be running; the settings file has its own).
    if args.get(1).map(String::as_str) == Some("settings") {
        std::process::exit(settings_host::run_cli(&args[2..]));
    }
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
            // Launched from Finder there is no terminal, so the user would
            // otherwise see nothing. Runs on the main thread, before the
            // Tauri builder, as rfd requires on macOS.
            rfd::MessageDialog::new()
                .set_title("Highlight Scout")
                .set_description(ALREADY_RUNNING)
                .set_level(rfd::MessageLevel::Info)
                .show();
            std::process::exit(2);
        }
        Err(StartError::Index(e)) => panic!("{e}"),
    };

    let (settings_store, cfg) = settings_host::load(Some(std::sync::Arc::new(|_: &app_settings::SettingsState| on_settings_changed())));
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
            settings: settings_store.clone(),
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
            commands::settings::test_r2_connection,
            commands::settings::r2_backup_now,
            commands::settings::r2_restore_now,
            commands::settings::set_autostart,
            commands::app_settings::settings_view,
            commands::app_settings::settings_set,
            commands::app_settings::settings_rebind,
            commands::app_settings::settings_rebind_many,
            commands::app_settings::settings_reset,
            commands::app_settings::settings_reset_section,
            commands::app_settings::settings_reset_all,
            commands::app_settings::settings_reset_entry,
            commands::app_settings::settings_undo_batch,
            commands::app_settings::settings_export,
            commands::app_settings::settings_import_preview,
            commands::app_settings::settings_import_apply,
            commands::app_settings::settings_set_secret,
            commands::app_settings::settings_remove_secret,
            commands::app_settings::settings_migrate_local,
            commands::app_settings::menu_shortcuts_suspend,
            commands::app_settings::menu_install,
            commands::app_settings::menu_shortcuts_restore,
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
            let _ = APP_HANDLE.set(app_handle.clone());
            // Outside changes (the CLI, another window) show within a second.
            app.state::<AppState>().settings.watch();

            set_summon(&app_handle, &shortcut);

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
            // The Settings window closing mid-recording gives the menu its shortcuts back.
            if let WindowEvent::Destroyed = event {
                crate::menu_suspend::restore_app_menu(window.app_handle(), Some(window.label()));
            }
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
