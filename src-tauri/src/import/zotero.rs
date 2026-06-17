use anyhow::{bail, Result};
use chrono::Utc;
use rusqlite::{Connection, OpenFlags};
use std::path::PathBuf;

use crate::import::archive::make_slug;
use crate::models::{Highlight, Work};

/// Zotero annotation type integers (itemAnnotations.type).
const TYPE_HIGHLIGHT: i64 = 1;
const TYPE_NOTE: i64 = 2;
const TYPE_IMAGE: i64 = 3;
const TYPE_UNDERLINE: i64 = 5;

pub struct ZoteroImporter {
    db_path: String,
}

struct AnnotationRow {
    annotation_key: String,
    work_key: String,
    ann_type: i64,
    color: Option<String>,
    text: Option<String>,
    comment: Option<String>,
    page_label: Option<String>,
    title: Option<String>,
    author: Option<String>,
    url: Option<String>,
    date: Option<String>,
    work_type: Option<String>,
}

impl ZoteroImporter {
    pub fn new(db_path: String) -> Self {
        ZoteroImporter { db_path }
    }

    /// Open the Zotero DB read-only and immutable so a running Zotero instance
    /// does not block the read (ADR-0006: direct SQLite, no app required).
    fn open(&self) -> Result<Connection> {
        let path = PathBuf::from(&self.db_path);
        if !path.exists() {
            bail!("Zotero database not found at {}", self.db_path);
        }
        let uri = format!("file:{}?immutable=1", path.display());
        let conn = Connection::open_with_flags(
            uri,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
        )?;
        Ok(conn)
    }

