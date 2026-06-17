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
    pub slug: String,
    pub text: String,
    pub note: Option<String>,
    pub title: String,
    pub author: Option<String>,
    pub authors: Vec<String>,
    pub work_type: String,
    pub source_system: String,
    pub source_id: Option<String>,
    pub url: Option<String>,
    pub highlighted_at: Option<String>,
    pub tags: Vec<String>,
    pub location: Option<String>,
    pub annotation_color: Option<String>,
    pub annotation_type: Option<String>,
    pub format: String,
    pub asset_path: Option<String>,
    pub citation: Option<String>,
    pub collections: Vec<String>,
    pub zotero_link: Option<String>,
    /// QMD relevance score (0–1) for semantic / find-related results; None for keyword.
    pub relevance: Option<f64>,
    pub snippet: String,
}

/// A page of search results plus whether more pages may exist.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchPage {
    pub rows: Vec<SearchResult>,
    pub has_more: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RegexFilter {
    pub source: String,
    pub flags: String,
}

/// Structured query assembled by the frontend parser (mirrors the Raycast
/// extension's ParsedQuery). The frontend folds date/year ranges into
/// after/before and scope presets into these fields before sending.
#[derive(Debug, Clone, Deserialize)]
pub struct SearchQuery {
    pub fts: String,
    pub has_positive: bool,
    #[serde(default)]
    pub positive_terms: Vec<String>,
    #[serde(default)]
    pub negatives: Vec<String>,
    #[serde(default)]
    pub regexes: Vec<RegexFilter>,
    pub author: Option<String>,
    pub title: Option<String>,
    #[serde(rename = "type")]
    pub work_type: Option<String>,
    pub tag: Option<String>,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub zotero: bool,
    pub after: Option<String>,
    pub before: Option<String>,
    pub source: Option<String>,
    pub color: Option<String>,
    pub sort: String,
    pub page: usize,
    pub page_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportStatus {
    pub works_imported: usize,
    pub highlights_imported: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagCount {
    pub tag: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkPosition {
    pub pos: i64,
    pub total: i64,
    pub max_loc: i64,
}
