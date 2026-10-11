use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use chrono::Local;
use tauri::Emitter;

use crate::busy::{Claim, Op};
use crate::import::csv_import::{self, CsvInspect, CsvMapping};
use crate::import::json_format;
use crate::import::kindle;
use crate::import::readwise::ReadwiseClient;
use crate::import::x;
use crate::import::zotero::ZoteroImporter;
use crate::models::{Highlight, ImportStatus, Work};
use crate::AppState;
use scout_archive::markdown;
use scout_index::sqlite;

#[tauri::command]
pub async fn inspect_csv(path: String) -> Result<CsvInspect, String> {
    csv_import::inspect(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn import_csv(
    path: String,
    mapping: CsvMapping,
    state: tauri::State<'_, AppState>,
    window: tauri::WebviewWindow,
) -> Result<ImportStatus, String> {
    let started = std::time::Instant::now();
    let result = async {
        progress(&window, "Reading CSV…", 0, 0);
        let (works, h) = csv_import::import(&path, &mapping).map_err(|e| e.to_string())?;
        persist(&state, "csv", &works, &h, None, &window)
    }
    .await;
    log_outcome("csv", started, &result);
    result
}

#[tauri::command]
pub async fn import_kindle(
    path: String,
    state: tauri::State<'_, AppState>,
    window: tauri::WebviewWindow,
) -> Result<ImportStatus, String> {
    let started = std::time::Instant::now();
    let result = async {
        progress(&window, "Reading Kindle clippings…", 0, 0);
        let (works, h) = kindle::import(&path).map_err(|e| e.to_string())?;
        persist(&state, "kindle", &works, &h, None, &window)
    }
    .await;
    log_outcome("kindle", started, &result);
    result
}

#[tauri::command]
pub async fn import_x(
    path: String,
    state: tauri::State<'_, AppState>,
    window: tauri::WebviewWindow,
) -> Result<ImportStatus, String> {
    // One writer at a time: this import rewrites tweet files in place.
    let _claim = state.inner().busy.try_claim(Op::TweetsImport)?;
    let started = std::time::Instant::now();
    let result = async {
        progress(&window, "Reading saved tweets…", 0, 0);
        let (works, h) = x::import(&path).map_err(|e| e.to_string())?;
        persist(&state, "x", &works, &h, None, &window)
    }
    .await;
    log_outcome("x", started, &result);
    result
}

#[tauri::command]
pub async fn import_json(
    path: String,
    state: tauri::State<'_, AppState>,
    window: tauri::WebviewWindow,
) -> Result<ImportStatus, String> {
    // A JSON export can hold tweets, which are merged into existing files.
    let _claim = state.inner().busy.try_claim(Op::FileImport)?;
    let started = std::time::Instant::now();
    let result = async {
        progress(&window, "Reading JSON…", 0, 0);
        let (works, h) = json_format::import(&path).map_err(|e| e.to_string())?;
        persist(&state, "json", &works, &h, None, &window)
    }
    .await;
    log_outcome("json", started, &result);
    result
}

#[tauri::command]
pub async fn export_json(path: String, state: tauri::State<'_, AppState>) -> Result<usize, String> {
    let now = chrono::Utc::now().to_rfc3339();
    let (works, highlights) = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        (
            sqlite::all_containers(&conn).map_err(|e| e.to_string())?,
            sqlite::all_records(&conn).map_err(|e| e.to_string())?,
        )
    };
    let count = highlights.len();
    let export = json_format::build_export(works, highlights, &now);
    let json = serde_json::to_string_pretty(&export).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(count)
}

/// Emit a structured progress event. current/total drive the progress bar
/// (pass 0 total for an indeterminate step).
fn progress(window: &tauri::WebviewWindow, message: &str, current: usize, total: usize) {
    let _ = window.emit(
        "import:progress",
        serde_json::json!({ "message": message, "current": current, "total": total }),
    );
}

/// Record an import run (success or error) to the persistent import log.
fn log_outcome(source: &str, started: std::time::Instant, result: &Result<ImportStatus, String>) {
    let (works, highlights, status, message) = match result {
        Ok(s) => (
            s.works_imported,
            s.highlights_imported,
            "ok",
            s.message.clone(),
        ),
        Err(e) => (0, 0, "error", e.clone()),
    };
    crate::import_log::append(&crate::import_log::ImportLogEntry {
        timestamp: Local::now().to_rfc3339(),
        source: source.to_string(),
        works,
        highlights,
        status: status.to_string(),
        message,
        duration_ms: started.elapsed().as_millis() as u64,
    });
}

#[tauri::command]
pub async fn get_import_log(
    _state: tauri::State<'_, AppState>,
) -> Result<Vec<crate::import_log::ImportLogEntry>, String> {
    Ok(crate::import_log::read_recent(100))
}

/// Persist a new last-sync cursor to config (in-memory + disk).
fn set_last_sync(state: &tauri::State<'_, AppState>, ts: &str) {
    if ts.is_empty() {
        return;
    }
    if let Ok(mut cfg) = state.config.write() {
        cfg.readwise_last_sync = ts.to_string();
        let _ = crate::config::save(&cfg);
    }
}

/// Shared persistence step for any source: write the raw batch snapshot,
/// the v2 Archive, and the SQLite index.
fn persist(
    state: &tauri::State<'_, AppState>,
    source: &str,
    works: &[Work],
    highlights_with_meta: &[(Highlight, String, Option<String>)],
    raw_json: Option<&str>,
    window: &tauri::WebviewWindow,
) -> Result<ImportStatus, String> {
    let archive_path = state.config().archive_path;
    let no_deletions = HashSet::new();
    let (status, tweets) = persist_core_reported(
        &state.db,
        &archive_path,
        PersistBatch {
            source,
            works,
            highlights: highlights_with_meta,
            archive_records: None,
            deleted_ids: &no_deletions,
            raw_json,
        },
        &|m, c, t| progress(window, m, c, t),
    )?;
    log_tweet_report(&tweets);
    after_persist(window, &status);
    Ok(status)
}

/// Record tweets the archive holds twice (or could not be updated) in the
/// import log, each by its key.
pub(crate) fn log_tweet_report(tweets: &crate::import::tweet_upsert::TweetReport) {
    let Some(message) = tweets.log_message() else {
        return;
    };
    crate::import_log::append(&crate::import_log::ImportLogEntry {
        timestamp: Local::now().to_rfc3339(),
        source: "x-existing-files".to_string(),
        works: tweets.duplicate_keys.len()
            + tweets.skipped.len()
            + tweets.index_slug_conflicts.len(),
        highlights: 0,
        status: "ok".to_string(),
        message,
        duration_ms: 0,
    });
}

/// Completion event plus auto-OCR of new image highlights (gated by
/// ocr_on_import; macOS only).
fn after_persist(window: &tauri::WebviewWindow, status: &ImportStatus) {
    let _ = window.emit("import:complete", status);
    use tauri::Manager;
    crate::ocr::maybe_auto_ocr(&window.app_handle(), window.clone());
}

/// One batch to persist.
pub(crate) struct PersistBatch<'a> {
    pub source: &'a str,
    pub works: &'a [Work],
    /// Upserted into the index as given.
    pub highlights: &'a [(Highlight, String, Option<String>)],
    /// What the archive files are rendered from, when not the batch itself
    /// (a Readwise sync renders from the merged, ordered set).
    pub archive_records: Option<&'a [Highlight]>,
    /// Removed from the index (`highlights` and `search_index`), so a later
    /// incremental merge cannot read them back.
    pub deleted_ids: &'a HashSet<String>,
    pub raw_json: Option<&'a str>,
}

