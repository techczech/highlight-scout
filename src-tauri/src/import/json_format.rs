// Highlight Scout's canonical JSON import/export format (ADR-0008). Versioned,
// round-trips losslessly. The integration point for third-party exporters.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::models::{Highlight, Work};

pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonExport {
    pub format_version: u32,
    #[serde(default)]
    pub exported_at: String,
    pub works: Vec<Work>,
    pub highlights: Vec<Highlight>,
}

pub fn build_export(works: Vec<Work>, highlights: Vec<Highlight>, now: &str) -> JsonExport {
    JsonExport {
        format_version: FORMAT_VERSION,
        exported_at: now.to_string(),
        works,
        highlights,
    }
}

/// Parse a canonical JSON file into (works, highlights+meta) for persisting.
pub fn import(path: &str) -> Result<(Vec<Work>, Vec<(Highlight, String, Option<String>)>)> {
    let content = std::fs::read_to_string(path)?;
    let parsed: JsonExport = serde_json::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Not a valid Highlight Scout JSON file: {}", e))?;
    if parsed.format_version > FORMAT_VERSION {
        bail!(
            "This file is format_version {}, but this app only understands up to {}",
            parsed.format_version,
            FORMAT_VERSION
        );
    }

    let work_meta: HashMap<String, (String, Option<String>)> = parsed
        .works
        .iter()
        .map(|w| (w.id.clone(), (w.title.clone(), w.author.clone())))
        .collect();

    let highlights = parsed
        .highlights
        .into_iter()
        .map(|h| {
            let (title, author) = work_meta
                .get(&h.work_id)
                .cloned()
                .unwrap_or_else(|| ("Untitled".to_string(), None));
            (h, title, author)
        })
        .collect();

    Ok((parsed.works, highlights))
}
