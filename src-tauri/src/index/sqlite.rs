use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::Path;

use crate::models::{Highlight, SearchResult, Work};

pub fn open(index_path: &Path) -> Result<Connection> {
    let conn = Connection::open(index_path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
    Ok(conn)
}

pub fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS works (
            id TEXT PRIMARY KEY,
            slug TEXT UNIQUE NOT NULL,
            title TEXT NOT NULL,
            author TEXT,
            work_type TEXT NOT NULL DEFAULT 'article',
            source_system TEXT NOT NULL,
            source_id TEXT,
            url TEXT,
            imported_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            source_data TEXT NOT NULL DEFAULT '{}'
        );

        CREATE TABLE IF NOT EXISTS highlights (
            id TEXT PRIMARY KEY,
            work_id TEXT NOT NULL REFERENCES works(id),
            text TEXT NOT NULL,
            note TEXT,
            highlighted_at TEXT,
            updated_at TEXT,
            tags TEXT NOT NULL DEFAULT '[]',
            location TEXT,
            location_type TEXT,
            annotation_color TEXT,
            annotation_type TEXT,
            format TEXT NOT NULL DEFAULT 'plain',
            source_data TEXT NOT NULL DEFAULT '{}'
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS search_index USING fts5(
            highlight_id UNINDEXED,
            work_id UNINDEXED,
            text,
            note,
            title,
            author,
            tags,
            tokenize='porter unicode61'
        );

        CREATE INDEX IF NOT EXISTS idx_highlights_work ON highlights(work_id);
        CREATE INDEX IF NOT EXISTS idx_works_source ON works(source_system, source_id);
    ",
    )?;
    Ok(())
}

pub fn upsert_work(conn: &Connection, work: &Work) -> Result<()> {
    conn.execute(
        "INSERT INTO works (id, slug, title, author, work_type, source_system, source_id, url,
          imported_at, updated_at, source_data)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)
         ON CONFLICT(id) DO UPDATE SET
           title=excluded.title, author=excluded.author, url=excluded.url,
           updated_at=excluded.updated_at, source_data=excluded.source_data",
        params![
            work.id,
            work.slug,
            work.title,
            work.author,
            work.work_type,
            work.source_system,
            work.source_id,
            work.url,
            work.imported_at,
            work.updated_at,
            serde_json::to_string(&work.source_data).unwrap_or_default()
        ],
    )?;
    Ok(())
}

pub fn upsert_highlight(
    conn: &Connection,
    h: &Highlight,
    work_title: &str,
    work_author: Option<&str>,
) -> Result<()> {
    let tags_str = serde_json::to_string(&h.tags).unwrap_or_default();
    let source_data_str = serde_json::to_string(&h.source_data).unwrap_or_default();

    let exists: bool = conn
        .query_row(
            "SELECT 1 FROM highlights WHERE id = ?1",
            params![h.id],
            |_| Ok(true),
        )
        .unwrap_or(false);

    if exists {
        conn.execute(
            "UPDATE highlights SET work_id=?2, text=?3, note=?4, highlighted_at=?5,
             updated_at=?6, tags=?7, location=?8, location_type=?9,
             annotation_color=?10, annotation_type=?11, format=?12, source_data=?13
             WHERE id=?1",
            params![
                h.id,
                h.work_id,
                h.text,
                h.note,
                h.highlighted_at,
                h.updated_at,
                tags_str,
                h.location,
                h.location_type,
                h.annotation_color,
                h.annotation_type,
                h.format,
                source_data_str
            ],
        )?;
        conn.execute(
            "DELETE FROM search_index WHERE highlight_id = ?1",
            params![h.id],
        )?;
    } else {
        conn.execute(
            "INSERT INTO highlights
             (id, work_id, text, note, highlighted_at, updated_at, tags,
              location, location_type, annotation_color, annotation_type, format, source_data)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
            params![
                h.id,
                h.work_id,
                h.text,
                h.note,
                h.highlighted_at,
                h.updated_at,
                tags_str,
                h.location,
                h.location_type,
                h.annotation_color,
                h.annotation_type,
                h.format,
                source_data_str
            ],
        )?;
    }

    conn.execute(
        "INSERT INTO search_index
         (highlight_id, work_id, text, note, title, author, tags)
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![
            h.id,
            h.work_id,
            h.text,
            h.note.as_deref().unwrap_or(""),
            work_title,
            work_author.unwrap_or(""),
            h.tags.join(" ")
        ],
    )?;

    Ok(())
}

