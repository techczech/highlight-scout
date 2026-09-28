use scout_index::sqlite;
use serde::Serialize;

use crate::models::{decorate, to_core_query, SearchPayload, SearchResult, TagCount, WorkPosition};
use crate::qmd;
use crate::AppState;

fn normalize(s: &str) -> String {
    s.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Map QMD hits back to highlights in our index (by work slug + snippet quote),
/// skipping `exclude` and de-duplicating. Shared by semantic search + related.
fn map_hits(
    conn: &rusqlite::Connection,
    archive: &str,
    hits: &[qmd::QmdHit],
    exclude: Option<&str>,
) -> Vec<SearchResult> {
    let mut out: Vec<SearchResult> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for hit in hits {
        let slug = qmd::slug_from_file(&hit.file);
        let Some(work_id) = sqlite::container_id_by_slug(conn, &slug) else {
            continue;
        };
        let rows: Vec<SearchResult> = sqlite::container_records(conn, &work_id)
            .unwrap_or_default()
            .into_iter()
            .map(|hit| decorate(hit, archive))
            .collect();
        if rows.is_empty() {
            continue;
        }
        let mut chosen = qmd::quote_from_snippet(&hit.snippet)
            .and_then(|q| {
                let nq = normalize(&q);
                rows.iter()
                    .find(|r| {
                        let nt = normalize(&r.text);
                        !nt.is_empty() && (nq.contains(&nt) || nt.contains(&nq))
                    })
                    .cloned()
            })
            .unwrap_or_else(|| rows[0].clone());

        if Some(chosen.highlight_id.as_str()) == exclude {
            continue;
        }
        chosen.relevance = Some(hit.score);
        if seen.insert(chosen.highlight_id.clone()) {
            out.push(chosen);
        }
    }
    out
}

/// Strip characters QMD's query grammar treats as operators (a leading `-` is
/// negation, `:` starts a typed line, `"` `*` `|` `(` `)` are operators). For an
/// embedding/BM25 query the bare words are all we need, so reduce to
/// alphanumeric + spaces (+ apostrophes) and cap the length.
fn sanitize_qmd(text: &str) -> String {
    let cleaned: String = text
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '\'' {
                c
            } else {
                ' '
            }
        })
        .collect();
    cleaned
        .split_whitespace()
        .take(60)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Build a typed QMD query document. Typed lines skip the slow LLM auto-expansion
/// (which is the ~8s cost) — this is the fast path (~0.5–1s).
fn typed_doc(text: &str, hybrid: bool) -> String {
    let q = sanitize_qmd(text);
    if hybrid {
        format!("lex: {}\nvec: {}", q, q)
    } else {
        format!("vec: {}", q)
    }
}

/// Semantic search via QMD (ADR-0005). Uses a typed lex+vec document (no LLM
/// expansion) for speed, then maps hits back to highlights.
#[tauri::command]
pub async fn semantic_search(
    query: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<SearchResult>, String> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }
    let archive = state.config().archive_path;
    qmd::ensure_collection(&archive)
        .await
        .map_err(|e| e.to_string())?;
    let hits = qmd::query(&typed_doc(&query, true), 60)
        .await
        .map_err(|e| e.to_string())?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    Ok(map_hits(&conn, &archive, &hits, None))
}

