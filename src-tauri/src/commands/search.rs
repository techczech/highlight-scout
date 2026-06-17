use crate::index::sqlite;
use crate::models::{SearchPage, SearchQuery, SearchResult, TagCount, WorkPosition};
use crate::AppState;

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
