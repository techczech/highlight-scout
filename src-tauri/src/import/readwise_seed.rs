use anyhow::{bail, Result};
use chrono::Utc;
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};

use crate::import::archive::make_slug;
use crate::models::{Highlight, Work};

/// Seed from the existing `highlights-archive` SQLite index (no Readwise API).
/// This avoids the LIST-endpoint rate limit for the bulk load; the API is then
/// used only for incremental updates. Returns (works, highlights, max_updated_at)
/// — the latter seeds the incremental sync cursor.
pub struct ReadwiseSeed {
    index_path: PathBuf,
}

impl ReadwiseSeed {
    /// Resolve the archive index from a path that may be the repo root or the
    /// sqlite file itself.
    pub fn new(archive_path: &str) -> Self {
        let p = Path::new(archive_path);
        let index_path = if p.extension().map(|e| e == "sqlite").unwrap_or(false) {
            p.to_path_buf()
        } else {
            p.join("indexes").join("highlights.sqlite")
        };
        ReadwiseSeed { index_path }
    }

    fn open(&self) -> Result<Connection> {
        if !self.index_path.exists() {
            bail!(
                "Readwise archive index not found at {}",
                self.index_path.display()
            );
        }
        let uri = format!("file:{}?immutable=1", self.index_path.display());
        Ok(Connection::open_with_flags(
            uri,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
        )?)
    }

    pub fn import_all(
        &self,
    ) -> Result<(Vec<Work>, Vec<(Highlight, String, Option<String>)>, String)> {
        let conn = self.open()?;
        let now = Utc::now().to_rfc3339();

        let sql = "SELECT local_highlight_id, source_highlight_id, local_work_id, path, title,
                          author, type, source_url, readwise_url, reader_document_id, text, note,
                          context, tags_json, highlighted_at, updated_at, location, location_type, url
                   FROM highlights
                   ORDER BY local_work_id";
        let mut stmt = conn.prepare(sql)?;

        let mut works: Vec<Work> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut highlights: Vec<(Highlight, String, Option<String>)> = Vec::new();
        let mut max_updated = String::new();

        let rows = stmt.query_map([], |r| {
            Ok(SeedRow {
                local_highlight_id: r.get(0)?,
                source_highlight_id: r.get(1)?,
                local_work_id: r.get(2)?,
                title: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
                author: r.get(5)?,
                work_type: r.get::<_, Option<String>>(6)?.unwrap_or_default(),
                source_url: r.get(7)?,
                readwise_url: r.get(8)?,
                reader_document_id: r.get(9)?,
                text: r.get::<_, Option<String>>(10)?.unwrap_or_default(),
                note: r.get(11)?,
                context: r.get(12)?,
                tags_json: r.get(13)?,
                highlighted_at: r.get(14)?,
                updated_at: r.get(15)?,
                location: r.get(16)?,
                location_type: r.get(17)?,
                url: r.get(18)?,
            })
        })?;

        for row in rows.filter_map(|r| r.ok()) {
            if let Some(u) = &row.updated_at {
                if u.as_str() > max_updated.as_str() {
                    max_updated = u.clone();
                }
            }

            let work_id = row.local_work_id.clone();
            let title = if row.title.is_empty() { "Untitled".to_string() } else { row.title.clone() };
            if seen.insert(work_id.clone()) {
                works.push(Work {
                    id: work_id.clone(),
                    slug: make_slug(row.author.as_deref(), &title, &work_id),
                    title: title.clone(),
                    author: row.author.clone(),
                    work_type: singular_type(&row.work_type),
                    source_system: "readwise".to_string(),
                    source_id: Some(work_id.clone()),
                    url: row.source_url.clone().or_else(|| row.url.clone()),
                    imported_at: now.clone(),
                    updated_at: row.updated_at.clone().unwrap_or_else(|| now.clone()),
                    source_data: serde_json::json!({
                        "readwise_url": row.readwise_url,
                        "reader_document_id": row.reader_document_id,
                    }),
                });
            }

            let tags: Vec<String> = row
                .tags_json
                .as_deref()
                .and_then(|t| serde_json::from_str(t).ok())
                .unwrap_or_default();

            highlights.push((
                Highlight {
                    id: row.local_highlight_id.clone(),
                    work_id: work_id.clone(),
                    text: row.text.clone(),
                    note: row.note.clone().filter(|n| !n.is_empty()),
                    highlighted_at: row.highlighted_at.clone(),
                    updated_at: row.updated_at.clone(),
                    tags,
                    location: row.location.clone(),
                    location_type: row.location_type.clone(),
                    annotation_color: None,
                    annotation_type: None,
                    format: "plain".to_string(),
                    source_data: serde_json::json!({
                        "readwise_url": row.readwise_url,
                        "source_highlight_id": row.source_highlight_id,
                        "context": row.context,
                    }),
                },
                title,
                row.author.clone(),
            ));
        }

        Ok((works, highlights, max_updated))
    }
}

struct SeedRow {
    local_highlight_id: String,
    source_highlight_id: Option<String>,
    local_work_id: String,
    title: String,
    author: Option<String>,
    work_type: String,
    source_url: Option<String>,
    readwise_url: Option<String>,
    reader_document_id: Option<String>,
    text: String,
    note: Option<String>,
    context: Option<String>,
    tags_json: Option<String>,
    highlighted_at: Option<String>,
    updated_at: Option<String>,
    location: Option<String>,
    location_type: Option<String>,
    url: Option<String>,
}

fn singular_type(t: &str) -> String {
    match t.to_lowercase().as_str() {
        "articles" => "article",
        "books" => "book",
        "tweets" => "tweet",
        "podcasts" => "podcast",
        "pdfs" => "pdf",
        "supplementals" => "supplemental",
        other => other,
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeds_from_existing_archive_if_present() {
        let home = std::env::var("HOME").unwrap_or_default();
        let archive = format!("{}/gitrepos/16_writing_and_research/highlights-archive", home);
        let index = format!("{}/indexes/highlights.sqlite", archive);
        if !std::path::Path::new(&index).exists() {
            eprintln!("skipping: no Readwise archive index");
            return;
        }

        let (works, highlights, max_updated) =
            ReadwiseSeed::new(&archive).import_all().expect("seed");

        assert!(!works.is_empty());
        assert!(!highlights.is_empty());
        // IDs must match the rw_book_/rw_highlight_ scheme so API updates upsert.
        assert!(works.iter().all(|w| w.id.starts_with("rw_book_")));
        assert!(highlights.iter().all(|(h, _, _)| h.id.starts_with("rw_highlight_")));

        eprintln!(
            "Seed OK: {} works, {} highlights, cursor={}",
            works.len(),
            highlights.len(),
            max_updated
        );
    }
}