/// "Find related": pure-vector QMD search seeded by one highlight's text,
/// excluding the source highlight.
#[tauri::command]
pub async fn find_related(
    text: String,
    exclude_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<SearchResult>, String> {
    if text.trim().is_empty() {
        return Ok(vec![]);
    }
    let archive = state.config().archive_path;
    qmd::ensure_collection(&archive)
        .await
        .map_err(|e| e.to_string())?;
    let hits = qmd::query(&typed_doc(&text, false), 40)
        .await
        .map_err(|e| e.to_string())?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    Ok(map_hits(&conn, &archive, &hits, Some(&exclude_id)))
}

#[tauri::command]
pub async fn qmd_available() -> Result<bool, String> {
    Ok(qmd::available().await)
}

/// Rebuild the QMD semantic index (update + embed), streaming progress.
#[tauri::command]
pub async fn qmd_reindex(
    state: tauri::State<'_, AppState>,
    window: tauri::WebviewWindow,
) -> Result<String, String> {
    let archive = state.config().archive_path;
    qmd::reindex(&archive, &window)
        .await
        .map_err(|e| e.to_string())?;
    use tauri::Emitter;
    let _ = window.emit(
        "import:complete",
        serde_json::json!({ "message": "Semantic index rebuilt" }),
    );
    Ok("Semantic index rebuilt".to_string())
}

/// Manually OCR all pending image highlights (macOS only). Returns the count written.
#[tauri::command]
pub async fn ocr_images(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
) -> Result<usize, String> {
    use std::sync::atomic::Ordering;
    use tauri::Manager;
    let state = app.state::<crate::AppState>();
    if !crate::ocr::available() {
        return Err("OCR is only available on macOS".into());
    }
    if state.is_ocring.swap(true, Ordering::SeqCst) {
        return Err("OCR already running".into());
    }
    // OcrGuard ensures is_ocring is cleared even if run_ocr_app panics.
    let _guard = crate::ocr::OcrGuard::acquire(&state.is_ocring);
    let archive = state.config().archive_path.clone();
    let n = crate::ocr::run_ocr_app(&app, &window, &archive, None).await;
    Ok(n)
}

#[tauri::command]
pub async fn search_query(
    query: SearchPayload,
    state: tauri::State<'_, AppState>,
) -> Result<ResultPage, String> {
    let archive = state.config().archive_path;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let page = sqlite::search_query(&conn, &to_core_query(query)).map_err(|e| e.to_string())?;
    Ok(ResultPage {
        rows: page
            .rows
            .into_iter()
            .map(|hit| decorate(hit, &archive))
            .collect(),
        has_more: page.has_more,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct ResultPage {
    pub rows: Vec<SearchResult>,
    pub has_more: bool,
}

#[tauri::command]
pub async fn work_highlights(
    work_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<SearchResult>, String> {
    let archive = state.config().archive_path;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    sqlite::container_records(&conn, &work_id)
        .map(|rows| {
            rows.into_iter()
                .map(|hit| decorate(hit, &archive))
                .collect()
        })
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn highlight_position(
    work_id: String,
    location: String,
    state: tauri::State<'_, AppState>,
) -> Result<Option<WorkPosition>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    sqlite::record_position(&conn, &work_id, &location).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_highlight(
    id: String,
    state: tauri::State<'_, AppState>,
) -> Result<Option<SearchResult>, String> {
    let archive = state.config().archive_path;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    Ok(sqlite::record_by_id(&conn, &id).map(|hit| decorate(hit, &archive)))
}

#[tauri::command]
pub async fn list_tags(state: tauri::State<'_, AppState>) -> Result<Vec<TagCount>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    sqlite::list_tags(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_facets(state: tauri::State<'_, AppState>) -> Result<serde_json::Value, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let (sources, colors) = sqlite::facets(&conn).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "sources": sources, "colors": colors }))
}

#[tauri::command]
pub async fn get_stats(state: tauri::State<'_, AppState>) -> Result<serde_json::Value, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let highlights = sqlite::record_count(&conn);
    let works = sqlite::container_count(&conn);
    Ok(serde_json::json!({ "highlights": highlights, "works": works }))
}

/// Highlights matching one query in the highlight index: in all, and per
/// source system (`x`, `readwise`, `zotero`). The rail's result counts.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct HighlightCounts {
    pub total: usize,
    pub sources: std::collections::BTreeMap<String, usize>,
}

/// Far past any archive: one page holds every match.
const COUNT_PAGE: usize = 10_000_000;

/// Count what paging through `q` would show, by running the same search as
/// one page (so negatives and the regex cap apply exactly as they do to the
/// list). Keyword order is irrelevant to a count, so recency (the cheap
/// order) is used; a regex query keeps its order, since its scan is capped.
pub fn count_highlights(
    conn: &rusqlite::Connection,
    q: &scout_index::models::SearchQuery,
) -> Result<HighlightCounts, String> {
    let mut q = q.clone();
    q.page = 0;
    q.page_size = COUNT_PAGE;
    if q.regexes.is_empty() {
        q.sort = "recent".into();
    }
    let page = sqlite::search_query(conn, &q).map_err(|e| e.to_string())?;
    let mut out = HighlightCounts::default();
    for hit in page.rows {
        out.total += 1;
        *out.sources.entry(hit.source_system).or_default() += 1;
    }
    Ok(out)
}

/// The rail's result counts for a highlight index query (page and sort ignored).
#[tauri::command]
pub async fn search_counts(
    query: SearchPayload,
    state: tauri::State<'_, AppState>,
) -> Result<HighlightCounts, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    count_highlights(&conn, &to_core_query(query))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        sqlite::init_schema(&conn).unwrap();
        for (id, src) in [("wx", "x"), ("wr", "readwise"), ("wz", "zotero")] {
            conn.execute(
                "INSERT INTO works (id,slug,title,author,work_type,source_system,source_id,url,imported_at,updated_at,source_data) VALUES (?1,?1,'W',NULL,'article',?2,NULL,NULL,'t','t','{}')",
                rusqlite::params![id, src],
            )
            .unwrap();
        }
        for (id, work, text) in [
            ("h1", "wx", "testing one"),
            ("h2", "wx", "testing two"),
            ("h3", "wr", "testing three"),
            ("h4", "wr", "nothing here"),
            ("h5", "wz", "testing four, flaky"),
        ] {
            conn.execute(
                "INSERT INTO highlights (id,work_id,text,tags,format,source_data) VALUES (?1,?2,?3,'[]','plain','{}')",
                rusqlite::params![id, work, text],
            )
            .unwrap();
            sqlite::reindex_record_fts(&conn, id).unwrap();
        }
        conn
    }

    fn query(extra: serde_json::Value) -> scout_index::models::SearchQuery {
        let mut v = serde_json::json!({
            "fts": "testing", "has_positive": true, "positive_terms": ["testing"],
            "author": null, "title": null, "type": null, "tag": null,
            "after": null, "before": null, "source": null, "color": null,
            "sort": "matches", "page": 3, "page_size": 1
        });
        v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        to_core_query(serde_json::from_value(v).unwrap())
    }

    #[test]
    fn counts_every_match_per_source_whatever_the_page() {
        let c = count_highlights(&db(), &query(serde_json::json!({}))).unwrap();
        assert_eq!(c.total, 4);
        assert_eq!(c.sources.get("x"), Some(&2));
        assert_eq!(c.sources.get("readwise"), Some(&1));
        assert_eq!(c.sources.get("zotero"), Some(&1));
    }

    #[test]
    fn counts_honour_negatives_and_ticked_sources() {
        let conn = db();
        let c = count_highlights(&conn, &query(serde_json::json!({ "negatives": ["flaky"] }))).unwrap();
        assert_eq!(c.total, 3);
        assert_eq!(c.sources.get("zotero"), None);
        let c = count_highlights(&conn, &query(serde_json::json!({ "sources": ["readwise"] }))).unwrap();
        assert_eq!(c.total, 1);
    }
}
