use crate::index::sqlite;
use crate::models::SearchResult;
use crate::AppState;

#[tauri::command]
pub async fn search_highlights(
    query: String,
    source: Option<String>,
    color: Option<String>,
    mode: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<SearchResult>, String> {
    // ADR-0005: FTS is the default mode. Semantic (QMD) is wired in the UI from
    // day one but ships as a fast-follow — reject the call clearly until then.
    if mode.as_deref() == Some("semantic") {
        return Err("Semantic search (QMD) is not available yet — use keyword mode.".to_string());
    }

    let conn = state.db.lock().map_err(|e| e.to_string())?;
    sqlite::search(
        &conn,
        &query,
        source.as_deref().filter(|s| !s.is_empty()),
        color.as_deref().filter(|c| !c.is_empty()),
        100,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_facets(
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
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
