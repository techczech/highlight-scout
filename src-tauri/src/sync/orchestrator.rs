// Sync-pass orchestration, free of Tauri: which sources run, running them in
// order with one failure never stopping the rest, turning raw importer errors
// into plain words, and the summary line the toast shows. The Tauri glue in
// `sync/mod.rs` supplies the real runner; tests supply fakes.

use std::future::Future;

use serde::Serialize;

use super::SyncSourceId;
use crate::config::Config;

/// What one source did in a pass.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SourceResult {
    pub source: &'static str,
    pub label: &'static str,
    /// New highlights (or saved tweets / Zotero items) added by this run.
    pub added: usize,
    /// Plain-words failure, e.g. "token rejected (401)". None on success.
    pub error: Option<String>,
    pub finished_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncReport {
    /// Monotonic per process, so the window can tell reports apart.
    pub seq: u64,
    /// "launch" | "interval" | "manual"
    pub trigger: String,
    pub started_at: String,
    pub finished_at: String,
    pub results: Vec<SourceResult>,
    pub summary: String,
}

/// A source is configured when its credentials or path exist. The old
/// per-source enable flags play no part.
pub fn configured_sources(c: &Config, path_exists: impl Fn(&str) -> bool) -> Vec<SyncSourceId> {
    let mut out = Vec::new();
    if !c.readwise_api_key.trim().is_empty() {
        out.push(SyncSourceId::ReadwiseHighlights);
        out.push(SyncSourceId::ReadwiseTweets);
    }
    let z = c.zotero_db_path.trim();
    if !z.is_empty() && path_exists(z) {
        out.push(SyncSourceId::Zotero);
    }
    out
}

/// Run each source in order. `run` returns the number of new items or the raw
/// error; a failure is recorded and the next source still runs.
///
/// `on_result` runs as soon as each source finishes, before the next starts,
/// so that source's state is recorded even if a later one hangs or the app
/// quits mid-pass.
pub async fn run_pass<F, Fut>(
    sources: &[SyncSourceId],
    mut run: F,
    now: impl Fn() -> String,
    mut on_result: impl FnMut(&SourceResult),
) -> Vec<SourceResult>
where
    F: FnMut(SyncSourceId) -> Fut,
    Fut: Future<Output = Result<usize, String>>,
{
    let mut results = Vec::with_capacity(sources.len());
    for &id in sources {
        let outcome = run(id).await;
        let (added, error) = match outcome {
            Ok(n) => (n, None),
            Err(e) => (0, Some(plain_error(id, &e))),
        };
        let result = SourceResult {
            source: id.key(),
            label: id.label(),
            added,
            error,
            finished_at: now(),
        };
        on_result(&result);
        results.push(result);
    }
    results
}

fn count_phrase(id: SyncSourceId, n: usize) -> String {
    let (one, many) = match id {
        SyncSourceId::ReadwiseHighlights => ("highlight", "highlights"),
        SyncSourceId::ReadwiseTweets => ("saved tweet", "saved tweets"),
        SyncSourceId::Zotero => ("Zotero item", "Zotero items"),
    };
    match n {
        0 => format!("no new {}", many),
        1 => format!("1 {}", one),
        _ => format!("{} {}", n, many),
    }
}

/// The toast line: "Added N highlights · M saved tweets · K Zotero items".
/// Names only sources that ran and succeeded; zero counts read "no new …";
/// nothing new anywhere reads "Up to date". Failures are shown separately
/// (the red line), so they are left out here.
pub fn summary_text(results: &[SourceResult]) -> String {
    if results.is_empty() {
        return "No sources configured".to_string();
    }
    let ok: Vec<(SyncSourceId, usize)> = results
        .iter()
        .filter(|r| r.error.is_none())
        .filter_map(|r| SyncSourceId::from_key(r.source).map(|id| (id, r.added)))
        .collect();
    if ok.is_empty() {
        return "Sync failed".to_string();
    }
    if ok.iter().all(|(_, n)| *n == 0) {
        return "Up to date".to_string();
    }
    let parts: Vec<String> = ok.iter().map(|(id, n)| count_phrase(*id, *n)).collect();
    let joined = parts.join(" · ");
    if ok[0].1 > 0 {
        format!("Added {}", joined)
    } else {
        // "no new highlights · 5 saved tweets" reads badly after "Added".
        let mut c = joined.chars();
        match c.next() {
            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            None => joined,
        }
    }
}

