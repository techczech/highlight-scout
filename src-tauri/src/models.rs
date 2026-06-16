use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Work {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub author: Option<String>,
    pub work_type: String,
    pub source_system: String,
    pub source_id: Option<String>,
    pub url: Option<String>,
    pub imported_at: String,
    pub updated_at: String,
    pub source_data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Highlight {
    pub id: String,
    pub work_id: String,
    pub text: String,
    pub note: Option<String>,
    pub highlighted_at: Option<String>,
    pub updated_at: Option<String>,
    pub tags: Vec<String>,
    pub location: Option<String>,
    pub location_type: Option<String>,
    pub annotation_color: Option<String>,
    pub annotation_type: Option<String>,
    pub format: String,
    pub source_data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub highlight_id: String,
    pub work_id: String,
    pub text: String,
    pub note: Option<String>,
    pub title: String,
    pub author: Option<String>,
    pub work_type: String,
    pub source_system: String,
    pub url: Option<String>,
    pub highlighted_at: Option<String>,
    pub tags: Vec<String>,
    pub annotation_color: Option<String>,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportStatus {
    pub works_imported: usize,
    pub highlights_imported: usize,
    pub message: String,
}
