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
