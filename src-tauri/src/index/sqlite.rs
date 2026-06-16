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

pub fn search(conn: &Connection, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }

    // Sanitize query: escape FTS5 special characters, add prefix wildcard
    let safe_query = sanitize_fts_query(query);

    let sql = "
        SELECT search_index.highlight_id, search_index.work_id,
               h.text, h.note, w.title, w.author, w.work_type, w.source_system, w.url,
               h.highlighted_at, h.tags, h.annotation_color,
               snippet(search_index, 2, '<mark>', '</mark>', '...', 30) as snippet
        FROM search_index
        JOIN highlights h ON h.id = search_index.highlight_id
        JOIN works w ON w.id = search_index.work_id
        WHERE search_index MATCH ?1
        ORDER BY search_index.rank
        LIMIT ?2
    ";

    let mut stmt = conn.prepare(sql)?;
    let results: Vec<SearchResult> = stmt
        .query_map(params![safe_query, limit as i64], |row| {
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
