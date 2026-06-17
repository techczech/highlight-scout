// Persistent import log (JSONL) — one line per import run, recording what was
// imported when and any error. Lives beside the config so it survives reinstalls.

use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportLogEntry {
    pub timestamp: String,
    pub source: String,
    pub works: usize,
    pub highlights: usize,
    pub status: String, // "ok" | "error"
    pub message: String,
    #[serde(default)]
    pub duration_ms: u64,
}

fn log_path() -> PathBuf {
    crate::config::base_dir().join("import-log.jsonl")
}

pub fn append(entry: &ImportLogEntry) {
    let path = log_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(line) = serde_json::to_string(entry) {
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&path) {
            let _ = writeln!(f, "{}", line);
        }
    }
}

/// Read the most recent entries, newest first.
pub fn read_recent(limit: usize) -> Vec<ImportLogEntry> {
    let content = std::fs::read_to_string(log_path()).unwrap_or_default();
    let mut entries: Vec<ImportLogEntry> = content
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    entries.reverse();
    entries.truncate(limit);
    entries
}
