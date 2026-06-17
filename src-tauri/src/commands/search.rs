use crate::index::sqlite;
use crate::models::{SearchPage, SearchQuery, SearchResult, TagCount, WorkPosition};
use crate::qmd;
use crate::AppState;

fn normalize(s: &str) -> String {
    s.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Semantic search via QMD (ADR-0005). Runs the full QMD query, then maps each
/// hit back to a highlight in our index by work slug + snippet quote text.
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
    let hits = qmd::query(&query, 60).await.map_err(|e| e.to_string())?;

    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut out: Vec<SearchResult> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for hit in &hits {
        let slug = qmd::slug_from_file(&hit.file);
        let Some(work_id) = sqlite::work_id_by_slug(&conn, &slug) else { continue };
        let rows = sqlite::work_highlights(&conn, &work_id, &archive).unwrap_or_default();
        if rows.is_empty() {
            continue;
        }
        // Pick the highlight whose text best matches the snippet quote.
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

        if seen.insert(chosen.highlight_id.clone()) {
            out.push(chosen);
        }
    }
    Ok(out)
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
