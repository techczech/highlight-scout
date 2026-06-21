use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub readwise_api_key: String,
    pub archive_path: String,
    pub shortcut: String,
    pub zotero_db_path: String,
    #[serde(default = "default_result_limit")]
    pub result_limit: u32,
    /// Existing highlights-archive repo to seed Readwise data from (no API).
    #[serde(default = "default_readwise_archive")]
    pub readwise_archive_path: String,
    /// ISO timestamp of the last Readwise sync; used as updatedAfter for
    /// incremental export so we never re-pull everything.
    #[serde(default)]
    pub readwise_last_sync: String,
    /// Days of inactivity before the app nudges you to import again. 0 = off.
    #[serde(default)]
    pub import_reminder_days: u32,
    #[serde(default)]
    pub readwise_sync_enabled: bool,
    #[serde(default)]
    pub readwise_sync_interval_hours: u32,
    #[serde(default)]
    pub readwise_tweets_sync_enabled: bool,
    #[serde(default)]
    pub readwise_tweets_sync_interval_hours: u32,
    #[serde(default)]
    pub readwise_tweets_last_sync: String,
    #[serde(default)]
    pub zotero_sync_enabled: bool,
    #[serde(default)]
    pub zotero_sync_interval_hours: u32,
    #[serde(default)]
    pub zotero_last_sync: String,
    #[serde(default)]
    pub autostart_enabled: bool,
}

fn default_result_limit() -> u32 {
    80
}

fn default_readwise_archive() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    format!("{}/gitrepos/16_writing_and_research/highlights-archive", home)
}

impl Default for Config {
    fn default() -> Self {
        Config {
            readwise_api_key: String::new(),
            // New installs: a visible, user-owned folder. Existing configs keep
            // whatever path they already saved (e.g. Dominik's git repo).
            archive_path: default_archive_path(),
            shortcut: "CmdOrCtrl+Alt+Shift+H".to_string(),
            zotero_db_path: default_zotero_path(),
            result_limit: default_result_limit(),
            readwise_archive_path: default_readwise_archive(),
            readwise_last_sync: String::new(),
            import_reminder_days: 0,
            readwise_sync_enabled: false,
            readwise_sync_interval_hours: 0,
            readwise_tweets_sync_enabled: false,
            readwise_tweets_sync_interval_hours: 0,
            readwise_tweets_last_sync: String::new(),
            zotero_sync_enabled: false,
            zotero_sync_interval_hours: 0,
            zotero_last_sync: String::new(),
            autostart_enabled: false,
        }
    }
}

fn default_archive_path() -> String {
    dirs::document_dir()
        .map(|d| d.join("Highlight Scout"))
        .unwrap_or_else(|| PathBuf::from("Highlight Scout"))
        .to_string_lossy()
        .to_string()
}

fn default_zotero_path() -> String {
    dirs::home_dir()
        .unwrap_or_default()
        .join("Zotero")
        .join("zotero.sqlite")
        .to_string_lossy()
        .to_string()
}

/// Base dir for config + index + import log. Non-destructive: if the legacy
/// `~/.config/highlight-scout` already exists (existing installs), keep using
/// it; otherwise use the OS-appropriate app-config dir (so Windows works).
pub fn base_dir() -> PathBuf {
    let legacy = dirs::home_dir()
        .unwrap_or_default()
        .join(".config")
        .join("highlight-scout");
    if legacy.exists() {
        return legacy;
    }
    dirs::config_dir()
        .map(|d| d.join("highlight-scout"))
        .unwrap_or(legacy)
}

pub fn config_path() -> PathBuf {
    base_dir().join("config.toml")
}

pub fn index_path() -> PathBuf {
    base_dir().join("index.sqlite")
}