pub fn search(
    conn: &Connection,
    query: &str,
    source: Option<&str>,
    color: Option<&str>,
    limit: usize,
) -> Result<Vec<SearchResult>> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }

    let safe_query = sanitize_fts_query(query);

    // Build the parameter list and optional WHERE clauses dynamically so the
    // same code path serves filtered and unfiltered searches.
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(safe_query)];
    let mut clauses = String::new();
    if let Some(src) = source {
        params.push(Box::new(src.to_string()));
        clauses.push_str(&format!(" AND w.source_system = ?{}", params.len()));
    }
    if let Some(col) = color {
        params.push(Box::new(col.to_string()));
        clauses.push_str(&format!(" AND h.annotation_color = ?{}", params.len()));
    }
    params.push(Box::new(limit as i64));
    let limit_idx = params.len();

    let sql = format!(
        "SELECT search_index.highlight_id, search_index.work_id,
               h.text, h.note, w.title, w.author, w.work_type, w.source_system, w.url,
               h.highlighted_at, h.tags, h.annotation_color,
               snippet(search_index, 2, '<mark>', '</mark>', '...', 30) as snippet
        FROM search_index
        JOIN highlights h ON h.id = search_index.highlight_id
        JOIN works w ON w.id = search_index.work_id
        WHERE search_index MATCH ?1{clauses}
        ORDER BY search_index.rank
        LIMIT ?{limit_idx}"
    );

    let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|b| b.as_ref()).collect();

    let mut stmt = conn.prepare(&sql)?;
    let results: Vec<SearchResult> = stmt
        .query_map(param_refs.as_slice(), |row| {
            let tags_str: String = row.get(10)?;
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
            Ok(SearchResult {
                highlight_id: row.get(0)?,
                work_id: row.get(1)?,
                text: row.get(2)?,
                note: row.get(3)?,
                title: row.get(4)?,
                author: row.get(5)?,
                work_type: row.get(6)?,
                source_system: row.get(7)?,
                url: row.get(8)?,
                highlighted_at: row.get(9)?,
                tags,
                annotation_color: row.get(11)?,
                snippet: row.get(12)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(results)
}

/// Distinct facets for the filter UI: sources and colours actually present.
pub fn facets(conn: &Connection) -> Result<(Vec<String>, Vec<String>)> {
    let mut src_stmt =
        conn.prepare("SELECT DISTINCT source_system FROM works ORDER BY source_system")?;
    let sources: Vec<String> = src_stmt
        .query_map([], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    let mut col_stmt = conn.prepare(
        "SELECT annotation_color, COUNT(*) c FROM highlights
         WHERE annotation_color IS NOT NULL
         GROUP BY annotation_color ORDER BY c DESC",
    )?;
    let colors: Vec<String> = col_stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();

    Ok((sources, colors))
}

fn sanitize_fts_query(query: &str) -> String {
    // For multi-word queries, wrap each word with prefix matching
    // Simple approach: if no FTS operators, add * to last token for prefix matching
    let trimmed = query.trim();
    if trimmed.contains('"') || trimmed.contains(' ') {
        // Multi-word or quoted: pass through, let FTS5 handle it
        trimmed.to_string()
    } else {
        // Single word: add prefix wildcard for live search feel
        format!("{}*", trimmed)
    }
}

pub fn highlight_count(conn: &Connection) -> usize {
    conn.query_row("SELECT COUNT(*) FROM highlights", [], |row| row.get(0))
        .unwrap_or(0)
}

pub fn work_count(conn: &Connection) -> usize {
    conn.query_row("SELECT COUNT(*) FROM works", [], |row| row.get(0))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::zotero::ZoteroImporter;

    #[test]
    fn full_zotero_pipeline_indexes_and_searches() {
        let home = std::env::var("HOME").unwrap_or_default();
        let db = format!("{}/Zotero/zotero.sqlite", home);
        if !std::path::Path::new(&db).exists() {
            eprintln!("skipping: no Zotero DB");
            return;
        }

        let conn = Connection::open_in_memory().expect("in-memory db");
        init_schema(&conn).expect("schema");

        let (works, highlights) = ZoteroImporter::new(db).import_all().expect("import");
        for w in &works {
            upsert_work(&conn, w).expect("upsert work");
        }
        for (h, title, author) in &highlights {
            upsert_highlight(&conn, h, title, author.as_deref()).expect("upsert highlight");
        }

        assert_eq!(work_count(&conn), works.len());
        assert_eq!(highlight_count(&conn), highlights.len());

        // Pick a real token from the first highlight and confirm search finds it.
        let token = highlights
            .iter()
            .flat_map(|(h, _, _)| h.text.split_whitespace())
            .find(|w| w.chars().all(|c| c.is_alphabetic()) && w.len() > 4)
            .expect("a searchable token")
            .to_lowercase();

        let res = search(&conn, &token, None, None, 50).expect("search");
        assert!(!res.is_empty(), "search for '{}' returned nothing", token);

        // Colour filter returns a subset that all carry that colour.
        let (sources, colors) = facets(&conn).expect("facets");
        assert!(sources.contains(&"zotero".to_string()));
        assert!(!colors.is_empty());

        let filtered = search(&conn, &token, None, Some(&colors[0]), 50).expect("filtered");
        for r in &filtered {
            assert_eq!(r.annotation_color.as_deref(), Some(colors[0].as_str()));
        }

        eprintln!(
            "Pipeline OK: indexed {} works / {} highlights; '{}' → {} hits; {} colours",
            works.len(),
            highlights.len(),
            token,
            res.len(),
            colors.len()
        );
    }
}
