use serde::{Deserialize, Serialize};

use crate::config::{self, Config};
use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub readwise_api_key: String,
    pub archive_path: String,
    pub zotero_db_path: String,
    pub readwise_archive_path: String,
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
}

#[tauri::command]
pub async fn get_settings(state: tauri::State<'_, AppState>) -> Result<Settings, String> {
    let c = state.config();
    Ok(Settings {
        readwise_api_key: c.readwise_api_key,
        archive_path: c.archive_path,
        zotero_db_path: c.zotero_db_path,
        readwise_archive_path: c.readwise_archive_path,
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
        readwise_archive_path: settings.readwise_archive_path,
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
    };

    config::save(&new_config).map_err(|e| e.to_string())?;
    {
        let mut guard = state.config.write().map_err(|e| e.to_string())?;
        *guard = new_config;
    }

    // The global shortcut is registered at startup; a change needs a relaunch.
    Ok(shortcut_changed)
}

#[tauri::command]
pub async fn set_autostart(enabled: bool, app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let a = app.autolaunch();
    if enabled { a.enable().map_err(|e| e.to_string()) } else { a.disable().map_err(|e| e.to_string()) }
}
