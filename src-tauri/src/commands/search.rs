use scout_index::sqlite;
use serde::Serialize;

use crate::models::{decorate, to_core_query, SearchPayload, SearchResult, TagCount, WorkPosition};
use crate::AppState;

fn normalize(s: &str) -> String {
    s.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// One semantic hit in the highlights corpus: the work's slug, the passage's
/// original text and its cosine to the query.
#[derive(Debug, Clone, PartialEq)]
pub struct MeaningHit {
    pub slug: String,
    pub quote: String,
    pub score: f64,
}

/// The work slug in a highlights-corpus document path
/// (`readings/works/<slug>.md`, maybe with a `#fragment`).
fn slug_from_rel_path(rel: &str) -> String {
    let rel = rel.split('#').next().unwrap_or(rel);
    let base = rel.rsplit('/').next().unwrap_or(rel);
    base.strip_suffix(".md").unwrap_or(base).to_string()
}

/// The engine's semantic results over the highlights corpus, one hit per
/// document, in rank order.
pub fn meaning_hits(r: &scout_corpus::SearchResults) -> Vec<MeaningHit> {
    r.results
        .iter()
        .filter(|d| d.corpus == "highlights")
        .flat_map(|d| {
            d.hits.iter().map(move |h| MeaningHit {
                slug: slug_from_rel_path(&d.rel_path),
                quote: h.quote.clone(),
                score: h.semantic_score.unwrap_or(d.score),
            })
        })
        .collect()
}

/// Map semantic hits back to highlights in the highlight index (by work slug
/// and quote; the work's first highlight when no quote matches), skipping
/// `exclude` and de-duplicating.
pub fn related_rows(
    conn: &rusqlite::Connection,
    archive: &str,
    hits: &[MeaningHit],
    exclude: Option<&str>,
) -> Vec<SearchResult> {
    let mut out: Vec<SearchResult> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for hit in hits {
        let Some(work_id) = sqlite::container_id_by_slug(conn, &hit.slug) else {
            continue;
        };
        let rows: Vec<SearchResult> = sqlite::container_records(conn, &work_id)
            .unwrap_or_default()
            .into_iter()
            .map(|hit| decorate(hit, archive))
            .collect();
        if rows.is_empty() {
            continue;
        }
        let nq = normalize(hit.quote.trim_start_matches('>'));
        let mut chosen = rows
            .iter()
            .find(|r| {
                let nt = normalize(&r.text);
                !nt.is_empty() && !nq.is_empty() && (nq.contains(&nt) || nt.contains(&nq))
            })
            .cloned()
            .unwrap_or_else(|| rows[0].clone());
        if Some(chosen.highlight_id.as_str()) == exclude {
            continue;
        }
        chosen.relevance = Some(hit.score);
        if seen.insert(chosen.highlight_id.clone()) {
            out.push(chosen);
        }
    }
    out
}

/// A highlight's text as a plain query: words only (the search grammar reads
/// `-`, `:`, quotes, `/…/` and OR as operators), capped at 60 words.
fn plain_words(text: &str) -> String {
    let cleaned: String = text
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '\'' { c } else { ' ' })
        .collect();
    cleaned
        .split_whitespace()
        .take(60)
        .map(|w| if matches!(w, "OR" | "AND" | "NOT") { w.to_lowercase() } else { w.to_string() })
        .collect::<Vec<_>>()
        .join(" ")
}