/// Turn a raw importer error into plain words. Never includes credentials:
/// the importers' errors carry HTTP status lines and paths, not headers.
pub fn plain_error(id: SyncSourceId, raw: &str) -> String {
    let lower = raw.to_lowercase();
    let status = http_status(raw);
    let readwise = matches!(
        id,
        SyncSourceId::ReadwiseHighlights | SyncSourceId::ReadwiseTweets
    );
    if lower.contains("no readwise api key") {
        return "no Readwise token set (Settings → Sources)".to_string();
    }
    if let Some(code) = status {
        return match code {
            401 | 403 => format!("token rejected ({})", code),
            429 => "Readwise is rate-limiting requests (429); the next sync will retry".to_string(),
            500..=599 => format!("Readwise server error ({}); the next sync will retry", code),
            _ => format!("request failed ({})", code),
        };
    }
    if readwise
        && (lower.contains("error sending request")
            || lower.contains("dns")
            || lower.contains("connect")
            || lower.contains("timed out"))
    {
        return "could not reach Readwise (offline?)".to_string();
    }
    if id == SyncSourceId::Zotero {
        if lower.contains("locked") || lower.contains("busy") {
            return "Zotero database is busy; close Zotero or wait for the next sync".to_string();
        }
        if lower.contains("unable to open") || lower.contains("no such file") {
            return "could not open the Zotero database".to_string();
        }
    }
    let first = raw.lines().next().unwrap_or(raw).trim();
    let mut s: String = first.chars().take(160).collect();
    if first.chars().count() > 160 {
        s.push('…');
    }
    s
}

