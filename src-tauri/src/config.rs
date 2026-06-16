use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub readwise_api_key: String,
    pub archive_path: String,
    pub shortcut: String,
}

impl Default for Config {
    fn default() -> Self {
        let home = std::env::var("HOME").unwrap_or_default();
        Config {
            readwise_api_key: String::new(),
            archive_path: format!(
                "{}/gitrepos/16_writing_and_research/highlights-archive-v2",
                home
            ),
            shortcut: "CmdOrCtrl+Shift+H".to_string(),
        }
    }
}

pub fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home)
        .join(".config")
        .join("highlight-scout")
        .join("config.toml")
}

pub fn index_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home)
        .join(".config")
        .join("highlight-scout")
        .join("index.sqlite")
}

pub fn load() -> Config {
    let path = config_path();
    if !path.exists() {
        let config = Config::default();
        // Write default config so user knows where to put their API key
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let toml = format!(
            "# Highlight Scout configuration\n\
             # Get your Readwise API key from https://readwise.io/access_token\n\
             readwise_api_key = \"{}\"\n\
             archive_path = \"{}\"\n\
             shortcut = \"{}\"\n",
            config.readwise_api_key, config.archive_path, config.shortcut
        );
        let _ = fs::write(&path, toml);
        return config;
    }

    let content = fs::read_to_string(&path).unwrap_or_default();
    // Simple TOML parsing: key = "value" lines
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
                _ => {}
            }
        }
    }

    // API key can also come from environment (for dev)
    if config.readwise_api_key.is_empty() {
        if let Ok(key) = std::env::var("READWISE_API_KEY") {
            config.readwise_api_key = key;
        }
    }

    config
}
