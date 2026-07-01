use serde::{Deserialize, Serialize};

use crate::config::{self, Config};
use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub readwise_api_key: String,
    pub archive_path: String,
    pub zotero_db_path: String,
    pub shortcut: String,
    pub result_limit: u32,
    pub import_reminder_days: u32,
    pub readwise_sync_enabled: bool,
    pub readwise_sync_interval_hours: u32,
    pub readwise_tweets_sync_enabled: bool,
    pub readwise_tweets_sync_interval_hours: u32,
    pub zotero_sync_enabled: bool,
    pub zotero_sync_interval_hours: u32,
    pub autostart_enabled: bool,
    pub ocr_on_import: bool,
    pub r2_enabled: bool,
    pub r2_account_id: String,
    pub r2_endpoint: String,
    pub r2_bucket: String,
    pub r2_prefix: String,
    pub r2_has_credentials: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct R2CredentialSave {
    pub access_key_id: String,
    pub secret_access_key: String,
}

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
pub async fn get_settings(state: tauri::State<'_, AppState>) -> Result<Settings, String> {
    let c = state.config();
    Ok(Settings {
        readwise_api_key: c.readwise_api_key,
        archive_path: c.archive_path,
        zotero_db_path: c.zotero_db_path,
        shortcut: c.shortcut,
        result_limit: c.result_limit,
        import_reminder_days: c.import_reminder_days,
        readwise_sync_enabled: c.readwise_sync_enabled,
        readwise_sync_interval_hours: c.readwise_sync_interval_hours,
        readwise_tweets_sync_enabled: c.readwise_tweets_sync_enabled,
        readwise_tweets_sync_interval_hours: c.readwise_tweets_sync_interval_hours,
        zotero_sync_enabled: c.zotero_sync_enabled,
        zotero_sync_interval_hours: c.zotero_sync_interval_hours,
        autostart_enabled: c.autostart_enabled,
        ocr_on_import: c.ocr_on_import,
        r2_enabled: c.r2_enabled,
        r2_account_id: c.r2_account_id,
        r2_endpoint: c.r2_endpoint,
        r2_bucket: c.r2_bucket,
        r2_prefix: c.r2_prefix,
        r2_has_credentials: crate::r2::has_credentials(),
    })
}

#[tauri::command]
pub async fn save_settings(
    settings: Settings,
    state: tauri::State<'_, AppState>,
) -> Result<bool, String> {
    // Preserve only the sync cursors — not editable via Settings UI.
    let (readwise_last_sync, readwise_tweets_last_sync, zotero_last_sync) = {
        let current = state.config.read().map_err(|e| e.to_string())?;
        (
            current.readwise_last_sync.clone(),
            current.readwise_tweets_last_sync.clone(),
            current.zotero_last_sync.clone(),
        )
    };
    let shortcut_changed = {
        let current = state.config.read().map_err(|e| e.to_string())?;
        current.shortcut != settings.shortcut
    };

    let new_config = Config {
        readwise_api_key: settings.readwise_api_key,
        archive_path: settings.archive_path,
        zotero_db_path: settings.zotero_db_path,
        readwise_archive_path: current_readwise_archive_path(&state)?,
        shortcut: settings.shortcut,
        result_limit: settings.result_limit.clamp(1, 300),
        import_reminder_days: settings.import_reminder_days,
        readwise_last_sync,
        readwise_tweets_last_sync,
        zotero_last_sync,
        readwise_sync_enabled: settings.readwise_sync_enabled,
        readwise_sync_interval_hours: settings.readwise_sync_interval_hours,
        readwise_tweets_sync_enabled: settings.readwise_tweets_sync_enabled,
        readwise_tweets_sync_interval_hours: settings.readwise_tweets_sync_interval_hours,
        zotero_sync_enabled: settings.zotero_sync_enabled,
        zotero_sync_interval_hours: settings.zotero_sync_interval_hours,
        autostart_enabled: settings.autostart_enabled,
        ocr_on_import: settings.ocr_on_import,
        r2_enabled: settings.r2_enabled,
        r2_account_id: settings.r2_account_id,
        r2_endpoint: settings.r2_endpoint,
        r2_bucket: settings.r2_bucket,
        r2_prefix: settings.r2_prefix,
    };

    config::save(&new_config).map_err(|e| e.to_string())?;
    {
        let mut guard = state.config.write().map_err(|e| e.to_string())?;
        *guard = new_config;
    }

    // The global shortcut is registered at startup; a change needs a relaunch.
    Ok(shortcut_changed)
}

fn current_readwise_archive_path(state: &tauri::State<'_, AppState>) -> Result<String, String> {
    let current = state.config.read().map_err(|e| e.to_string())?;
    Ok(current.readwise_archive_path.clone())
}

#[tauri::command]
pub async fn save_r2_credentials(credentials: R2CredentialSave) -> Result<R2ActionStatus, String> {
    crate::r2::save_credentials(&credentials.access_key_id, &credentials.secret_access_key)
        .map(|_| R2ActionStatus {
            ok: true,
            message: "R2 credentials saved in Keychain".to_string(),
            uploaded: 0,
            downloaded: 0,
            skipped: 0,
            failed: 0,
        })
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn test_r2_connection(state: tauri::State<'_, AppState>) -> Result<R2ActionStatus, String> {
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
    if enabled { a.enable().map_err(|e| e.to_string()) } else { a.disable().map_err(|e| e.to_string()) }
}