/// Find an HTTP error status in strings like "Readwise export error: 401 Unauthorized".
fn http_status(raw: &str) -> Option<u16> {
    let lower = raw.to_lowercase();
    if !(lower.contains("error:") || lower.contains("status")) {
        return None;
    }
    raw.split(|c: char| !c.is_ascii_digit())
        .filter(|t| t.len() == 3)
        .filter_map(|t| t.parse::<u16>().ok())
        .find(|c| (400..600).contains(c))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn block_on<F: Future>(f: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(f)
    }

    fn now() -> String {
        "2026-09-27T09:00:00Z".to_string()
    }

    fn res(id: SyncSourceId, added: usize, error: Option<&str>) -> SourceResult {
        SourceResult {
            source: id.key(),
            label: id.label(),
            added,
            error: error.map(String::from),
            finished_at: now(),
        }
    }

    #[test]
    fn configured_sources_follow_credentials_and_paths_not_flags() {
        let mut c = Config::default();
        c.readwise_api_key = String::new();
        c.zotero_db_path = "/z/zotero.sqlite".into();
        assert!(configured_sources(&c, |_| false).is_empty());

        c.readwise_api_key = "tok".into();
        assert_eq!(
            configured_sources(&c, |_| false),
            vec![SyncSourceId::ReadwiseHighlights, SyncSourceId::ReadwiseTweets]
        );
        assert_eq!(
            configured_sources(&c, |p| p == "/z/zotero.sqlite"),
            vec![
                SyncSourceId::ReadwiseHighlights,
                SyncSourceId::ReadwiseTweets,
                SyncSourceId::Zotero
            ]
        );
        c.readwise_api_key = String::new();
        assert_eq!(configured_sources(&c, |_| true), vec![SyncSourceId::Zotero]);
    }

    #[test]
    fn each_source_is_recorded_before_the_next_one_runs() {
        let log = RefCell::new(Vec::<String>::new());
        let sources = [SyncSourceId::ReadwiseHighlights, SyncSourceId::Zotero];
        block_on(run_pass(
            &sources,
            |id| {
                log.borrow_mut().push(format!("run {}", id.key()));
                async move { Ok(1) }
            },
            now,
            |r| log.borrow_mut().push(format!("recorded {}", r.source)),
        ));
        assert_eq!(
            *log.borrow(),
            [
                "run readwise",
                "recorded readwise",
                "run zotero",
                "recorded zotero"
            ]
        );
    }

    #[test]
    fn run_pass_runs_every_source_and_one_failure_does_not_stop_the_rest() {
        let calls = RefCell::new(Vec::new());
        let sources = [
            SyncSourceId::ReadwiseHighlights,
            SyncSourceId::ReadwiseTweets,
            SyncSourceId::Zotero,
        ];
        let results = block_on(run_pass(
            &sources,
            |id| {
                calls.borrow_mut().push(id);
                async move {
                    match id {
                        SyncSourceId::ReadwiseHighlights => Ok(732),
                        SyncSourceId::ReadwiseTweets => {
                            Err("Readwise Reader error: 401 Unauthorized".to_string())
                        }
                        SyncSourceId::Zotero => Ok(3),
                    }
                }
            },
            now,
            |_| {},
        ));
        assert_eq!(*calls.borrow(), sources.to_vec());
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].added, 732);
        assert_eq!(results[1].error.as_deref(), Some("token rejected (401)"));
        assert_eq!(results[1].added, 0);
        assert_eq!(results[2].added, 3);
        assert!(results[2].error.is_none());
        assert_eq!(
            summary_text(&results),
            "Added 732 highlights · 3 Zotero items"
        );
    }

    #[test]
    fn summary_counts_and_zeroes() {
        use SyncSourceId::*;
        assert_eq!(
            summary_text(&[res(ReadwiseHighlights, 732, None), res(ReadwiseTweets, 1, None), res(Zotero, 0, None)]),
            "Added 732 highlights · 1 saved tweet · no new Zotero items"
        );
        assert_eq!(
            summary_text(&[res(ReadwiseHighlights, 0, None), res(ReadwiseTweets, 5, None)]),
            "No new highlights · 5 saved tweets"
        );
        assert_eq!(
            summary_text(&[res(ReadwiseHighlights, 1, None)]),
            "Added 1 highlight"
        );
        assert_eq!(
            summary_text(&[res(ReadwiseHighlights, 0, None), res(ReadwiseTweets, 0, None), res(Zotero, 0, None)]),
            "Up to date"
        );
        assert_eq!(summary_text(&[res(Zotero, 0, None)]), "Up to date");
        assert_eq!(summary_text(&[res(Zotero, 0, Some("x"))]), "Sync failed");
        assert_eq!(summary_text(&[]), "No sources configured");
    }

    #[test]
    fn plain_errors() {
        use SyncSourceId::*;
        assert_eq!(
            plain_error(ReadwiseHighlights, "Readwise export error: 401 Unauthorized"),
            "token rejected (401)"
        );
        assert_eq!(
            plain_error(ReadwiseHighlights, "Readwise export error: 403 Forbidden"),
            "token rejected (403)"
        );
        assert!(plain_error(ReadwiseTweets, "Readwise Reader error: 429 Too Many Requests").contains("(429)"));
        assert!(plain_error(ReadwiseHighlights, "Readwise export error: 502 Bad Gateway").contains("(502)"));
        assert_eq!(
            plain_error(ReadwiseHighlights, "No Readwise API key configured. Open Settings (⌘,)."),
            "no Readwise token set (Settings → Sources)"
        );
        assert_eq!(
            plain_error(ReadwiseHighlights, "error sending request for url (https://readwise.io/api/v2/export/)"),
            "could not reach Readwise (offline?)"
        );
        assert!(plain_error(Zotero, "database is locked").starts_with("Zotero database is busy"));
        let long = "x".repeat(300);
        assert_eq!(plain_error(Zotero, &long).chars().count(), 161);
    }
}
