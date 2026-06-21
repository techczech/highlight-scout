// Schedulable-source registry + the "is this source due?" rule. The scheduler
// (lib.rs) and the Settings Sync tab both drive off SCHEDULABLE.

use chrono::{DateTime, Utc};
use tauri::Manager;

use crate::config::Config;
use crate::models::ImportStatus;
use crate::AppState;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SyncSourceId { ReadwiseHighlights, ReadwiseTweets, Zotero }

pub const SCHEDULABLE: [SyncSourceId; 3] =
    [SyncSourceId::ReadwiseHighlights, SyncSourceId::ReadwiseTweets, SyncSourceId::Zotero];

impl SyncSourceId {
    pub fn enabled(self, c: &Config) -> bool {
        match self { Self::ReadwiseHighlights => c.readwise_sync_enabled, Self::ReadwiseTweets => c.readwise_tweets_sync_enabled, Self::Zotero => c.zotero_sync_enabled }
    }
    pub fn interval_hours(self, c: &Config) -> u32 {
        match self { Self::ReadwiseHighlights => c.readwise_sync_interval_hours, Self::ReadwiseTweets => c.readwise_tweets_sync_interval_hours, Self::Zotero => c.zotero_sync_interval_hours }
    }
    pub fn last_sync(self, c: &Config) -> &str {
        match self { Self::ReadwiseHighlights => &c.readwise_last_sync, Self::ReadwiseTweets => &c.readwise_tweets_last_sync, Self::Zotero => &c.zotero_last_sync }
    }
}

/// Pure rule: a source is due if enabled, interval>0, and (no last_sync, or
/// now - last_sync >= interval hours).
pub fn is_due(id: SyncSourceId, c: &Config, now: DateTime<Utc>) -> bool {
    if !id.enabled(c) || id.interval_hours(c) == 0 { return false; }
    let last = id.last_sync(c);
    if last.is_empty() { return true; }
    match DateTime::parse_from_rfc3339(last) {
        Ok(t) => now.signed_duration_since(t.with_timezone(&Utc)).num_minutes() >= (id.interval_hours(c) as i64) * 60,
        Err(_) => true,
    }
}

/// Run one source by reusing the existing command internals. Each updates its
/// own cursor on success (the underlying commands already do for readwise /
/// readwise_tweets; zotero is full each run so no cursor is needed).
///
/// Takes `AppHandle` (not `State`) to avoid holding a `State` guard across
/// `.await` boundaries in the scheduler loop — each arm fetches a fresh `State`
/// immediately before its single `.await`, satisfying the borrow checker.
pub async fn run_source(
    id: SyncSourceId,
    handle: &tauri::AppHandle,
    window: tauri::WebviewWindow,
) -> Result<ImportStatus, String> {
    match id {
        SyncSourceId::ReadwiseHighlights => {
            let state = handle.state::<AppState>();
            crate::commands::import::run_import(state, window).await
        }
        SyncSourceId::ReadwiseTweets => {
            let state = handle.state::<AppState>();
            crate::commands::import::import_readwise_tweets(state, window).await
        }
        SyncSourceId::Zotero => {
            let state = handle.state::<AppState>();
            crate::commands::import::run_zotero_import(state, window).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn cfg(enabled: bool, hours: u32, last: &str) -> Config {
        let mut c = Config::default();
        c.readwise_tweets_sync_enabled = enabled;
        c.readwise_tweets_sync_interval_hours = hours;
        c.readwise_tweets_last_sync = last.to_string();
        c
    }

    #[test]
    fn due_logic() {
        let now = Utc::now();
        let id = SyncSourceId::ReadwiseTweets;
        assert!(!is_due(id, &cfg(false, 6, ""), now), "disabled → not due");
        assert!(!is_due(id, &cfg(true, 0, ""), now), "interval 0 → not due");
        assert!(is_due(id, &cfg(true, 6, ""), now), "enabled, never synced → due");
        let recent = (now - Duration::hours(2)).to_rfc3339();
        assert!(!is_due(id, &cfg(true, 6, &recent), now), "synced 2h ago, 6h interval → not due");
        let old = (now - Duration::hours(7)).to_rfc3339();
        assert!(is_due(id, &cfg(true, 6, &old), now), "synced 7h ago, 6h interval → due");
    }
}
