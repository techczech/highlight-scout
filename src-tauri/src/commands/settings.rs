use serde::{Deserialize, Serialize};

use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct R2ActionStatus {
    pub ok: bool,
    pub message: String,
    pub uploaded: usize,
    pub downloaded: usize,
    pub skipped: usize,
    pub failed: usize,
}

#[tauri::command]
pub async fn test_r2_connection(
    state: tauri::State<'_, AppState>,
) -> Result<R2ActionStatus, String> {
    let config = state.config();
    crate::r2::test_connection(&config)
        .await
        .map(|_| R2ActionStatus {
            ok: true,
            message: "Connected to R2".to_string(),
            uploaded: 0,
            downloaded: 0,
            skipped: 0,
            failed: 0,
        })
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn r2_backup_now(state: tauri::State<'_, AppState>) -> Result<R2ActionStatus, String> {
    let config = state.config();
    crate::r2::push_archive(&config)
        .await
        .map(|r| R2ActionStatus {
            ok: r.failed == 0,
            message: r.message,
            uploaded: r.uploaded,
            downloaded: r.downloaded,
            skipped: r.skipped,
            failed: r.failed,
        })
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn r2_restore_now(state: tauri::State<'_, AppState>) -> Result<R2ActionStatus, String> {
    let config = state.config();
    crate::r2::pull_archive(&config)
        .await
        .map(|r| R2ActionStatus {
            ok: r.failed == 0,
            message: r.message,
            uploaded: r.uploaded,
            downloaded: r.downloaded,
            skipped: r.skipped,
            failed: r.failed,
        })
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_autostart(enabled: bool, app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let a = app.autolaunch();
    if enabled {
        a.enable().map_err(|e| e.to_string())
    } else {
        a.disable().map_err(|e| e.to_string())
    }
}
