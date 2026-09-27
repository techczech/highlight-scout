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
    /// Sync every configured source (Readwise highlights, Readwise saved
    /// tweets, Zotero) in the background when the app opens. Replaces the old
    /// per-source `*_sync_enabled` / `*_sync_interval_hours` keys (0.5.6).
    #[serde(default = "default_true")]
    pub sync_on_launch: bool,
    /// While the app runs, sync all configured sources again every N hours.
    /// 0 = off.
    #[serde(default = "default_sync_interval_hours")]
    pub sync_interval_hours: u32,
    #[serde(default)]
    pub readwise_tweets_last_sync: String,
    #[serde(default)]
    pub zotero_last_sync: String,
    #[serde(default)]
    pub autostart_enabled: bool,
    #[serde(default = "default_true")]
    pub ocr_on_import: bool,
    #[serde(default)]
    pub r2_enabled: bool,
    #[serde(default)]
    pub r2_account_id: String,
    #[serde(default)]
    pub r2_endpoint: String,
    #[serde(default = "default_r2_bucket")]
    pub r2_bucket: String,
    #[serde(default = "default_r2_prefix")]
    pub r2_prefix: String,
}

fn default_true() -> bool {
    true
}

fn default_sync_interval_hours() -> u32 {
    6
}

/// Keys written by 0.5.5 and earlier. Still accepted when reading (ignored);
/// never written again.
const LEGACY_SYNC_KEYS: [&str; 6] = [
    "readwise_sync_enabled",
    "readwise_sync_interval_hours",
    "readwise_tweets_sync_enabled",
    "readwise_tweets_sync_interval_hours",
    "zotero_sync_enabled",
    "zotero_sync_interval_hours",
];

fn default_result_limit() -> u32 {
    80
}

fn default_r2_bucket() -> String {
    "highlight-scout".to_string()
}

fn default_r2_prefix() -> String {
    "highlight-scout".to_string()
}

fn default_readwise_archive() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    format!(
        "{}/gitrepos/16_writing_and_research/highlights-archive",
        home
    )
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
            sync_on_launch: true,
            sync_interval_hours: default_sync_interval_hours(),
            readwise_tweets_last_sync: String::new(),
            zotero_last_sync: String::new(),
            autostart_enabled: false,
            ocr_on_import: true,
            r2_enabled: false,
            r2_account_id: String::new(),
            r2_endpoint: String::new(),
            r2_bucket: default_r2_bucket(),
            r2_prefix: default_r2_prefix(),
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
         sync_on_launch = {}\n\
         sync_interval_hours = {}\n\
         readwise_tweets_last_sync = \"{}\"\n\
         zotero_last_sync = \"{}\"\n\
         autostart_enabled = {}\n\
         ocr_on_import = {}\n\
         r2_enabled = {}\n\
         r2_account_id = \"{}\"\n\
         r2_endpoint = \"{}\"\n\
         r2_bucket = \"{}\"\n\
         r2_prefix = \"{}\"\n",
        config.readwise_api_key,
        config.archive_path,
        config.shortcut,
        config.zotero_db_path,
        config.result_limit,
        config.readwise_archive_path,
        config.readwise_last_sync,
        config.import_reminder_days,
        config.sync_on_launch,
        config.sync_interval_hours,
        config.readwise_tweets_last_sync,
        config.zotero_last_sync,
        config.autostart_enabled,
        config.ocr_on_import,
        config.r2_enabled,
        config.r2_account_id,
        config.r2_endpoint,
        config.r2_bucket,
        config.r2_prefix
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
                "sync_on_launch" => config.sync_on_launch = val == "true",
                "sync_interval_hours" => {
                    config.sync_interval_hours =
                        val.parse().unwrap_or(default_sync_interval_hours())
                }
                "readwise_tweets_last_sync" => config.readwise_tweets_last_sync = val.to_string(),
                "zotero_last_sync" => config.zotero_last_sync = val.to_string(),
                "autostart_enabled" => config.autostart_enabled = val == "true",
                "ocr_on_import" => config.ocr_on_import = val == "true",
                "r2_enabled" => config.r2_enabled = val == "true",
                "r2_account_id" => config.r2_account_id = val.to_string(),
                "r2_endpoint" => config.r2_endpoint = val.to_string(),
                "r2_bucket" => config.r2_bucket = val.to_string(),
                "r2_prefix" => config.r2_prefix = val.to_string(),
                _ => {}
            }
        }
    }
    config
}

