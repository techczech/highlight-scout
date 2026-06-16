use std::collections::HashMap;

use tauri::Emitter;

use crate::import::archive;
use crate::import::readwise::ReadwiseClient;
use crate::index::sqlite;
use crate::models::ImportStatus;
use crate::AppState;

#[tauri::command]
pub async fn run_import(
    state: tauri::State<'_, AppState>,
    window: tauri::WebviewWindow,
) -> Result<ImportStatus, String> {
    let api_key = state.config.readwise_api_key.clone();
    let archive_path = state.config.archive_path.clone();

    if api_key.is_empty() {
        return Err(format!(
            "No Readwise API key configured. Edit ~/.config/highlight-scout/config.toml"
        ));
    }

    // Emit progress to frontend
    let _ = window.emit("import:progress", "Fetching from Readwise...");

    let client = ReadwiseClient::new(api_key);
    let (works, highlights_with_meta) = client
        .import_all()
        .await
        .map_err(|e| e.to_string())?;

    let _ = window.emit(
        "import:progress",
        format!("Fetched {} works, {} highlights. Writing archive...", works.len(), highlights_with_meta.len()),
    );

    // Group highlights by work for archive writing
    let mut highlights_by_work: HashMap<String, Vec<&crate::models::Highlight>> = HashMap::new();
    let highlights: Vec<crate::models::Highlight> = highlights_with_meta
        .iter()
        .map(|(h, _, _)| h.clone())
        .collect();
    for h in &highlights {
        highlights_by_work.entry(h.work_id.clone()).or_default().push(h);
    }

    // Write Archive
    archive::write_archive(&archive_path, &works, &highlights_by_work)
        .map_err(|e| format!("Archive write failed: {}", e))?;

    let _ = window.emit("import:progress", "Updating search index...");

    // Update SQLite index
    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        for work in &works {
            sqlite::upsert_work(&conn, work).map_err(|e| e.to_string())?;
        }
        for (h, title, author) in &highlights_with_meta {
            sqlite::upsert_highlight(&conn, h, title, author.as_deref())
                .map_err(|e| e.to_string())?;
        }
    }

    let status = ImportStatus {
        works_imported: works.len(),
        highlights_imported: highlights.len(),
        message: format!(
            "Import complete: {} works, {} highlights",
            works.len(),
            highlights.len()
        ),
    };

    let _ = window.emit("import:complete", &status);

    Ok(status)
}

#[tauri::command]
pub async fn get_config(state: tauri::State<'_, AppState>) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "archive_path": state.config.archive_path,
        "has_api_key": !state.config.readwise_api_key.is_empty(),
        "shortcut": state.config.shortcut,
    }))
}
