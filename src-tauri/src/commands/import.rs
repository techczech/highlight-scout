use std::collections::HashMap;

use chrono::Local;
use tauri::Emitter;

use crate::import::archive;
use crate::import::csv_import::{self, CsvInspect, CsvMapping};
use crate::import::json_format;
use crate::import::kindle;
use crate::import::readwise::ReadwiseClient;
use crate::import::readwise_seed::ReadwiseSeed;
use crate::import::x;
use crate::import::zotero::ZoteroImporter;
use crate::index::sqlite;
use crate::models::{Highlight, ImportStatus, Work};
use crate::AppState;

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
pub async fn export_json(
    path: String,
    state: tauri::State<'_, AppState>,
) -> Result<usize, String> {
    let now = chrono::Utc::now().to_rfc3339();
    let (works, highlights) = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        (
            sqlite::all_works(&conn).map_err(|e| e.to_string())?,
            sqlite::all_highlights(&conn).map_err(|e| e.to_string())?,
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
        Ok(s) => (s.works_imported, s.highlights_imported, "ok", s.message.clone()),
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
    let archive_path = archive_path.as_str();

    // Raw import-batch snapshot (ADR-0001 provenance).
    if let Some(raw) = raw_json {
        let stamp = Local::now().format("%Y-%m-%d-%H%M%S").to_string();
        let _ = archive::write_import_batch(archive_path, source, &stamp, raw);
    }

    let total = highlights_with_meta.len();
    progress(
        window,
        &format!("Writing archive: {} works…", works.len()),
        0,
        total,
    );

    // Group highlights by work for archive writing.
    let mut highlights_by_work: HashMap<String, Vec<&Highlight>> = HashMap::new();
    for (h, _, _) in highlights_with_meta {
        highlights_by_work.entry(h.work_id.clone()).or_default().push(h);
    }

    archive::write_archive(archive_path, works, &highlights_by_work)
        .map_err(|e| format!("Archive write failed: {}", e))?;

    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        for work in works {
            sqlite::upsert_work(&conn, work).map_err(|e| e.to_string())?;
        }
        for (i, (h, title, author)) in highlights_with_meta.iter().enumerate() {
            sqlite::upsert_highlight(&conn, h, title, author.as_deref())
                .map_err(|e| e.to_string())?;
            if i % 500 == 0 {
                progress(window, &format!("Indexing {}/{} highlights…", i, total), i, total);
            }
        }
    }
    progress(window, "Finishing…", total, total);

    let status = ImportStatus {
        works_imported: works.len(),
        highlights_imported: highlights_with_meta.len(),
        message: format!(
            "{} import complete: {} works, {} highlights",
            source,
            works.len(),
            highlights_with_meta.len()
        ),
    };

    let _ = window.emit("import:complete", &status);
    Ok(status)
}

/// Seed Readwise data from the existing highlights-archive SQLite — no API,
/// so it never hits the rate limit. Sets last_sync so subsequent API updates
/// are incremental.
#[tauri::command]
pub async fn run_readwise_seed(
    state: tauri::State<'_, AppState>,
    window: tauri::WebviewWindow,
) -> Result<ImportStatus, String> {
    let started = std::time::Instant::now();
    let result = async {
        let archive = state.config().readwise_archive_path;
        progress(&window, "Reading Readwise archive…", 0, 0);
        let seed = ReadwiseSeed::new(&archive);
        let (works, highlights_with_meta, max_updated) =
            seed.import_all().map_err(|e| e.to_string())?;
        let status = persist(&state, "readwise", &works, &highlights_with_meta, None, &window)?;
        set_last_sync(&state, &max_updated);
        Ok::<ImportStatus, String>(status)
    }
    .await;
    log_outcome("readwise-seed", started, &result);
    result
}

#[tauri::command]
pub async fn run_import(
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
        let updated_after = if last_sync.is_empty() { None } else { Some(last_sync.as_str()) };
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
        let (works, highlights_with_meta, raw_json) = client
            .import_export(updated_after)
            .await
            .map_err(|e| e.to_string())?;

        if works.is_empty() {
            let done = ImportStatus { works_imported: 0, highlights_imported: 0, message: "Already up to date".into() };
            let _ = window.emit("import:complete", &done);
            set_last_sync(&state, &sync_start);
            return Ok(done);
        }

        let status = persist(&state, "readwise", &works, &highlights_with_meta, Some(&raw_json), &window)?;
        set_last_sync(&state, &sync_start);

        // Full article bodies (ADR-0007 MVP). Additive and resilient: a Reader
        // failure must not fail the highlight import that already succeeded.
        progress(&window, "Fetching full article text…", 0, 0);
        let archive_path = state.config().archive_path;
        let final_message = match client.fetch_reader_fulltext().await {
            Ok(by_url) => {
                let mut written = 0usize;
                for work in &works {
                    if let Some(url) = &work.url {
                        if let Some(md) = by_url.get(url) {
                            if archive::write_fulltext(&archive_path, &work.slug, md).is_ok() {
                                written += 1;
                            }
                        }
                    }
                }
                format!("{} · {} full texts saved", status.message, written)
            }
            Err(e) => format!("{} · full text skipped ({})", status.message, e),
        };

        let done = ImportStatus {
            works_imported: status.works_imported,
            highlights_imported: status.highlights_imported,
            message: final_message,
        };
        let _ = window.emit("import:complete", &done);
        Ok(done)
    }
    .await;
    log_outcome("readwise", started, &result);
    result
}

#[tauri::command]
pub async fn run_zotero_import(
    state: tauri::State<'_, AppState>,
    window: tauri::WebviewWindow,
) -> Result<ImportStatus, String> {
    let started = std::time::Instant::now();
    let result = async {
        progress(&window, "Reading Zotero database…", 0, 0);
        let cfg = state.config();
        let importer = ZoteroImporter::with_archive(cfg.zotero_db_path, cfg.archive_path);
        let (works, highlights_with_meta) = importer.import_all().map_err(|e| e.to_string())?;
        persist(&state, "zotero", &works, &highlights_with_meta, None, &window)
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
