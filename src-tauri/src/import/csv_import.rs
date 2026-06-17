// CSV importer with column mapping (ADR-0009). `inspect` reads the header +
// sample rows for the mapping UI; `import` converts rows to works/highlights
// using a saved mapping. Text is the only required field; unmapped columns are
// preserved in source_data; works group by title+author; IDs are content-hashed
// so re-import is idempotent.

use anyhow::{bail, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::import::archive::make_slug;
use crate::import::common::{highlight_id, work_id};
use crate::models::{Highlight, Work};

#[derive(Debug, Serialize)]
pub struct CsvInspect {
    pub headers: Vec<String>,
    pub sample_rows: Vec<Vec<String>>,
    pub delimiter: String,
}

/// Column header names mapped to canonical fields (None = not mapped).
#[derive(Debug, Clone, Deserialize)]
pub struct CsvMapping {
    pub text: Option<String>,
    pub title: Option<String>,
    pub author: Option<String>,
    pub note: Option<String>,
    pub date: Option<String>,
    pub location: Option<String>,
    pub tags: Option<String>,
    pub url: Option<String>,
    pub color: Option<String>,
    #[serde(default = "default_delim")]
    pub delimiter: String,
}

fn default_delim() -> String {
    ",".to_string()
}

fn delim_byte(s: &str) -> u8 {
    match s {
        "\\t" | "\t" | "tab" => b'\t',
        ";" => b';',
        "|" => b'|',
        _ => b',',
    }
}

/// Sniff the delimiter from the first line: whichever of , ; \t | occurs most.
fn sniff_delimiter(first_line: &str) -> &'static str {
    let counts = [
        (",", first_line.matches(',').count()),
        (";", first_line.matches(';').count()),
        ("\t", first_line.matches('\t').count()),
        ("|", first_line.matches('|').count()),
    ];
    counts
        .iter()
        .max_by_key(|(_, n)| *n)
        .filter(|(_, n)| *n > 0)
        .map(|(d, _)| *d)
        .unwrap_or(",")
}

pub fn inspect(path: &str) -> Result<CsvInspect> {
    let content = std::fs::read_to_string(path)?;
    let first_line = content.lines().next().unwrap_or("");
    let delimiter = sniff_delimiter(first_line);

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim_byte(delimiter))
        .flexible(true)
        .from_reader(content.as_bytes());

    let headers: Vec<String> = rdr
        .headers()?
        .iter()
        .map(|s| s.to_string())
        .collect();

    let sample_rows: Vec<Vec<String>> = rdr
        .records()
        .take(5)
        .filter_map(|r| r.ok())
        .map(|rec| rec.iter().map(|s| s.to_string()).collect())
        .collect();

    Ok(CsvInspect {
        headers,
        sample_rows,
        delimiter: delimiter.to_string(),
    })
}

