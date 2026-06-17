use anyhow::Result;
use slug::slugify;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::models::{Highlight, Work};

/// Build a stable, unique, filesystem-safe slug for a Work.
/// Format: {author}-{title}-{source_id}, truncated to a safe length.
/// The source_id suffix guarantees uniqueness even when author+title collide
/// across sources or types (ADR-0003 uses a flat works/ directory).
pub fn make_slug(author: Option<&str>, title: &str, source_id: &str) -> String {
    let author_part = author.unwrap_or("unknown");
    let base = slugify(format!("{}-{}", author_part, title));
    // Truncate the descriptive part so the final filename stays well under
    // the 255-byte filesystem limit, then append the (numeric/short) id.
    let truncated: String = base.chars().take(120).collect();
    let truncated = truncated.trim_end_matches('-');
    let id_part = slugify(source_id);
    if id_part.is_empty() {
        truncated.to_string()
    } else {
        format!("{}-{}", truncated, id_part)
    }
}

/// Write the full body text of a Work to readings/fulltext/{slug}.md (ADR-0003).
pub fn write_fulltext(archive_path: &str, slug: &str, text: &str) -> Result<()> {
    let dir = Path::new(archive_path).join("readings").join("fulltext");
    fs::create_dir_all(&dir)?;
    fs::write(dir.join(format!("{}.md", slug)), text)?;
    Ok(())
}

/// Write a raw source export snapshot to imports/{source}-{stamp}.json (ADR-0001,
/// "Import batch" in CONTEXT.md). Preserves provenance; the Archive is derived from these.
pub fn write_import_batch(
    archive_path: &str,
    source: &str,
    stamp: &str,
    raw_json: &str,
) -> Result<()> {
    let dir = Path::new(archive_path).join("imports");
    fs::create_dir_all(&dir)?;
    fs::write(dir.join(format!("{}-{}.json", source, stamp)), raw_json)?;
    Ok(())
}

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

    let works_dir = base.join("readings").join("works");

    for work in works {
        // ADR-0003: flat directory — readings/works/{slug}.md
        let file_path = works_dir.join(format!("{}.md", work.slug));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_includes_source_id_for_uniqueness() {
        let a = make_slug(Some("Jane Smith"), "How LLMs Work", "123");
        assert_eq!(a, "jane-smith-how-llms-work-123");
    }

    #[test]
    fn slug_disambiguates_same_title_by_id() {
        let a = make_slug(Some("Smith"), "Same Title", "1");
        let b = make_slug(Some("Smith"), "Same Title", "2");
        assert_ne!(a, b);
    }

    #[test]
    fn slug_truncates_long_titles() {
        let long = "word ".repeat(100);
        let s = make_slug(Some("Author"), &long, "42");
        // descriptive part capped at 120 chars + "-42"
        assert!(s.len() <= 130, "slug too long: {}", s.len());
        assert!(s.ends_with("-42"));
    }

    #[test]
    fn slug_handles_missing_author() {
        let s = make_slug(None, "Title", "9");
        assert_eq!(s, "unknown-title-9");
    }

    fn sample_highlight(text: &str, format: &str) -> Highlight {
        Highlight {
            id: "h1".into(),
            work_id: "w1".into(),
            text: text.into(),
            note: None,
            highlighted_at: Some("2024-01-15T10:00:00Z".into()),
            updated_at: None,
            tags: vec!["methods".into()],
            location: None,
            location_type: None,
            annotation_color: Some("green".into()),
            annotation_type: Some("highlight".into()),
            format: format.into(),
            source_data: serde_json::json!({}),
        }
    }

    #[test]
    fn renders_plain_highlight_as_blockquote() {
        let h = sample_highlight("Hello world", "plain");
        let out = render_highlight(&h);
        assert!(out.contains("> Hello world"));
        assert!(out.contains("highlighted_at: 2024-01-15"));
        assert!(out.contains("tags: methods"));
        assert!(out.contains("color: green"));
    }

    #[test]
    fn renders_latex_as_fenced_block() {
        let h = sample_highlight("\\frac{1}{2}", "latex");
        let out = render_highlight(&h);
        assert!(out.contains("```latex"));
        assert!(out.contains("\\frac{1}{2}"));
        assert!(out.contains("format: latex"));
    }

    #[test]
    fn renders_image_as_asset_reference() {
        let h = sample_highlight("", "image");
        let out = render_highlight(&h);
        assert!(out.contains("![](../assets/h1.png)"));
        assert!(out.contains("format: image"));
    }
}