/// Window-free persistence: raw snapshot, archive files, index.
pub(crate) fn persist_core(
    db: &Mutex<rusqlite::Connection>,
    archive_path: &str,
    batch: PersistBatch<'_>,
    progress: &dyn Fn(&str, usize, usize),
) -> Result<ImportStatus, String> {
    persist_core_reported(db, archive_path, batch, progress).map(|(status, _)| status)
}

/// `persist_core`, also returning what happened to the batch's tweets. Every
/// tweet import (the X file, Readwise Reader, the headless `--import-x`)
/// comes through here, so each finds an existing tweet file by its key.
pub(crate) fn persist_core_reported(
    db: &Mutex<rusqlite::Connection>,
    archive_path: &str,
    batch: PersistBatch<'_>,
    progress: &dyn Fn(&str, usize, usize),
) -> Result<(ImportStatus, crate::import::tweet_upsert::TweetReport), String> {
    let PersistBatch {
        source,
        works,
        highlights: highlights_with_meta,
        archive_records,
        deleted_ids,
        raw_json,
    } = batch;
    let mut works = crate::archive_meta::dedupe_works(works.to_vec());
    {
        let conn = db.lock().map_err(|e| e.to_string())?;
        crate::import::readwise_identity::resolve_slugs(&conn, archive_path, &mut works);
    }

    // Raw import-batch snapshot (ADR-0001 provenance).
    if let Some(raw) = raw_json {
        let stamp = Local::now().format("%Y-%m-%d-%H%M%S").to_string();
        let _ = markdown::write_import_batch(archive_path, source, &stamp, raw);
    }

    let total = highlights_with_meta.len();
    progress(
        &format!("Writing archive: {} works…", works.len()),
        0,
        total,
    );

    // Group highlights by work for archive writing.
    let mut highlights_by_work: HashMap<String, Vec<&Highlight>> = HashMap::new();
    let batch_records: Vec<&Highlight> = match archive_records {
        Some(list) => list.iter().collect(),
        None => highlights_with_meta.iter().map(|(h, _, _)| h).collect(),
    };
    for h in batch_records {
        highlights_by_work
            .entry(h.container_id.clone())
            .or_default()
            .push(h);
    }

    // Tweets are merged into the file that already holds their key; `works`
    // comes back pointing at those files, `written.highlights` holding each
    // tweet highlight as the archive now has it.
    let written =
        crate::import::tweet_upsert::write_archive(archive_path, &mut works, &highlights_by_work)
            .map_err(|e| format!("Archive write failed: {}", e))?;
    let tweet_works: HashMap<&str, &Work> = works
        .iter()
        .filter(|w| crate::import::tweet_upsert::is_tweet(w))
        .map(|w| (w.id.as_str(), w))
        .collect();

    let mut index_conflicts: HashSet<String> = HashSet::new();
    {
        let conn = db.lock().map_err(|e| e.to_string())?;
        delete_highlights(&conn, deleted_ids).map_err(|e| e.to_string())?;
        for work in &works {
            if written.skipped_works.contains(&work.id) {
                continue;
            }
            if crate::import::tweet_upsert::is_tweet(work)
                && !point_tweet_row(&conn, work).map_err(|e| e.to_string())?
            {
                index_conflicts.insert(work.id.clone());
                continue;
            }
            sqlite::upsert_container(&conn, work).map_err(|e| e.to_string())?;
        }
        for (i, (h, title, author)) in highlights_with_meta.iter().enumerate() {
            // A tweet whose file was left alone, or whose index row could
            // not be pointed at its file, keeps the index entry it had.
            if deleted_ids.contains(&h.id)
                || written.skipped_works.contains(&h.container_id)
                || index_conflicts.contains(&h.container_id)
            {
                continue;
            }
            match (
                written.highlights.get(&h.id),
                tweet_works.get(h.container_id.as_str()),
            ) {
                (Some(merged), Some(w)) => {
                    sqlite::upsert_record(&conn, merged, &w.title, w.author.as_deref())
                }
                _ => sqlite::upsert_record(&conn, h, title, author.as_deref()),
            }
            .map_err(|e| e.to_string())?;
            if i % 500 == 0 {
                progress(&format!("Indexing {}/{} highlights…", i, total), i, total);
            }
        }
    }
    progress("Finishing…", total, total);

    let mut message = format!(
        "{} import complete: {} works, {} highlights",
        source,
        works.len(),
        highlights_with_meta.len()
    );
    let mut tweets = written.tweets;
    tweets.index_slug_conflicts = works
        .iter()
        .filter(|w| index_conflicts.contains(&w.id))
        .map(crate::import::tweet_upsert::key)
        .collect();
    if !tweets.duplicate_keys.is_empty() {
        message.push_str(&format!(
            " · {} tweets have more than one file in the archive (see the import log)",
            tweets.duplicate_keys.len()
        ));
    }
    let left = tweets.skipped.len() + tweets.index_slug_conflicts.len();
    if left > 0 {
        message.push_str(&format!(
            " · {left} tweets could not be updated (see the import log)"
        ));
    }
    Ok((
        ImportStatus {
            works_imported: works.len(),
            highlights_imported: highlights_with_meta.len(),
            message,
        },
        tweets,
    ))
}

