// Background sync of every configured source: once at launch (when
// `sync_on_launch` is on), every `sync_interval_hours` while the app runs, and
// on demand via "Sync now". The pure rules live in `orchestrator` (which
// sources, running order, summary text, plain-words errors) and `state`
// (persisted last-synced times and failures); this file is the Tauri glue.

pub mod orchestrator;
pub mod state;

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{Emitter, Manager};

use crate::config::Config;
use crate::models::ImportStatus;
use crate::AppState;
use orchestrator::SyncReport;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncSourceId {
    ReadwiseHighlights,
    ReadwiseTweets,
    Zotero,
}

pub const ALL_SOURCES: [SyncSourceId; 3] = [
    SyncSourceId::ReadwiseHighlights,
    SyncSourceId::ReadwiseTweets,
    SyncSourceId::Zotero,
];

impl SyncSourceId {
    /// Stable key used in sync-state.json and events.
    pub fn key(self) -> &'static str {
        match self {
            Self::ReadwiseHighlights => "readwise",
            Self::ReadwiseTweets => "readwise_tweets",
            Self::Zotero => "zotero",
        }
    }
    pub fn from_key(k: &str) -> Option<Self> {
        ALL_SOURCES.into_iter().find(|id| id.key() == k)
    }
    /// Name shown to the user, e.g. in "Readwise: token rejected (401)".
    pub fn label(self) -> &'static str {
        match self {
            Self::ReadwiseHighlights => "Readwise",
            Self::ReadwiseTweets => "Readwise saved tweets",
            Self::Zotero => "Zotero",
        }
    }
    /// `works.source_system` of the records this source writes; new items are
    /// counted as the change in this source's record count across a run.
    fn source_system(self) -> &'static str {
        match self {
            Self::ReadwiseHighlights => "readwise",
            Self::ReadwiseTweets => "x",
            Self::Zotero => "zotero",
        }
    }
    /// The import cursor each importer already keeps in config.
    fn config_cursor(self, c: &Config) -> &str {
        match self {
            Self::ReadwiseHighlights => &c.readwise_last_sync,
            Self::ReadwiseTweets => &c.readwise_tweets_last_sync,
            Self::Zotero => &c.zotero_last_sync,
        }
    }
}

/// Run one source by reusing the existing command internals. Each updates its
/// own cursor on success.
pub async fn run_source(
    id: SyncSourceId,
    handle: &tauri::AppHandle,
    window: tauri::WebviewWindow,
) -> Result<ImportStatus, String> {
    let state = handle.state::<AppState>();
    match id {
        SyncSourceId::ReadwiseHighlights => {
            crate::commands::import::run_import(state, window).await
        }
        SyncSourceId::ReadwiseTweets => {
            crate::commands::import::import_readwise_tweets(state, window).await
        }
        SyncSourceId::Zotero => crate::commands::import::run_zotero_import(state, window).await,
    }
}

fn record_count(handle: &tauri::AppHandle, id: SyncSourceId) -> usize {
    let state = handle.state::<AppState>();
    let Ok(conn) = state.db.lock() else {
        return 0;
    };
    conn.query_row(
        "SELECT COUNT(*) FROM highlights h JOIN works w ON w.id = h.work_id \
         WHERE w.source_system = ?1",
        [id.source_system()],
        |row| row.get::<_, i64>(0),
    )
    .map(|n| n.max(0) as usize)
    .unwrap_or(0)
}

