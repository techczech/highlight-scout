// Tauri commands for the all-sources sync (see `crate::sync`).

use crate::sync::{self, orchestrator::SyncReport, SyncStatus};

/// "Sync now": one pass over every configured source. The toast and any red
/// failure lines come from the `sync:finished` event this also emits.
#[tauri::command]
pub async fn sync_now(app: tauri::AppHandle) -> Result<SyncReport, String> {
    sync::sync_all(&app, "manual").await
}

/// Per-source last-synced times and current failures, plus the latest report.
#[tauri::command]
pub async fn get_sync_status(app: tauri::AppHandle) -> Result<SyncStatus, String> {
    Ok(sync::status(&app))
}

/// When each source's current failure happened (`sync-state.json`, read
/// only), for Settings → Sync's "last try … failed".
#[tauri::command]
pub async fn get_sync_error_times() -> Result<std::collections::BTreeMap<String, String>, String> {
    let st = sync::state::load_from(&sync::state::state_path());
    Ok(st
        .sources
        .into_iter()
        .filter_map(|(k, s)| s.last_error_at.map(|t| (k, t)))
        .collect())
}
