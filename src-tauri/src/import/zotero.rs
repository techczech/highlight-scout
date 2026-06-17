use anyhow::{bail, Result};
use chrono::Utc;
use rusqlite::{Connection, OpenFlags};
use std::collections::HashMap;
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
    /// Archive root: image annotation PNGs are copied into its assets/ dir.
    archive_path: Option<String>,
}

struct AnnotationRow {
    annotation_key: String,
    work_key: String,
    parent_item_id: i64,
    attachment_key: String,
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

/// Full bibliographic metadata for one Zotero item, assembled from itemData,
/// creators, and collections.
#[derive(Default, Clone)]
struct ItemMeta {
    fields: HashMap<String, String>,
    creators: Vec<(String, String)>, // (last, first) for author-type creators
    collections: Vec<String>,
}

impl ZoteroImporter {
    #[allow(dead_code)] // used by tests and as a convenience constructor
    pub fn new(db_path: String) -> Self {
        ZoteroImporter {
            db_path,
            archive_path: None,
        }
    }

    pub fn with_archive(db_path: String, archive_path: String) -> Self {
        ZoteroImporter {
            db_path,
            archive_path: Some(archive_path),
        }
    }

    /// Zotero renders image-annotation PNGs lazily into its cache. Copy the
    /// cached render (if present) into the archive's assets/ as the highlight's
    /// asset, returning true on success. Cache layout: ~/Zotero/cache/library/<key>.png.
    fn extract_image(&self, annotation_key: &str, highlight_id: &str) -> bool {
        let Some(archive) = &self.archive_path else {
            return false;
        };
        // Cache sits beside the DB: <zotero_dir>/cache/library/<key>.png
        let zotero_dir = std::path::Path::new(&self.db_path)
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_default();
        let src = zotero_dir
            .join("cache")
            .join("library")
            .join(format!("{}.png", annotation_key));
        if !src.exists() {
            return false;
        }
        let assets = std::path::Path::new(archive).join("readings").join("assets");
        if std::fs::create_dir_all(&assets).is_err() {
            return false;
        }
        let dest = assets.join(format!("{}.png", highlight_id));
        std::fs::copy(&src, &dest).is_ok()
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
              parent.itemID AS parent_item_id,
              att_item.key AS attachment_key,
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
            JOIN items att_item ON att_item.itemID = att.itemID
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
                    parent_item_id: row.get("parent_item_id")?,
                    attachment_key: row.get("attachment_key")?,
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

        // Bulk-load full metadata for every parent item referenced.
        let parent_ids: Vec<i64> = {
            let mut set = std::collections::HashSet::new();
            rows.iter().filter(|r| set.insert(r.parent_item_id)).map(|r| r.parent_item_id).collect()
        };
        let meta = load_item_meta(&conn, &parent_ids)?;

        let mut works: Vec<Work> = Vec::new();
        let mut seen_works: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut highlights: Vec<(Highlight, String, Option<String>)> = Vec::new();
        let mut skipped_images = 0usize;

        let mut extracted_images = 0usize;

        for r in &rows {
            let is_image = r.ann_type == TYPE_IMAGE;
            let highlight_id = format!("zotero-{}", r.annotation_key);

            // Determine the body and format. Image annotations try to extract a
            // cached PNG; if the render exists they become image highlights,
            // otherwise they fall back to their comment, or are skipped.
            let mut format = "plain".to_string();
            let body = if is_image {
                if self.extract_image(&r.annotation_key, &highlight_id) {
                    extracted_images += 1;
                    format = "image".to_string();
                    // Image body holds the comment (caption) if any.
                    r.comment.clone().unwrap_or_default()
                } else {
                    match r.comment.as_deref() {
                        Some(c) if !c.trim().is_empty() => c.to_string(),
                        _ => {
                            skipped_images += 1;
                            continue;
                        }
                    }
                }
            } else {
                match (r.text.as_deref(), r.comment.as_deref()) {
                    (Some(t), _) if !t.trim().is_empty() => t.to_string(),
                    (_, Some(c)) if !c.trim().is_empty() => c.to_string(),
                    _ => continue,
                }
            };

            let title = r.title.clone().unwrap_or_else(|| "Untitled".to_string());
            let work_type = map_zotero_type(r.work_type.as_deref());

            // Register the Work once, attaching full bibliographic metadata.
            let work_id = format!("zotero-{}", r.work_key);
            if seen_works.insert(work_id.clone()) {
                let m = meta.get(&r.parent_item_id).cloned().unwrap_or_default();
                let citation = build_citation(&m, &title, r.date.as_deref(), r.work_type.as_deref());
                let authors_full: Vec<String> = m
                    .creators
                    .iter()
                    .map(|(l, f)| if f.is_empty() { l.clone() } else { format!("{}, {}", l, f) })
                    .collect();
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
                    source_data: serde_json::json!({
                        "zotero_key": r.work_key,
                        "item_type": r.work_type,
                        "date": r.date,
                        "citation": citation,
                        "authors": authors_full,
                        "collections": m.collections,
                        "fields": m.fields,
                    }),
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
                    id: highlight_id,
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
                    format,
                    source_data: serde_json::json!({
                        "zotero_color_hex": r.color,
                        "zotero_annotation_key": r.annotation_key,
                        "zotero_attachment_key": r.attachment_key,
                        "page_label": r.page_label,
                    }),
                },
                title,
                r.author.clone(),
            ));
        }

