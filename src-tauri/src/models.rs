use serde::{Deserialize, Serialize};

pub use scout_index::models::{
    Container as Work, Position as WorkPosition, Record as Highlight, RegexFilter, TagCount,
};

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
    /// OCR text extracted from image highlights; None when not yet processed.
    pub ocr_text: Option<String>,
    /// Semantic relevance (cosine, 0-1) for find-related results; None for keyword.
    pub relevance: Option<f64>,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportStatus {
    pub works_imported: usize,
    pub highlights_imported: usize,
    pub message: String,
}

/// Turn a generic index Hit into the HS SearchResult the frontend expects.
/// Reclaims the derivation dropped from scout-core's map_row: asset_path,
/// citation, authors, collections, zotero_link (from source_data JSON).
pub fn decorate(hit: scout_index::models::Hit, archive: &str) -> SearchResult {
    let asset_path = if hit.format == "image" {
        Some(format!(
            "{}/readings/assets/{}.png",
            archive.trim_end_matches('/'),
            hit.record_id
        ))
    } else {
        None
    };
    let work_sd = &hit.container_source_data;
    let hl_sd = &hit.record_source_data;
    let citation = work_sd
        .get("citation")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);
    let authors: Vec<String> = work_sd
        .get("authors")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let collections: Vec<String> = work_sd
        .get("collections")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let zotero_link = match (
        hl_sd.get("zotero_attachment_key").and_then(|v| v.as_str()),
        hl_sd.get("zotero_annotation_key").and_then(|v| v.as_str()),
    ) {
        (Some(ak), Some(annk)) if !ak.is_empty() => Some(format!(
            "zotero://open-pdf/library/items/{}?annotation={}",
            ak, annk
        )),
        (Some(ak), _) if !ak.is_empty() => Some(format!("zotero://open-pdf/library/items/{}", ak)),
        _ => None,
    };
    SearchResult {
        highlight_id: hit.record_id,
        work_id: hit.container_id,
        slug: hit.slug,
        text: hit.text,
        note: hit.note,
        title: hit.title,
        author: hit.author,
        authors,
        work_type: hit.kind,
        source_system: hit.source_system,
        source_id: hit.source_id,
        url: hit.url,
        highlighted_at: hit.created_at,
        tags: hit.tags,
        location: hit.location,
        annotation_color: hit.annotation_color,
        annotation_type: hit.annotation_type,
        format: hit.format,
        asset_path,
        citation,
        collections,
        zotero_link,
        ocr_text: hit.ocr_text,
        relevance: None,
        snippet: String::new(),
    }
}

/// Search payload sent by the existing frontend. The wire shape stays unchanged.
#[derive(Debug, Clone, Deserialize)]
pub struct SearchPayload {
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
    #[serde(default)]
    pub has_image: bool,
    /// Combinable work-type filter (OR across the list). Empty = no type filter.
    #[serde(default)]
    pub types: Vec<String>,
    pub after: Option<String>,
    pub before: Option<String>,
    pub source: Option<String>,
    /// Ticked highlight sources (any-of); empty = every source. The frontend
    /// has already folded the Zotero quick filter into it.
    #[serde(default)]
    pub sources: Vec<String>,
    pub color: Option<String>,
    pub sort: String,
    pub page: usize,
    pub page_size: usize,
}

pub fn to_core_query(p: SearchPayload) -> scout_index::models::SearchQuery {
    scout_index::models::SearchQuery {
        fts: p.fts,
        has_positive: p.has_positive,
        positive_terms: p.positive_terms,
        negatives: p.negatives,
        regexes: p.regexes,
        author: p.author,
        title: p.title,
        kind: p.work_type,
        tag: p.tag,
        tag_any: if p.favorite {
            vec!["favorite".into(), "Liked".into()]
        } else {
            vec![]
        },
        source_any: if !p.sources.is_empty() {
            p.sources
        } else if p.zotero {
            vec!["zotero".into()]
        } else {
            vec![]
        },
        has_image: p.has_image,
        kinds: p.types,
        after: p.after,
        before: p.before,
        source: p.source,
        color: p.color,
        sort: p.sort,
        page: p.page,
        page_size: p.page_size,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(extra: serde_json::Value) -> SearchPayload {
        let mut v = serde_json::json!({
            "fts": "", "has_positive": false, "author": null, "title": null, "type": null, "tag": null,
            "after": null, "before": null, "source": null, "color": null, "sort": "matches", "page": 0, "page_size": 80
        });
        v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn ticked_sources_become_any_of_and_old_payloads_still_parse() {
        // A payload without `sources` (older frontend) keeps the Zotero quick filter.
        let q = to_core_query(payload(serde_json::json!({ "zotero": true })));
        assert_eq!(q.source_any, vec!["zotero".to_string()]);
        // Ticked sources restrict to any of them.
        let q = to_core_query(payload(serde_json::json!({ "sources": ["x", "readwise"] })));
        assert_eq!(q.source_any, vec!["x".to_string(), "readwise".to_string()]);
        // Nothing ticked-off: no source restriction.
        assert!(to_core_query(payload(serde_json::json!({}))).source_any.is_empty());
    }
}
