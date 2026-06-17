use std::collections::HashMap;

use chrono::Local;
use tauri::Emitter;

use crate::import::archive;
use crate::import::readwise::ReadwiseClient;
use crate::import::readwise_seed::ReadwiseSeed;
use crate::import::zotero::ZoteroImporter;
use crate::index::sqlite;
use crate::models::{Highlight, ImportStatus, Work};
use crate::AppState;

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

    let _ = window.emit(
        "import:progress",
        format!(
            "Fetched {} works, {} highlights. Writing archive…",
            works.len(),
            highlights_with_meta.len()
        ),
    );

    // Group highlights by work for archive writing.
    let mut highlights_by_work: HashMap<String, Vec<&Highlight>> = HashMap::new();
    for (h, _, _) in highlights_with_meta {
        highlights_by_work.entry(h.work_id.clone()).or_default().push(h);
    }

    archive::write_archive(archive_path, works, &highlights_by_work)
        .map_err(|e| format!("Archive write failed: {}", e))?;

    let _ = window.emit("import:progress", "Updating search index…");

    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        for work in works {
            sqlite::upsert_work(&conn, work).map_err(|e| e.to_string())?;
        }
        for (h, title, author) in highlights_with_meta {
            sqlite::upsert_highlight(&conn, h, title, author.as_deref())
                .map_err(|e| e.to_string())?;
        }
    }

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
    let archive = state.config().readwise_archive_path;
    let _ = window.emit("import:progress", "Seeding from Readwise archive…");

    let seed = ReadwiseSeed::new(&archive);
    let (works, highlights_with_meta, max_updated) =
        seed.import_all().map_err(|e| e.to_string())?;

    let status = persist(&state, "readwise", &works, &highlights_with_meta, None, &window)?;
    // Use the archive's newest updated_at as the incremental cursor.
    set_last_sync(&state, &max_updated);
    Ok(status)
}

#[tauri::command]
pub async fn run_import(
    state: tauri::State<'_, AppState>,
    window: tauri::WebviewWindow,
) -> Result<ImportStatus, String> {
    let cfg = state.config();
    let api_key = cfg.readwise_api_key;

    if api_key.is_empty() {
        return Err("No Readwise API key configured. Open Settings (⌘,).".to_string());
    }

    // Incremental when we have a cursor; full export otherwise.
    let last_sync = cfg.readwise_last_sync.clone();
    let updated_after = if last_sync.is_empty() { None } else { Some(last_sync.as_str()) };
    let sync_start = chrono::Utc::now().to_rfc3339();

    let _ = window.emit(
        "import:progress",
        if updated_after.is_some() {
            "Updating from Readwise (changes only)…"
        } else {
            "Importing from Readwise (full export)…"
        },
    );

    let client = ReadwiseClient::new(api_key);
    let (works, highlights_with_meta, raw_json) = client
        .import_export(updated_after)
        .await
        .map_err(|e| e.to_string())?;

    if works.is_empty() {
        let _ = window.emit(
            "import:complete",
            &ImportStatus { works_imported: 0, highlights_imported: 0, message: "Already up to date".into() },
        );
        set_last_sync(&state, &sync_start);
        return Ok(ImportStatus { works_imported: 0, highlights_imported: 0, message: "Already up to date".into() });
    }

    let status = persist(
        &state,
        "readwise",
        &works,
        &highlights_with_meta,
        Some(&raw_json),
        &window,
    )?;
    set_last_sync(&state, &sync_start);

    // Full article bodies (ADR-0007 MVP). Additive and resilient: a Reader
    // failure must not fail the highlight import that already succeeded.
    let _ = window.emit("import:progress", "Fetching full article text…");
    let archive_path = state.config().archive_path;
    match client.fetch_reader_fulltext().await {
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
            let _ = window.emit(
                "import:complete",
                &crate::models::ImportStatus {
                    works_imported: status.works_imported,
                    highlights_imported: status.highlights_imported,
                    message: format!("{} · {} full texts saved", status.message, written),
                },
            );
        }
        Err(e) => {
            let _ = window.emit(
                "import:complete",
                &crate::models::ImportStatus {
                    works_imported: status.works_imported,
                    highlights_imported: status.highlights_imported,
                    message: format!("{} · full text skipped ({})", status.message, e),
                },
            );
        }
    }

    Ok(status)
}

#[tauri::command]
pub async fn run_zotero_import(
    state: tauri::State<'_, AppState>,
    window: tauri::WebviewWindow,
) -> Result<ImportStatus, String> {
    let _ = window.emit("import:progress", "Reading Zotero database…");

    let cfg = state.config();
    let importer = ZoteroImporter::with_archive(cfg.zotero_db_path, cfg.archive_path);
    let (works, highlights_with_meta) = importer.import_all().map_err(|e| e.to_string())?;

    persist(&state, "zotero", &works, &highlights_with_meta, None, &window)
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