fn serialize(config: &Config) -> String {
    format!(
        "# Highlight Scout configuration\n\
         # Get your Readwise API key from https://readwise.io/access_token\n\
         readwise_api_key = \"{}\"\n\
         archive_path = \"{}\"\n\
         shortcut = \"{}\"\n\
         zotero_db_path = \"{}\"\n\
         result_limit = {}\n\
         readwise_archive_path = \"{}\"\n\
         readwise_last_sync = \"{}\"\n\
         import_reminder_days = {}\n\
         readwise_sync_enabled = {}\n\
         readwise_sync_interval_hours = {}\n\
         readwise_tweets_sync_enabled = {}\n\
         readwise_tweets_sync_interval_hours = {}\n\
         readwise_tweets_last_sync = \"{}\"\n\
         zotero_sync_enabled = {}\n\
         zotero_sync_interval_hours = {}\n\
         zotero_last_sync = \"{}\"\n\
         autostart_enabled = {}\n",
        config.readwise_api_key,
        config.archive_path,
        config.shortcut,
        config.zotero_db_path,
        config.result_limit,
        config.readwise_archive_path,
        config.readwise_last_sync,
        config.import_reminder_days,
        config.readwise_sync_enabled,
        config.readwise_sync_interval_hours,
        config.readwise_tweets_sync_enabled,
        config.readwise_tweets_sync_interval_hours,
        config.readwise_tweets_last_sync,
        config.zotero_sync_enabled,
        config.zotero_sync_interval_hours,
        config.zotero_last_sync,
        config.autostart_enabled
    )
}

pub fn save(config: &Config) -> std::io::Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, serialize(config))
}

pub(crate) fn parse_config_text(content: &str) -> Config {
    // Simple TOML parsing: `key = "value"` / `key = number` lines.
    let mut config = Config::default();
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some((key, val)) = line.split_once('=') {
            let key = key.trim();
            let val = val.trim().trim_matches('"');
            match key {
                "readwise_api_key" => config.readwise_api_key = val.to_string(),
                "archive_path" => config.archive_path = val.to_string(),
                "shortcut" => config.shortcut = val.to_string(),
                "zotero_db_path" => config.zotero_db_path = val.to_string(),
                "readwise_archive_path" => config.readwise_archive_path = val.to_string(),
                "readwise_last_sync" => config.readwise_last_sync = val.to_string(),
                "result_limit" => {
                    if let Ok(n) = val.parse::<u32>() {
                        config.result_limit = n.clamp(1, 300);
                    }
                }
                "import_reminder_days" => {
                    if let Ok(n) = val.parse::<u32>() {
                        config.import_reminder_days = n;
                    }
                }
                "readwise_sync_enabled" => config.readwise_sync_enabled = val == "true",
                "readwise_sync_interval_hours" => config.readwise_sync_interval_hours = val.parse().unwrap_or(0),
                "readwise_tweets_sync_enabled" => config.readwise_tweets_sync_enabled = val == "true",
                "readwise_tweets_sync_interval_hours" => config.readwise_tweets_sync_interval_hours = val.parse().unwrap_or(0),
                "readwise_tweets_last_sync" => config.readwise_tweets_last_sync = val.to_string(),
                "zotero_sync_enabled" => config.zotero_sync_enabled = val == "true",
                "zotero_sync_interval_hours" => config.zotero_sync_interval_hours = val.parse().unwrap_or(0),
                "zotero_last_sync" => config.zotero_last_sync = val.to_string(),
                "autostart_enabled" => config.autostart_enabled = val == "true",
                _ => {}
            }
        }
    }
    config
}

pub fn load() -> Config {
    let path = config_path();
    if !path.exists() {
        let config = Config::default();
        let _ = save(&config);
        return config;
    }

    let content = fs::read_to_string(&path).unwrap_or_default();
    let mut config = parse_config_text(&content);

    // API key can also come from environment (for dev).
    if config.readwise_api_key.is_empty() {
        if let Ok(key) = std::env::var("READWISE_API_KEY") {
            config.readwise_api_key = key;
        }
    }

    config
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_sync_fields_round_trip() {
        let mut c = Config::default();
        c.readwise_tweets_sync_enabled = true;
        c.readwise_tweets_sync_interval_hours = 6;
        c.readwise_tweets_last_sync = "2026-06-21T00:00:00Z".into();
        c.zotero_sync_enabled = true;
        c.zotero_sync_interval_hours = 24;
        c.autostart_enabled = true;
        let text = serialize(&c);
        let parsed = parse_config_text(&text);
        assert!(parsed.readwise_tweets_sync_enabled);
        assert_eq!(parsed.readwise_tweets_sync_interval_hours, 6);
        assert_eq!(parsed.readwise_tweets_last_sync, "2026-06-21T00:00:00Z");
        assert!(parsed.zotero_sync_enabled);
        assert_eq!(parsed.zotero_sync_interval_hours, 24);
        assert!(parsed.autostart_enabled);
    }
}