/// Make sure a tweet's index row can name the file the archive holds for
/// it. scout-index keeps a work's first slug on upsert and has no way to
/// move it, so a row with another slug is moved here. Returns false, and
/// changes nothing, when another row already holds that slug or the move
/// does not land on exactly one row: the caller then leaves the index as it
/// was for this tweet.
fn point_tweet_row(conn: &rusqlite::Connection, work: &Work) -> rusqlite::Result<bool> {
    use rusqlite::OptionalExtension;
    let held_by_other: Option<String> = conn
        .query_row(
            "SELECT id FROM works WHERE slug = ?1 AND id != ?2",
            [&work.slug, &work.id],
            |r| r.get(0),
        )
        .optional()?;
    if held_by_other.is_some() {
        return Ok(false);
    }
    let current: Option<String> = conn
        .query_row("SELECT slug FROM works WHERE id = ?1", [&work.id], |r| {
            r.get(0)
        })
        .optional()?;
    match current {
        Some(slug) if slug != work.slug => Ok(matches!(
            conn.execute(
                "UPDATE works SET slug = ?2 WHERE id = ?1",
                [&work.id, &work.slug],
            ),
            Ok(1)
        )),
        _ => Ok(true),
    }
}

/// Remove highlights from the index: the row and its full-text entry.
/// scout-index has no delete, so this runs over the same connection.
pub(crate) fn delete_highlights(
    conn: &rusqlite::Connection,
    ids: &HashSet<String>,
) -> rusqlite::Result<usize> {
    let mut removed = 0;
    for id in ids {
        conn.execute("DELETE FROM search_index WHERE highlight_id = ?1", [id])?;
        removed += conn.execute("DELETE FROM highlights WHERE id = ?1", [id])?;
    }
    Ok(removed)
}