        if extracted_images > 0 {
            eprintln!("Zotero import: extracted {} image annotations", extracted_images);
        }
        if skipped_images > 0 {
            eprintln!(
                "Zotero import: skipped {} image annotations (no cached render, no comment)",
                skipped_images
            );
        }

        Ok((works, highlights))
    }
}

/// Bulk-load itemData fields, author creators, and collection names for a set
/// of parent items. IDs come straight from the DB (i64), so inlining them in the
/// IN clause is injection-safe and avoids per-item round trips.
fn load_item_meta(conn: &Connection, ids: &[i64]) -> Result<HashMap<i64, ItemMeta>> {
    let mut map: HashMap<i64, ItemMeta> = HashMap::new();
    if ids.is_empty() {
        return Ok(map);
    }
    let in_list = ids
        .iter()
        .map(|i| i.to_string())
        .collect::<Vec<_>>()
        .join(",");

    // Fields.
    let fields_sql = format!(
        "SELECT id.itemID, f.fieldName, idv.value
         FROM itemData id
         JOIN itemDataValues idv ON idv.valueID = id.valueID
         JOIN fields f ON f.fieldID = id.fieldID
         WHERE id.itemID IN ({in_list})"
    );
    let mut stmt = conn.prepare(&fields_sql)?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let id: i64 = row.get(0)?;
        let name: String = row.get(1)?;
        let value: String = row.get(2)?;
        map.entry(id).or_default().fields.insert(name, value);
    }

    // Author creators in order.
    let creators_sql = format!(
        "SELECT ic.itemID, c.lastName, c.firstName
         FROM itemCreators ic
         JOIN creators c ON c.creatorID = ic.creatorID
         WHERE ic.itemID IN ({in_list}) AND ic.creatorTypeID = 1
         ORDER BY ic.itemID, ic.orderIndex"
    );
    let mut stmt = conn.prepare(&creators_sql)?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let id: i64 = row.get(0)?;
        let last: Option<String> = row.get(1)?;
        let first: Option<String> = row.get(2)?;
        map.entry(id)
            .or_default()
            .creators
            .push((last.unwrap_or_default(), first.unwrap_or_default()));
    }

    // Collections.
    let coll_sql = format!(
        "SELECT ci.itemID, col.collectionName
         FROM collectionItems ci
         JOIN collections col ON col.collectionID = ci.collectionID
         WHERE ci.itemID IN ({in_list})"
    );
    let mut stmt = conn.prepare(&coll_sql)?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let id: i64 = row.get(0)?;
        let name: String = row.get(1)?;
        map.entry(id).or_default().collections.push(name);
    }

    Ok(map)
}

