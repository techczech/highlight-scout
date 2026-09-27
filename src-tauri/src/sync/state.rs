// Persisted per-source sync status (`sync-state.json` beside the config):
// when each source last synced successfully, and its current failure, which
// stays until a later sync of that source succeeds.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::orchestrator::SourceResult;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceState {
    #[serde(default)]
    pub last_synced_at: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
    #[serde(default)]
    pub last_error_at: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncState {
    /// Keyed by `SyncSourceId::key()`.
    #[serde(default)]
    pub sources: BTreeMap<String, SourceState>,
}

impl SyncState {
    /// Fold a pass's results in: success sets last_synced_at and clears the
    /// error; failure records the error and keeps the last success time.
    pub fn apply(&mut self, results: &[SourceResult]) {
        for r in results {
            let s = self.sources.entry(r.source.to_string()).or_default();
            match &r.error {
                None => {
                    s.last_synced_at = Some(r.finished_at.clone());
                    s.last_error = None;
                    s.last_error_at = None;
                }
                Some(e) => {
                    s.last_error = Some(e.clone());
                    s.last_error_at = Some(r.finished_at.clone());
                }
            }
        }
    }

    /// Fill a missing last_synced_at from a pre-0.5.6 config cursor, so the
    /// first launch after upgrading still shows when each source last synced.
    pub fn seed_last_synced(&mut self, key: &str, legacy_cursor: &str) {
        if legacy_cursor.is_empty() {
            return;
        }
        let s = self.sources.entry(key.to_string()).or_default();
        if s.last_synced_at.is_none() {
            s.last_synced_at = Some(legacy_cursor.to_string());
        }
    }
}

pub fn state_path() -> PathBuf {
    crate::config::base_dir().join("sync-state.json")
}

pub fn load_from(path: &Path) -> SyncState {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn save_to(path: &Path, state: &SyncState) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(state).map_err(std::io::Error::other)?;
    std::fs::write(path, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(source: &'static str, error: Option<&str>, at: &str) -> SourceResult {
        SourceResult {
            source,
            label: "",
            added: 0,
            error: error.map(String::from),
            finished_at: at.to_string(),
        }
    }

    #[test]
    fn failure_persists_until_that_source_succeeds() {
        let mut s = SyncState::default();
        s.apply(&[r("readwise", None, "t1"), r("zotero", None, "t1")]);
        s.apply(&[r("readwise", Some("token rejected (401)"), "t2"), r("zotero", None, "t2")]);
        let rw = &s.sources["readwise"];
        assert_eq!(rw.last_error.as_deref(), Some("token rejected (401)"));
        assert_eq!(rw.last_synced_at.as_deref(), Some("t1"));
        assert_eq!(s.sources["zotero"].last_synced_at.as_deref(), Some("t2"));

        // A pass in which readwise did not run leaves the error in place.
        s.apply(&[r("zotero", None, "t3")]);
        assert!(s.sources["readwise"].last_error.is_some());

        s.apply(&[r("readwise", None, "t4")]);
        let rw = &s.sources["readwise"];
        assert!(rw.last_error.is_none());
        assert_eq!(rw.last_synced_at.as_deref(), Some("t4"));
    }

    #[test]
    fn seeds_from_legacy_cursor_only_when_missing() {
        let mut s = SyncState::default();
        s.seed_last_synced("readwise", "2026-07-22T10:00:00Z");
        s.seed_last_synced("zotero", "");
        assert_eq!(
            s.sources["readwise"].last_synced_at.as_deref(),
            Some("2026-07-22T10:00:00Z")
        );
        assert!(!s.sources.contains_key("zotero"));
        s.apply(&[r("readwise", None, "t9")]);
        s.seed_last_synced("readwise", "old");
        assert_eq!(s.sources["readwise"].last_synced_at.as_deref(), Some("t9"));
    }

    #[test]
    fn round_trips_through_disk() {
        let dir = std::env::temp_dir().join(format!("hs-sync-state-{}", std::process::id()));
        let path = dir.join("sync-state.json");
        let mut s = SyncState::default();
        s.apply(&[r("readwise_tweets", Some("could not reach Readwise (offline?)"), "t1")]);
        save_to(&path, &s).unwrap();
        assert_eq!(load_from(&path), s);
        assert_eq!(load_from(&dir.join("missing.json")), SyncState::default());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