#[tauri::command]
pub async fn run_import(
    state: tauri::State<'_, AppState>,
    window: tauri::WebviewWindow,
) -> Result<ImportStatus, String> {
    let claim = state.inner().busy.try_claim(Op::ReadwiseImport)?;
    run_import_claimed(&claim, state.clone(), window).await
}

/// The Readwise highlight sync, under a claim the caller already holds (a
/// manual import's own, or the scheduled pass's).
pub(crate) async fn run_import_claimed(
    claim: &Claim<'_>,
    state: tauri::State<'_, AppState>,
    window: tauri::WebviewWindow,
) -> Result<ImportStatus, String> {
    let started = std::time::Instant::now();
    let result = async {
        let cfg = state.config();
        let api_key = cfg.readwise_api_key;
        if api_key.is_empty() {
            return Err("No Readwise API key configured. Open Settings (⌘,).".to_string());
        }

        // Incremental when we have a cursor; full export otherwise.
        let last_sync = cfg.readwise_last_sync.clone();
        let updated_after = if last_sync.is_empty() {
            None
        } else {
            Some(last_sync.as_str())
        };
        let sync_start = chrono::Utc::now().to_rfc3339();

        progress(
            &window,
            if updated_after.is_some() {
                "Updating from Readwise (changes only)…"
            } else {
                "Importing from Readwise (full export)…"
            },
            0,
            0,
        );

        let client = ReadwiseClient::new(api_key);
        let archive_path = cfg.archive_path.clone();
        let outcome = crate::import::readwise_sync::sync_readwise(
            claim,
            &client,
            &state.db,
            &archive_path,
            updated_after,
            &|m, c, t| progress(&window, m, c, t),
        )
        .await?;
        let mut status = outcome.status;
        let works = outcome.works;
        if outcome.fell_back_to_full {
            status
                .message
                .push_str(" · index was missing highlights, so a full export ran");
        }

        if works.is_empty() {
            let _ = window.emit("import:complete", &status);
            set_last_sync(&state, &sync_start);
            return Ok(status);
        }
        after_persist(&window, &status);
        set_last_sync(&state, &sync_start);

        // Full article bodies (ADR-0007 MVP), in the background so the import
        // (and the sync pass's per-source state) completes without waiting on
        // Reader. Only documents updated since this sync's cursor are fetched.
        // Additive and resilient: a Reader failure never fails the import.
        spawn_fulltext(
            &window,
            state.config().readwise_api_key,
            archive_path,
            updated_after.map(String::from),
            works,
        );
        Ok(status)
    }
    .await;
    log_outcome("readwise", started, &result);
    result
}

/// Fetch and save Reader full texts for `works` off the import path. The
/// outcome goes to the import log and a `fulltext:complete` event.
fn spawn_fulltext(
    window: &tauri::WebviewWindow,
    api_key: String,
    archive_path: String,
    updated_after: Option<String>,
    works: Vec<Work>,
) {
    let window = window.clone();
    tauri::async_runtime::spawn(async move {
        let started = std::time::Instant::now();
        let client = ReadwiseClient::new(api_key);
        let result = match client.fetch_reader_fulltext(updated_after.as_deref()).await {
            Ok(by_url) => {
                let written = works
                    .iter()
                    .filter_map(|w| w.url.as_ref().and_then(|u| by_url.get(u)).map(|md| (w, md)))
                    .filter(|(w, md)| {
                        crate::archive_meta::write_fulltext(&archive_path, &w.slug, md).is_ok()
                    })
                    .count();
                Ok(ImportStatus {
                    works_imported: written,
                    highlights_imported: 0,
                    message: format!("{} full texts saved", written),
                })
            }
            Err(e) => Err(format!("full text skipped ({})", e)),
        };
        log_outcome("readwise-fulltext", started, &result);
        let _ = window.emit(
            "fulltext:complete",
            serde_json::json!({ "message": match &result { Ok(s) => s.message.clone(), Err(e) => e.clone() } }),
        );
    });
}

