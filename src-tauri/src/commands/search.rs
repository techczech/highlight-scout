use crate::index::sqlite;
use crate::models::SearchResult;
use crate::AppState;

#[tauri::command]
pub async fn search_highlights(
    query: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<SearchResult>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    sqlite::search(&conn, &query, 100).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_stats(state: tauri::State<'_, AppState>) -> Result<serde_json::Value, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let highlights = sqlite::highlight_count(&conn);
    let works = sqlite::work_count(&conn);
    Ok(serde_json::json!({ "highlights": highlights, "works": works }))
}
