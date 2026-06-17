// Kindle `My Clippings.txt` parser (ADR-0009). Blocks separated by a line of
// '='. Each block: "Title (Author)", a metadata line (Highlight/Note/Bookmark +
// page/location + date), a blank line, then the text. Bookmarks are skipped;
// standalone notes become note-type highlights. Idempotent via content-hash IDs.

use anyhow::{bail, Result};
use chrono::Utc;

use crate::import::archive::make_slug;
use crate::import::common::{highlight_id, work_id};
use crate::models::{Highlight, Work};

/// Split "Title (Author Name)" → (title, Option<author>).
fn split_title_author(line: &str) -> (String, Option<String>) {
    let line = line.trim().trim_start_matches('\u{feff}');
    if let Some(open) = line.rfind('(') {
        if line.ends_with(')') {
            let title = line[..open].trim().to_string();
            let author = line[open + 1..line.len() - 1].trim().to_string();
            return (
                if title.is_empty() { "Untitled".into() } else { title },
                if author.is_empty() { None } else { Some(author) },
            );
        }
    }
    (line.to_string(), None)
}

pub fn import(path: &str) -> Result<(Vec<Work>, Vec<(Highlight, String, Option<String>)>)> {
    let content = std::fs::read_to_string(path)?;
    let now = Utc::now().to_rfc3339();

    let mut works = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut highlights = Vec::new();

    for block in content.split("==========") {
        let lines: Vec<&str> = block.lines().collect();
        // Find the title line (first non-empty).
        let start = lines.iter().position(|l| !l.trim().is_empty());
        let Some(start) = start else { continue };
        let title_line = lines[start];
        let meta_line = lines.get(start + 1).copied().unwrap_or("");
        let meta_lower = meta_line.to_lowercase();

        if meta_lower.contains("bookmark") {
            continue; // bookmarks carry no text
        }
        let is_note = meta_lower.contains("note");

        // Text is everything after the metadata line, skipping the blank.
        let text = lines
            .iter()
            .skip(start + 2)
            .map(|l| l.trim())
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string();
        if text.is_empty() {
            continue;
        }

        let (title, author) = split_title_author(title_line);
        let author_s = author.clone().unwrap_or_default();

        // Pull a "Location 123-125" or "page 12" out of the metadata line.
        let location = parse_location(meta_line);

        let wid = work_id("kindle", &title, &author_s);
        if seen.insert(wid.clone()) {
            works.push(Work {
                id: wid.clone(),
                slug: make_slug(author.as_deref(), &title, &wid),
                title: title.clone(),
                author: author.clone(),
                work_type: "book".to_string(),
                source_system: "kindle".to_string(),
                source_id: None,
                url: None,
                imported_at: now.clone(),
                updated_at: now.clone(),
                source_data: serde_json::json!({ "kindle_file": path }),
            });
        }

        highlights.push((
            Highlight {
                id: highlight_id("kindle", &title, &author_s, &text, &location),
                work_id: wid,
                text,
                note: None,
                highlighted_at: None,
                updated_at: Some(now.clone()),
                tags: vec![],
                location: if location.is_empty() { None } else { Some(location) },
                location_type: Some("location".to_string()),
                annotation_color: None,
                annotation_type: if is_note { Some("note".to_string()) } else { None },
                format: "plain".to_string(),
                source_data: serde_json::json!({ "kindle_meta": meta_line.trim() }),
            },
            title,
            author,
        ));
    }

    if highlights.is_empty() {
        bail!("No Kindle highlights found (is this a My Clippings.txt file?)");
    }
    Ok((works, highlights))
}

fn parse_location(meta: &str) -> String {
    let lower = meta.to_lowercase();
    for key in ["location ", "page "] {
        if let Some(pos) = lower.find(key) {
            let rest = &meta[pos + key.len()..];
            let val: String = rest
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '-')
                .collect();
            if !val.is_empty() {
                return val;
            }
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_clipping() {
        let sample = "The Book Title (Smith, Jane)\r\n- Your Highlight on page 5 | Location 100-102 | Added on Monday\r\n\r\nThis is the highlighted text.\r\n==========\r\n";
        let dir = std::env::temp_dir().join("hs-kindle-test.txt");
        std::fs::write(&dir, sample).unwrap();
        let (works, hls) = import(dir.to_str().unwrap()).unwrap();
        assert_eq!(works.len(), 1);
        assert_eq!(works[0].title, "The Book Title");
        assert_eq!(works[0].author.as_deref(), Some("Smith, Jane"));
        assert_eq!(hls.len(), 1);
        assert_eq!(hls[0].0.text, "This is the highlighted text.");
        assert_eq!(hls[0].0.location.as_deref(), Some("100-102"));
        let _ = std::fs::remove_file(&dir);
    }
}