pub fn import(
    path: &str,
    mapping: &CsvMapping,
) -> Result<(Vec<Work>, Vec<(Highlight, String, Option<String>)>)> {
    let Some(text_col) = mapping.text.clone() else {
        bail!("CSV import needs a column mapped to the highlight text");
    };
    let now = Utc::now().to_rfc3339();
    let content = std::fs::read_to_string(path)?;

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim_byte(&mapping.delimiter))
        .flexible(true)
        .from_reader(content.as_bytes());

    let headers: Vec<String> = rdr.headers()?.iter().map(|s| s.to_string()).collect();
    let idx = |name: &Option<String>| -> Option<usize> {
        name.as_ref().and_then(|n| headers.iter().position(|h| h == n))
    };
    let text_i = headers.iter().position(|h| h == &text_col);
    let (title_i, author_i, note_i, date_i, loc_i, tags_i, url_i, color_i) = (
        idx(&mapping.title),
        idx(&mapping.author),
        idx(&mapping.note),
        idx(&mapping.date),
        idx(&mapping.location),
        idx(&mapping.tags),
        idx(&mapping.url),
        idx(&mapping.color),
    );

    let mut works = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut highlights = Vec::new();

    for rec in rdr.records().filter_map(|r| r.ok()) {
        let get = |i: Option<usize>| -> String {
            i.and_then(|i| rec.get(i)).unwrap_or("").trim().to_string()
        };
        let text = get(text_i);
        if text.is_empty() {
            continue; // text is required
        }
        let title = {
            let t = get(title_i);
            if t.is_empty() { "Untitled".to_string() } else { t }
        };
        let author = get(author_i);
        let author_opt = if author.is_empty() { None } else { Some(author.clone()) };
        let note = get(note_i);
        let date = get(date_i);
        let location = get(loc_i);
        let url = get(url_i);
        let color = get(color_i);
        let tags: Vec<String> = {
            let raw = get(tags_i);
            if raw.is_empty() {
                vec![]
            } else {
                raw.split([';', ','])
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .collect()
            }
        };

        // Preserve every column verbatim in source_data.
        let mut all_cols = serde_json::Map::new();
        for (i, h) in headers.iter().enumerate() {
            if let Some(v) = rec.get(i) {
                all_cols.insert(h.clone(), serde_json::Value::String(v.to_string()));
            }
        }

        let wid = work_id("csv", &title, &author);
        if seen.insert(wid.clone()) {
            works.push(Work {
                id: wid.clone(),
                slug: make_slug(author_opt.as_deref(), &title, &wid),
                title: title.clone(),
                author: author_opt.clone(),
                work_type: "csv".to_string(),
                source_system: "csv".to_string(),
                source_id: None,
                url: if url.is_empty() { None } else { Some(url.clone()) },
                imported_at: now.clone(),
                updated_at: now.clone(),
                source_data: serde_json::json!({ "csv_file": path }),
            });
        }

        highlights.push((
            Highlight {
                id: highlight_id("csv", &title, &author, &text, &location),
                work_id: wid,
                text,
                note: if note.is_empty() { None } else { Some(note) },
                highlighted_at: if date.is_empty() { None } else { Some(date) },
                updated_at: Some(now.clone()),
                tags,
                location: if location.is_empty() { None } else { Some(location) },
                location_type: None,
                annotation_color: if color.is_empty() { None } else { Some(color) },
                annotation_type: None,
                format: "plain".to_string(),
                source_data: serde_json::json!({ "columns": all_cols }),
            },
            title,
            author_opt,
        ));
    }

    if highlights.is_empty() {
        bail!("No rows with highlight text found");
    }
    Ok((works, highlights))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(name: &str, body: &str) -> String {
        let p = std::env::temp_dir().join(name);
        std::fs::write(&p, body).unwrap();
        p.to_string_lossy().to_string()
    }

    #[test]
    fn inspect_detects_headers_and_delimiter() {
        let p = write("hs-csv-1.csv", "Highlight,Title,Author\nhello,Book,Jane\n");
        let ins = inspect(&p).unwrap();
        assert_eq!(ins.headers, vec!["Highlight", "Title", "Author"]);
        assert_eq!(ins.delimiter, ",");
        assert_eq!(ins.sample_rows[0], vec!["hello", "Book", "Jane"]);
    }

    #[test]
    fn import_maps_and_groups() {
        let p = write(
            "hs-csv-2.csv",
            "Quote,Book,Who,Tags\nfirst,B1,Jane,\"a; b\"\nsecond,B1,Jane,\nthird,B2,Bob,\n",
        );
        let mapping = CsvMapping {
            text: Some("Quote".into()),
            title: Some("Book".into()),
            author: Some("Who".into()),
            note: None, date: None, location: None,
            tags: Some("Tags".into()),
            url: None, color: None,
            delimiter: ",".into(),
        };
        let (works, hls) = import(&p, &mapping).unwrap();
        assert_eq!(works.len(), 2); // B1, B2
        assert_eq!(hls.len(), 3);
        assert_eq!(hls[0].0.tags, vec!["a", "b"]);
        // unmapped/extra columns preserved
        assert!(hls[0].0.source_data.get("columns").is_some());
        // idempotent: same id on re-import
        let (_, hls2) = import(&p, &mapping).unwrap();
        assert_eq!(hls[0].0.id, hls2[0].0.id);
    }

    #[test]
    fn requires_text_mapping() {
        let p = write("hs-csv-3.csv", "A,B\n1,2\n");
        let mapping = CsvMapping {
            text: None, title: None, author: None, note: None, date: None,
            location: None, tags: None, url: None, color: None, delimiter: ",".into(),
        };
        assert!(import(&p, &mapping).is_err());
    }
}