/// Build a compact APA-ish citation from item metadata.
fn build_citation(m: &ItemMeta, title: &str, date: Option<&str>, item_type: Option<&str>) -> String {
    let f = |k: &str| m.fields.get(k).cloned().unwrap_or_default();

    let authors = if m.creators.is_empty() {
        String::new()
    } else {
        m.creators
            .iter()
            .map(|(l, fi)| {
                let initial = fi.chars().next().map(|c| format!(", {}.", c)).unwrap_or_default();
                format!("{}{}", l, initial)
            })
            .collect::<Vec<_>>()
            .join("; ")
    };

    let year = date
        .and_then(|d| {
            d.split(|c: char| !c.is_ascii_digit())
                .find(|p| p.len() == 4)
                .map(|s| s.to_string())
        })
        .unwrap_or_default();

    let container = {
        let p = f("publicationTitle");
        if !p.is_empty() {
            p
        } else {
            f("bookTitle")
        }
    };
    let volume = f("volume");
    let issue = f("issue");
    let pages = f("pages");
    let publisher = f("publisher");
    let doi = f("DOI");

    let mut out = String::new();
    if !authors.is_empty() {
        out.push_str(&authors);
        out.push(' ');
    }
    if !year.is_empty() {
        out.push_str(&format!("({}). ", year));
    }
    out.push_str(title.trim_end_matches('.'));
    out.push_str(". ");
    if !container.is_empty() {
        out.push_str(&container);
        if !volume.is_empty() {
            out.push_str(&format!(", {}", volume));
            if !issue.is_empty() {
                out.push_str(&format!("({})", issue));
            }
        }
        if !pages.is_empty() {
            out.push_str(&format!(", {}", pages));
        }
        out.push_str(". ");
    } else if !publisher.is_empty() {
        out.push_str(&publisher);
        out.push_str(". ");
    }
    if !doi.is_empty() {
        out.push_str(&format!("https://doi.org/{}", doi.trim_start_matches("https://doi.org/")));
    }
    // item_type kept for potential future formatting variations.
    let _ = item_type;
    out.trim().to_string()
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

    /// Verify image extraction against the real DB + Zotero cache, into a temp
    /// archive. Skips if no DB. Asserts that image-format highlights are created
    /// and the referenced PNGs land in assets/.
    #[test]
    fn extracts_image_annotations_if_present() {
        let home = std::env::var("HOME").unwrap_or_default();
        let db = format!("{}/Zotero/zotero.sqlite", home);
        if !std::path::Path::new(&db).exists() {
            eprintln!("skipping: no Zotero DB");
            return;
        }
        let archive = std::env::temp_dir().join("highlight-scout-img-test");
        let _ = std::fs::remove_dir_all(&archive);

        let importer =
            ZoteroImporter::with_archive(db, archive.to_string_lossy().to_string());
        let (_works, highlights) = importer.import_all().expect("import");

        let images: Vec<_> = highlights
            .iter()
            .filter(|(h, _, _)| h.format == "image")
            .collect();
        eprintln!("Image extraction: {} image highlights", images.len());

        // Every image highlight must have its PNG present in assets/.
        for (h, _, _) in &images {
            let png = archive
                .join("readings")
                .join("assets")
                .join(format!("{}.png", h.id));
            assert!(png.exists(), "missing asset for {}", h.id);
        }
        let _ = std::fs::remove_dir_all(&archive);
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

        // Rich metadata: most works should have a citation, some a collection,
        // and highlights should carry the PDF attachment key for zotero:// links.
        let with_citation = works
            .iter()
            .filter(|w| w.source_data.get("citation").and_then(|c| c.as_str()).map_or(false, |s| !s.is_empty()))
            .count();
        let with_collections = works
            .iter()
            .filter(|w| w.source_data.get("collections").and_then(|c| c.as_array()).map_or(false, |a| !a.is_empty()))
            .count();
        let with_attachment = highlights
            .iter()
            .filter(|(h, _, _)| h.source_data.get("zotero_attachment_key").and_then(|k| k.as_str()).map_or(false, |s| !s.is_empty()))
            .count();
        assert!(with_citation > 0, "expected some works with citations");
        assert!(with_attachment > 0, "expected attachment keys for zotero links");

        eprintln!(
            "Zotero import OK: {} works ({} cited, {} in collections), {} highlights ({} coloured, {} w/ attachment)",
            works.len(),
            with_citation,
            with_collections,
            highlights.len(),
            with_color,
            with_attachment,
        );
    }
}