/// Run a source and report how many new records it added.
async fn run_counted(
    id: SyncSourceId,
    handle: &tauri::AppHandle,
    window: tauri::WebviewWindow,
) -> Result<usize, String> {
    let before = record_count(handle, id);
    run_source(id, handle, window).await?;
    Ok(record_count(handle, id).saturating_sub(before))
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceStatus {
    pub key: &'static str,
    pub label: &'static str,
    pub configured: bool,
    pub last_synced_at: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncStatus {
    pub running: bool,
    pub last_report: Option<SyncReport>,
    pub sources: Vec<SourceStatus>,
}

fn zotero_exists(p: &str) -> bool {
    std::path::Path::new(p).exists()
}

pub fn status(handle: &tauri::AppHandle) -> SyncStatus {
    let app = handle.state::<AppState>();
    let cfg = app.config();
    let configured = orchestrator::configured_sources(&cfg, zotero_exists);
    let mut st = state::load_from(&state::state_path());
    for id in ALL_SOURCES {
        st.seed_last_synced(id.key(), id.config_cursor(&cfg));
    }
    let sources = ALL_SOURCES
        .into_iter()
        .map(|id| {
            let s = st.sources.get(id.key()).cloned().unwrap_or_default();
            SourceStatus {
                key: id.key(),
                label: id.label(),
                configured: configured.contains(&id),
                last_synced_at: s.last_synced_at,
                last_error: s.last_error,
            }
        })
        .collect();
    SyncStatus {
        running: app.sync_pass_running.load(Ordering::SeqCst),
        last_report: app.last_sync_report.lock().ok().and_then(|g| g.clone()),
        sources,
    }
}

/// Clears the pass-running flag on drop (panic-safe).
struct PassGuard<'a>(&'a std::sync::atomic::AtomicBool);
impl Drop for PassGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// One sync pass over every configured source, in the background. Emits
/// `sync:finished` with the fresh `SyncStatus` (carrying the report).
pub async fn sync_all(handle: &tauri::AppHandle, trigger: &str) -> Result<SyncReport, String> {
    let app = handle.state::<AppState>();
    if app
        .sync_pass_running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("A sync is already running".to_string());
    }
    let _guard = PassGuard(&app.sync_pass_running);
    if app.is_syncing.load(Ordering::SeqCst) {
        return Err("An import is already running".to_string());
    }
    let window = handle
        .get_webview_window("main")
        .ok_or_else(|| "Main window not available".to_string())?;
    let cfg = app.config();
    let sources = orchestrator::configured_sources(&cfg, zotero_exists);
    let started_at = chrono::Utc::now().to_rfc3339();

    let results = orchestrator::run_pass(
        &sources,
        |id| run_counted(id, handle, window.clone()),
        || chrono::Utc::now().to_rfc3339(),
    )
    .await;

    let path = state::state_path();
    let mut st = state::load_from(&path);
    st.apply(&results);
    let _ = state::save_to(&path, &st);

    let report = SyncReport {
        seq: app.sync_seq.fetch_add(1, Ordering::SeqCst) + 1,
        trigger: trigger.to_string(),
        started_at,
        finished_at: chrono::Utc::now().to_rfc3339(),
        summary: orchestrator::summary_text(&results),
        results,
    };
    if let Ok(mut g) = app.last_sync_report.lock() {
        *g = Some(report.clone());
    }
    drop(_guard);
    let _ = handle.emit("sync:finished", status(handle));
    Ok(report)
}

/// Launch sync (after a short delay so the window is up and listening), then
/// a periodic pass every `sync_interval_hours` (default 6; 0 = off), re-read
/// each tick so a Settings change takes effect without a restart.
pub fn spawn_scheduler(handle: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(3)).await;
        let on_launch = handle.state::<AppState>().config().sync_on_launch;
        if on_launch {
            let _ = sync_all(&handle, "launch").await;
        }
        let mut last = Instant::now();
        let mut tick = tokio::time::interval(Duration::from_secs(300));
        tick.tick().await; // the first tick fires immediately
        loop {
            tick.tick().await;
            let hours = handle.state::<AppState>().config().sync_interval_hours;
            if hours == 0 || last.elapsed() < Duration::from_secs(hours as u64 * 3600) {
                continue;
            }
            if sync_all(&handle, "interval").await.is_ok() {
                last = Instant::now();
            }
        }
    });
}