/// True when the on-disk text predates the 0.5.6 sync settings.
pub(crate) fn needs_migration(content: &str) -> bool {
    let keys: Vec<&str> = content
        .lines()
        .map(str::trim)
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| l.split_once('=').map(|(k, _)| k.trim()))
        .collect();
    !keys.contains(&"sync_on_launch") || keys.iter().any(|k| LEGACY_SYNC_KEYS.contains(k))
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

    // 0.5.6 migration: a config without `sync_on_launch` (or still carrying
    // the old per-source sync keys) is rewritten in the new shape. The parser
    // has already defaulted sync_on_launch = true and the interval to 6 h.
    if needs_migration(&content) {
        let _ = save(&config);
    }

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
        c.sync_on_launch = false;
        c.sync_interval_hours = 24;
        c.readwise_tweets_last_sync = "2026-06-21T00:00:00Z".into();
        c.zotero_last_sync = "2026-06-22T00:00:00Z".into();
        c.autostart_enabled = true;
        c.ocr_on_import = false;
        let text = serialize(&c);
        assert!(!needs_migration(&text));
        let parsed = parse_config_text(&text);
        assert!(!parsed.sync_on_launch);
        assert_eq!(parsed.sync_interval_hours, 24);
        assert_eq!(parsed.readwise_tweets_last_sync, "2026-06-21T00:00:00Z");
        assert_eq!(parsed.zotero_last_sync, "2026-06-22T00:00:00Z");
        assert!(parsed.autostart_enabled);
        assert!(!parsed.ocr_on_import);
    }

    #[test]
    fn defaults_sync_on_launch_every_six_hours() {
        let c = Config::default();
        assert!(c.sync_on_launch);
        assert_eq!(c.sync_interval_hours, 6);
    }

    /// The shape of a 0.5.5 config with every scheduled sync switched off.
    const LEGACY_055: &str = "readwise_api_key = \"k\"\n\
        archive_path = \"/tmp/a\"\n\
        readwise_last_sync = \"2026-07-22T10:00:00+00:00\"\n\
        readwise_sync_enabled = false\n\
        readwise_sync_interval_hours = 0\n\
        readwise_tweets_sync_enabled = false\n\
        readwise_tweets_sync_interval_hours = 0\n\
        readwise_tweets_last_sync = \"2026-07-01T00:00:00Z\"\n\
        zotero_sync_enabled = false\n\
        zotero_sync_interval_hours = 0\n\
        zotero_last_sync = \"\"\n";

    #[test]
    fn legacy_config_migrates_to_sync_on_launch() {
        assert!(needs_migration(LEGACY_055));
        let c = parse_config_text(LEGACY_055);
        assert!(c.sync_on_launch, "old disabled flags must not turn launch sync off");
        assert_eq!(c.sync_interval_hours, 6);
        // Cursors and credentials survive.
        assert_eq!(c.readwise_api_key, "k");
        assert_eq!(c.readwise_last_sync, "2026-07-22T10:00:00+00:00");
        assert_eq!(c.readwise_tweets_last_sync, "2026-07-01T00:00:00Z");
        // Rewritten text drops the legacy keys and is stable.
        let text = serialize(&c);
        for k in LEGACY_SYNC_KEYS {
            assert!(!text.contains(k), "legacy key {k} written back");
        }
        assert!(text.contains("sync_on_launch = true"));
        assert!(!needs_migration(&text));
        let again = parse_config_text(&text);
        assert!(again.sync_on_launch);
        assert_eq!(again.readwise_last_sync, c.readwise_last_sync);
    }

    #[test]
    fn r2_metadata_round_trips_without_credentials() {
        let mut c = Config::default();
        c.r2_enabled = true;
        c.r2_account_id = "account".into();
        c.r2_endpoint = "https://account.r2.cloudflarestorage.com".into();
        c.r2_bucket = "highlight-scout".into();
        c.r2_prefix = "dominik/highlights".into();

        let text = serialize(&c);
        assert!(!text.contains("secret"));
        assert!(!text.contains("access_key"));

        let parsed = parse_config_text(&text);
        assert!(parsed.r2_enabled);
        assert_eq!(parsed.r2_account_id, "account");
        assert_eq!(
            parsed.r2_endpoint,
            "https://account.r2.cloudflarestorage.com"
        );
        assert_eq!(parsed.r2_bucket, "highlight-scout");
        assert_eq!(parsed.r2_prefix, "dominik/highlights");
    }
}
