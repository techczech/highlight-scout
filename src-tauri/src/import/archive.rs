use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::models::{Highlight, Work};

/// Write v2 Archive Markdown files for a batch of works and their highlights.
pub fn write_archive(
    archive_path: &str,
    works: &[Work],
    highlights_by_work: &HashMap<String, Vec<&Highlight>>,
) -> Result<()> {
    let base = Path::new(archive_path);
    fs::create_dir_all(base.join("readings").join("works"))?;
    fs::create_dir_all(base.join("readings").join("fulltext"))?;
    fs::create_dir_all(base.join("readings").join("assets"))?;

    for work in works {
        let type_dir = base
            .join("readings")
            .join("works")
            .join(&work.work_type);
        fs::create_dir_all(&type_dir)?;

        let file_path = type_dir.join(format!("{}.md", work.slug));
        let empty = vec![];
        let work_highlights = highlights_by_work.get(&work.id).unwrap_or(&empty);

        let content = render_work_file(work, work_highlights);
        fs::write(&file_path, content)?;
    }

    Ok(())
}

fn render_work_file(work: &Work, highlights: &[&Highlight]) -> String {
    let mut out = String::new();

    // Frontmatter
    out.push_str("---\n");
    out.push_str(&format!("title: {}\n", escape_yaml(&work.title)));
    if let Some(author) = &work.author {
        out.push_str(&format!("author: {}\n", escape_yaml(author)));
    }
    out.push_str(&format!("type: {}\n", work.work_type));
    out.push_str(&format!("source_system: {}\n", work.source_system));
    if let Some(sid) = &work.source_id {
        out.push_str(&format!("source_id: \"{}\"\n", sid));
    }
    if let Some(url) = &work.url {
        out.push_str(&format!("url: {}\n", url));
    }
    out.push_str(&format!("imported_at: {}\n", work.imported_at));
    out.push_str(&format!("updated_at: {}\n", work.updated_at));
    out.push_str(&format!(
        "source_data: {}\n",
        serde_json::to_string(&work.source_data).unwrap_or_else(|_| "{}".to_string())
    ));
    out.push_str("---\n\n");

    // Highlights
    for hl in highlights {
        out.push_str(&render_highlight(hl));
        out.push_str("\n---\n\n");
    }

    out
}

fn render_highlight(h: &Highlight) -> String {
    let mut out = String::new();

    match h.format.as_str() {
        "image" => {
            out.push_str(&format!("![](../assets/{}.png)\n\n", h.id));
        }
        "latex" => {
            out.push_str("```latex\n");
            out.push_str(&h.text);
            out.push_str("\n```\n\n");
        }
        _ => {
            // Plain text: blockquote, wrapping long lines
            for line in h.text.lines() {
                out.push_str(&format!("> {}\n", line));
            }
            out.push('\n');
        }
    }

    // Metadata line
    let mut meta_parts = Vec::new();
    if let Some(date) = &h.highlighted_at {
        // Trim to date portion if ISO datetime
        let date_short = date.split('T').next().unwrap_or(date);
        meta_parts.push(format!("highlighted_at: {}", date_short));
    }
    if !h.tags.is_empty() {
        meta_parts.push(format!("tags: {}", h.tags.join(", ")));
    }
    if let Some(color) = &h.annotation_color {
        meta_parts.push(format!("color: {}", color));
    }
    if let Some(atype) = &h.annotation_type {
        meta_parts.push(format!("type: {}", atype));
    }
    if h.format != "plain" {
        meta_parts.push(format!("format: {}", h.format));
    }
    if !meta_parts.is_empty() {
        out.push_str(&meta_parts.join(" | "));
        out.push('\n');
    }

    // Note
    if let Some(note) = &h.note {
        if !note.is_empty() {
            out.push('\n');
            out.push_str(note);
            out.push('\n');
        }
    }

    out
}

fn escape_yaml(s: &str) -> String {
    if s.contains(':') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\\\""))
    } else {
        s.to_string()
    }
}
