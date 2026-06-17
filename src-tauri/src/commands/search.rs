use crate::index::sqlite;
use crate::models::{SearchPage, SearchQuery, SearchResult, TagCount, WorkPosition};
use crate::qmd;
use crate::AppState;

fn normalize(s: &str) -> String {
    s.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ")
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
        let Some(work_id) = sqlite::work_id_by_slug(conn, &slug) else { continue };
        let rows = sqlite::work_highlights(conn, &work_id, archive).unwrap_or_default();
        if rows.is_empty() {
            continue;
        }
        let chosen = qmd::quote_from_snippet(&hit.snippet)
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
        .map(|c| if c.is_alphanumeric() || c == '\'' { c } else { ' ' })
        .collect();
    cleaned.split_whitespace().take(60).collect::<Vec<_>>().join(" ")
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
    qmd::ensure_collection(&archive).await.map_err(|e| e.to_string())?;
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
    qmd::ensure_collection(&archive).await.map_err(|e| e.to_string())?;
    let hits = qmd::query(&typed_doc(&text, false), 40)
        .await
        .map_err(|e| e.to_string())?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    Ok(map_hits(&conn, &archive, &hits, Some(&exclude_id)))
}

/// Rebuild the QMD semantic index (update + embed), streaming progress.
#[tauri::command]
pub async fn qmd_reindex(
    state: tauri::State<'_, AppState>,
    window: tauri::WebviewWindow,
) -> Result<String, String> {
    let archive = state.config().archive_path;
    qmd::reindex(&archive, &window).await.map_err(|e| e.to_string())?;
    use tauri::Emitter;
    let _ = window.emit(
        "import:complete",
        serde_json::json!({ "message": "Semantic index rebuilt" }),
    );
    Ok("Semantic index rebuilt".to_string())
}

#[tauri::command]
pub async fn search_query(
    query: SearchQuery,
    state: tauri::State<'_, AppState>,
) -> Result<SearchPage, String> {
    let archive = state.config().archive_path;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    sqlite::search_query(&conn, &query, &archive).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn work_highlights(
    work_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<SearchResult>, String> {
    let archive = state.config().archive_path;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    sqlite::work_highlights(&conn, &work_id, &archive).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn highlight_position(
    work_id: String,
    location: String,
    state: tauri::State<'_, AppState>,
) -> Result<Option<WorkPosition>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    sqlite::highlight_position(&conn, &work_id, &location).map_err(|e| e.to_string())
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
    let highlights = sqlite::highlight_count(&conn);
    let works = sqlite::work_count(&conn);
    Ok(serde_json::json!({ "highlights": highlights, "works": works }))
}