/// Fold duplicate Readwise work files into their oldest file. Never runs on
/// its own: only when the driver invokes it (or the CLI flag). `dry_run`
/// lists the pairs and block counts and writes nothing.
#[tauri::command]
pub async fn merge_duplicate_readwise_works(
    dry_run: bool,
    state: tauri::State<'_, AppState>,
) -> Result<crate::import::readwise_identity::MergeReport, String> {
    crate::import::readwise_identity::merge_duplicates_claimed(
        &state.busy,
        &state.config().archive_path,
        &state.db,
        dry_run,
    )
}

#[tauri::command]
pub async fn import_readwise_tweets(
    state: tauri::State<'_, AppState>,
    window: tauri::WebviewWindow,
) -> Result<ImportStatus, String> {
    let claim = state.inner().busy.try_claim(Op::TweetsImport)?;
    import_readwise_tweets_claimed(&claim, state.clone(), window).await
}

pub(crate) async fn import_readwise_tweets_claimed(
    _claim: &Claim<'_>,
    state: tauri::State<'_, AppState>,
    window: tauri::WebviewWindow,
) -> Result<ImportStatus, String> {
    let started = std::time::Instant::now();
    let result = async {
        let cfg = state.config();
        if cfg.readwise_api_key.is_empty() {
            return Err("No Readwise API key configured. Open Settings (⌘,).".to_string());
        }
        let after = cfg.readwise_tweets_last_sync.clone();
        let updated_after: Option<&str> = if after.is_empty() {
            None
        } else {
            Some(after.as_str())
        };
        let sync_start = chrono::Utc::now().to_rfc3339();
        progress(&window, "Importing saved tweets from Readwise…", 0, 0);
        let (works, h) =
            crate::import::readwise_tweets::import(&cfg.readwise_api_key, updated_after)
                .await
                .map_err(|e| e.to_string())?;
        let status = persist(&state, "x", &works, &h, None, &window)?;
        if let Ok(mut c) = state.config.write() {
            c.readwise_tweets_last_sync = sync_start;
            let _ = crate::config::save(&c);
        }
        Ok::<ImportStatus, String>(status)
    }
    .await;
    log_outcome("readwise-tweets", started, &result);
    result
}

#[tauri::command]
pub async fn run_zotero_import(
    state: tauri::State<'_, AppState>,
    window: tauri::WebviewWindow,
) -> Result<ImportStatus, String> {
    let claim = state.inner().busy.try_claim(Op::ZoteroImport)?;
    run_zotero_import_claimed(&claim, state.clone(), window).await
}

pub(crate) async fn run_zotero_import_claimed(
    _claim: &Claim<'_>,
    state: tauri::State<'_, AppState>,
    window: tauri::WebviewWindow,
) -> Result<ImportStatus, String> {
    let started = std::time::Instant::now();
    let result = async {
        let sync_start = chrono::Utc::now().to_rfc3339();
        progress(&window, "Reading Zotero database…", 0, 0);
        let cfg = state.config();
        let importer = ZoteroImporter::with_archive(cfg.zotero_db_path, cfg.archive_path);
        let (works, highlights_with_meta) = importer.import_all().map_err(|e| e.to_string())?;
        let status = persist(
            &state,
            "zotero",
            &works,
            &highlights_with_meta,
            None,
            &window,
        )?;
        if let Ok(mut c) = state.config.write() {
            c.zotero_last_sync = sync_start;
            let _ = crate::config::save(&c);
        }
        Ok::<ImportStatus, String>(status)
    }
    .await;
    log_outcome("zotero", started, &result);
    result
}

#[tauri::command]
pub async fn get_config(state: tauri::State<'_, AppState>) -> Result<serde_json::Value, String> {
    let c = state.config();
    Ok(serde_json::json!({
        "archive_path": c.archive_path,
        "has_api_key": !c.readwise_api_key.is_empty(),
        "shortcut": c.shortcut,
        "zotero_db_path": c.zotero_db_path,
    }))
}