    pub fn import_all(&self) -> Result<(Vec<Work>, Vec<(Highlight, String, Option<String>)>)> {
        let conn = self.open()?;
        let now = Utc::now().to_rfc3339();

        let sql = "
            SELECT
              aitem.key AS annotation_key,
              pitem.key AS work_key,
              ann.type AS ann_type,
              ann.color AS color,
              ann.text AS text,
              ann.comment AS comment,
              ann.pageLabel AS page_label,
              titleval.value AS title,
              (SELECT cr.lastName FROM itemCreators ic
                 JOIN creators cr ON cr.creatorID = ic.creatorID
                 WHERE ic.itemID = parent.itemID AND ic.creatorTypeID = 1
                 ORDER BY ic.orderIndex LIMIT 1) AS author,
              urlval.value AS url,
              dateval.value AS date,
              it.typeName AS work_type
            FROM itemAnnotations ann
            JOIN items aitem ON aitem.itemID = ann.itemID
            JOIN itemAttachments att ON att.itemID = ann.parentItemID
            JOIN items parent ON parent.itemID = att.parentItemID
            JOIN items pitem ON pitem.itemID = parent.itemID
            JOIN itemTypes it ON it.itemTypeID = parent.itemTypeID
            LEFT JOIN itemData td ON td.itemID = parent.itemID AND td.fieldID = 110
            LEFT JOIN itemDataValues titleval ON titleval.valueID = td.valueID
            LEFT JOIN itemData ud ON ud.itemID = parent.itemID AND ud.fieldID = 1
            LEFT JOIN itemDataValues urlval ON urlval.valueID = ud.valueID
            LEFT JOIN itemData dd ON dd.itemID = parent.itemID AND dd.fieldID = 14
            LEFT JOIN itemDataValues dateval ON dateval.valueID = dd.valueID
            ORDER BY pitem.key, ann.sortIndex
        ";

        let mut stmt = conn.prepare(sql)?;
        let rows: Vec<AnnotationRow> = stmt
            .query_map([], |row| {
                Ok(AnnotationRow {
                    annotation_key: row.get("annotation_key")?,
                    work_key: row.get("work_key")?,
                    ann_type: row.get("ann_type")?,
                    color: row.get("color")?,
                    text: row.get("text")?,
                    comment: row.get("comment")?,
                    page_label: row.get("page_label")?,
                    title: row.get("title")?,
                    author: row.get("author")?,
                    url: row.get("url")?,
                    date: row.get("date")?,
                    work_type: row.get("work_type")?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        let mut works: Vec<Work> = Vec::new();
        let mut seen_works: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut highlights: Vec<(Highlight, String, Option<String>)> = Vec::new();
        let mut skipped_images = 0usize;

        for r in &rows {
            // Skip image annotations without a comment — we cannot render the
            // captured region without extracting the PNG (deferred; logged).
            let is_image = r.ann_type == TYPE_IMAGE;
            let body = match (r.text.as_deref(), r.comment.as_deref()) {
                (Some(t), _) if !t.trim().is_empty() => t.to_string(),
                (_, Some(c)) if !c.trim().is_empty() => c.to_string(),
                _ => {
                    if is_image {
                        skipped_images += 1;
                    }
                    continue;
                }
            };

            let title = r.title.clone().unwrap_or_else(|| "Untitled".to_string());
            let work_type = map_zotero_type(r.work_type.as_deref());

            // Register the Work once.
            let work_id = format!("zotero-{}", r.work_key);
            if seen_works.insert(work_id.clone()) {
                works.push(Work {
                    id: work_id.clone(),
                    slug: make_slug(r.author.as_deref(), &title, &r.work_key),
                    title: title.clone(),
                    author: r.author.clone(),
                    work_type: work_type.to_string(),
                    source_system: "zotero".to_string(),
                    source_id: Some(r.work_key.clone()),
                    url: r.url.clone(),
                    imported_at: now.clone(),
                    updated_at: now.clone(),
                    source_data: serde_json::json!({ "zotero_key": r.work_key, "date": r.date }),
                });
            }

            let annotation_type = map_annotation_type(r.ann_type);
            // The comment is a separate user note when there is also highlight text.
            let note = match (r.text.as_deref(), r.comment.as_deref()) {
                (Some(t), Some(c)) if !t.trim().is_empty() && !c.trim().is_empty() => {
                    Some(c.to_string())
                }
                _ => None,
            };

            highlights.push((
                Highlight {
                    id: format!("zotero-{}", r.annotation_key),
                    work_id: work_id.clone(),
                    text: body,
                    note,
                    highlighted_at: None,
                    updated_at: Some(now.clone()),
                    tags: vec![],
                    location: r.page_label.clone(),
                    location_type: r.page_label.as_ref().map(|_| "page".to_string()),
                    annotation_color: map_color(r.color.as_deref()),
                    annotation_type: Some(annotation_type.to_string()),
                    format: "plain".to_string(),
                    source_data: serde_json::json!({
                        "zotero_color_hex": r.color,
                        "zotero_annotation_key": r.annotation_key,
                        "page_label": r.page_label,
                    }),
                },
                title,
                r.author.clone(),
            ));
        }

        if skipped_images > 0 {
            eprintln!(
                "Zotero import: skipped {} image annotations without comments (PNG extraction deferred)",
                skipped_images
            );
        }

        Ok((works, highlights))
    }
}

fn map_annotation_type(t: i64) -> &'static str {
    match t {
        TYPE_HIGHLIGHT => "highlight",
        TYPE_NOTE => "comment",
        TYPE_IMAGE => "image",
        TYPE_UNDERLINE => "underline",
        _ => "highlight",
    }
}

/// Map Zotero's standard 8-colour palette to names so semantic filtering works
/// (user: red = important, green = methods). Custom colours keep their hex value.
fn map_color(hex: Option<&str>) -> Option<String> {
    let hex = hex?.to_lowercase();
    let name = match hex.as_str() {
        "#ffd400" => "yellow",
        "#ff6666" => "red",
        "#5fb236" => "green",
        "#2ea8e5" => "blue",
        "#a28ae5" => "purple",
        "#e56eee" => "magenta",
        "#f19837" => "orange",
        "#aaaaaa" => "gray",
        other => other,
    };
    Some(name.to_string())
}

fn map_zotero_type(type_name: Option<&str>) -> &'static str {
    match type_name {
        Some("journalArticle") | Some("preprint") | Some("magazineArticle")
        | Some("newspaperArticle") => "article",
        Some("book") | Some("bookSection") => "book",
        Some("thesis") => "thesis",
        Some("report") => "report",
        Some("conferencePaper") => "article",
        Some("webpage") | Some("blogPost") => "article",
        _ => "article",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_colors_map_to_names() {
        assert_eq!(map_color(Some("#ff6666")).as_deref(), Some("red"));
        assert_eq!(map_color(Some("#5fb236")).as_deref(), Some("green"));
        assert_eq!(map_color(Some("#FFD400")).as_deref(), Some("yellow")); // case-insensitive
        assert_eq!(map_color(None), None);
    }

    #[test]
    fn custom_colors_keep_hex() {
        assert_eq!(map_color(Some("#fff066")).as_deref(), Some("#fff066"));
    }

    #[test]
    fn annotation_types_map() {
        assert_eq!(map_annotation_type(1), "highlight");
        assert_eq!(map_annotation_type(5), "underline");
        assert_eq!(map_annotation_type(3), "image");
    }

    /// Integration test against a real Zotero DB if one is present. Skips
    /// silently otherwise so it does not fail on machines without Zotero.
    #[test]
    fn imports_real_zotero_db_if_present() {
        let home = std::env::var("HOME").unwrap_or_default();
        let db = format!("{}/Zotero/zotero.sqlite", home);
        if !std::path::Path::new(&db).exists() {
            eprintln!("skipping: no Zotero DB at {}", db);
            return;
        }

        let importer = ZoteroImporter::new(db);
        let (works, highlights) = importer.import_all().expect("import should succeed");

        assert!(!works.is_empty(), "should find at least one work");
        assert!(!highlights.is_empty(), "should find at least one highlight");

        // Every highlight must reference a real work and carry a type.
        let work_ids: std::collections::HashSet<_> = works.iter().map(|w| &w.id).collect();
        for (h, _, _) in &highlights {
            assert!(work_ids.contains(&h.work_id), "orphan highlight {}", h.id);
            assert!(h.annotation_type.is_some(), "missing annotation_type");
        }

        // At least some annotations should carry a colour.
        let with_color = highlights
            .iter()
            .filter(|(h, _, _)| h.annotation_color.is_some())
            .count();
        assert!(with_color > 0, "expected some coloured annotations");

        eprintln!(
            "Zotero import OK: {} works, {} highlights, {} coloured",
            works.len(),
            highlights.len(),
            with_color
        );
    }
}