/// "Find related": highlights nearest in meaning to one highlight's text
/// (the engine's semantic search over the highlights corpus), excluding the
/// source highlight.
#[tauri::command]
pub async fn find_related(
    text: String,
    exclude_id: String,
    app: tauri::AppHandle,
) -> Result<Vec<SearchResult>, String> {
    use tauri::Manager;
    let q = plain_words(&text);
    if q.is_empty() {
        return Ok(vec![]);
    }
    let bg = app.clone();
    let hits = tauri::async_runtime::spawn_blocking(move || {
        let state = bg.state::<AppState>();
        let engine = state
            .meaning
            .search_engine(crate::corpus::engine_from_env()?)?;
        crate::corpus::search(
            &engine,
            &scout_corpus::api::SearchQuery {
                query: q,
                in_: vec!["highlights".into()],
                limit: 40,
                passage: true,
                mode: scout_corpus::SearchMode::Semantic,
            },
        )
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| {
        if e.kind == "meaning_unavailable" {
            "Find related needs the meaning index for Highlights: switch the main window to Semantic and choose Build meaning index.".to_string()
        } else {
            e.message
        }
    })?;
    let state = app.state::<AppState>();
    let archive = state.config().archive_path;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    Ok(related_rows(&conn, &archive, &meaning_hits(&hits.body), Some(&exclude_id)))
}

/// Manually OCR all pending image highlights (macOS only). Returns the count written.
#[tauri::command]
pub async fn ocr_images(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
) -> Result<usize, String> {
    use std::sync::atomic::Ordering;
    use tauri::Manager;
    let state = app.state::<crate::AppState>();
    if !crate::ocr::available() {
        return Err("OCR is only available on macOS".into());
    }
    if state.is_ocring.swap(true, Ordering::SeqCst) {
        return Err("OCR already running".into());
    }
    // OcrGuard ensures is_ocring is cleared even if run_ocr_app panics.
    let _guard = crate::ocr::OcrGuard::acquire(&state.is_ocring);
    let archive = state.config().archive_path.clone();
    let n = crate::ocr::run_ocr_app(&app, &window, &archive, None).await;
    Ok(n)
}

#[tauri::command]
pub async fn search_query(
    query: SearchPayload,
    state: tauri::State<'_, AppState>,
) -> Result<ResultPage, String> {
    let archive = state.config().archive_path;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let page = sqlite::search_query(&conn, &to_core_query(query)).map_err(|e| e.to_string())?;
    Ok(ResultPage {
        rows: page
            .rows
            .into_iter()
            .map(|hit| decorate(hit, &archive))
            .collect(),
        has_more: page.has_more,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct ResultPage {
    pub rows: Vec<SearchResult>,
    pub has_more: bool,
}

#[tauri::command]
pub async fn work_highlights(
    work_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<SearchResult>, String> {
    let archive = state.config().archive_path;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    sqlite::container_records(&conn, &work_id)
        .map(|rows| {
            rows.into_iter()
                .map(|hit| decorate(hit, &archive))
                .collect()
        })
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn highlight_position(
    work_id: String,
    location: String,
    state: tauri::State<'_, AppState>,
) -> Result<Option<WorkPosition>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    sqlite::record_position(&conn, &work_id, &location).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_highlight(
    id: String,
    state: tauri::State<'_, AppState>,
) -> Result<Option<SearchResult>, String> {
    let archive = state.config().archive_path;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    Ok(sqlite::record_by_id(&conn, &id).map(|hit| decorate(hit, &archive)))
}

#[tauri::command]
pub async fn list_tags(state: tauri::State<'_, AppState>) -> Result<Vec<TagCount>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    sqlite::list_tags(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_facets(state: tauri::State<'_, AppState>) -> Result<serde_json::Value, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let (sources, colors) = sqlite::facets(&conn).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "sources": sources, "colors": colors }))
}

#[tauri::command]
pub async fn get_stats(state: tauri::State<'_, AppState>) -> Result<serde_json::Value, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let highlights = sqlite::record_count(&conn);
    let works = sqlite::container_count(&conn);
    Ok(serde_json::json!({ "highlights": highlights, "works": works }))
}

/// Highlights matching one query in the highlight index: in all, and per
/// source system (`x`, `readwise`, `zotero`). The rail's result counts.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct HighlightCounts {
    pub total: usize,
    pub sources: std::collections::BTreeMap<String, usize>,
}

/// Far past any archive: one page holds every match.
const COUNT_PAGE: usize = 10_000_000;

/// Count what paging through `q` would show, by running the same search as
/// one page (so negatives and the regex cap apply exactly as they do to the
/// list). Keyword order is irrelevant to a count, so recency (the cheap
/// order) is used; a regex query keeps its order, since its scan is capped.
pub fn count_highlights(
    conn: &rusqlite::Connection,
    q: &scout_index::models::SearchQuery,
) -> Result<HighlightCounts, String> {
    let mut q = q.clone();
    q.page = 0;
    q.page_size = COUNT_PAGE;
    if q.regexes.is_empty() {
        q.sort = "recent".into();
    }
    let page = sqlite::search_query(conn, &q).map_err(|e| e.to_string())?;
    let mut out = HighlightCounts::default();
    for hit in page.rows {
        out.total += 1;
        *out.sources.entry(hit.source_system).or_default() += 1;
    }
    Ok(out)
}

/// The rail's result counts for a highlight index query (page and sort ignored).
#[tauri::command]
pub async fn search_counts(
    query: SearchPayload,
    state: tauri::State<'_, AppState>,
) -> Result<HighlightCounts, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    count_highlights(&conn, &to_core_query(query))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        sqlite::init_schema(&conn).unwrap();
        for (id, src) in [("wx", "x"), ("wr", "readwise"), ("wz", "zotero")] {
            conn.execute(
                "INSERT INTO works (id,slug,title,author,work_type,source_system,source_id,url,imported_at,updated_at,source_data) VALUES (?1,?1,'W',NULL,'article',?2,NULL,NULL,'t','t','{}')",
                rusqlite::params![id, src],
            )
            .unwrap();
        }
        for (id, work, text) in [
            ("h1", "wx", "testing one"),
            ("h2", "wx", "testing two"),
            ("h3", "wr", "testing three"),
            ("h4", "wr", "nothing here"),
            ("h5", "wz", "testing four, flaky"),
        ] {
            conn.execute(
                "INSERT INTO highlights (id,work_id,text,tags,format,source_data) VALUES (?1,?2,?3,'[]','plain','{}')",
                rusqlite::params![id, work, text],
            )
            .unwrap();
            sqlite::reindex_record_fts(&conn, id).unwrap();
        }
        conn
    }

    fn query(extra: serde_json::Value) -> scout_index::models::SearchQuery {
        let mut v = serde_json::json!({
            "fts": "testing", "has_positive": true, "positive_terms": ["testing"],
            "author": null, "title": null, "type": null, "tag": null,
            "after": null, "before": null, "source": null, "color": null,
            "sort": "matches", "page": 3, "page_size": 1
        });
        v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        to_core_query(serde_json::from_value(v).unwrap())
    }

    #[test]
    fn related_hits_map_to_highlights_by_slug_and_quote_without_the_source() {
        let conn = db();
        let hit = |slug: &str, quote: &str, score| MeaningHit { slug: slug.into(), quote: quote.into(), score };
        let hits = vec![
            hit("wx", "testing two", 0.9),
            hit("wr", "> Testing   THREE", 0.8),
            hit("wr", "testing three", 0.7),
            hit("gone", "testing", 0.6),
            hit("wz", "no such quote", 0.5),
            hit("wx", "testing one", 0.4),
        ];
        let rows = related_rows(&conn, "/tmp/a", &hits, Some("h1"));
        let ids: Vec<(&str, Option<f64>)> = rows.iter().map(|r| (r.highlight_id.as_str(), r.relevance)).collect();
        // h2 by quote, h3 once (case, spaces and "> " ignored), the unknown
        // slug skipped, wz's first highlight, and the source h1 left out.
        assert_eq!(ids, vec![("h2", Some(0.9)), ("h3", Some(0.8)), ("h5", Some(0.5))]);
    }

    #[test]
    fn plain_words_strip_the_query_grammar() {
        assert_eq!(plain_words("self-driving: \"cars\" OR /x/ in:tweets"), "self driving cars or x in tweets");
        assert_eq!(plain_words(&"w ".repeat(80)).split(' ').count(), 60);
        assert_eq!(slug_from_rel_path("readings/works/schon-abc.md#2"), "schon-abc");
    }

    #[test]
    fn counts_every_match_per_source_whatever_the_page() {
        let c = count_highlights(&db(), &query(serde_json::json!({}))).unwrap();
        assert_eq!(c.total, 4);
        assert_eq!(c.sources.get("x"), Some(&2));
        assert_eq!(c.sources.get("readwise"), Some(&1));
        assert_eq!(c.sources.get("zotero"), Some(&1));
    }

    #[test]
    fn counts_honour_negatives_and_ticked_sources() {
        let conn = db();
        let c = count_highlights(&conn, &query(serde_json::json!({ "negatives": ["flaky"] }))).unwrap();
        assert_eq!(c.total, 3);
        assert_eq!(c.sources.get("zotero"), None);
        let c = count_highlights(&conn, &query(serde_json::json!({ "sources": ["readwise"] }))).unwrap();
        assert_eq!(c.total, 1);
    }
}
