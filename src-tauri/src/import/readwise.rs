use anyhow::{bail, Result};
use chrono::Utc;
use reqwest::Client;
use serde::Deserialize;

use crate::models::{Highlight, Work};
use scout_archive::markdown::make_slug;

const READWISE_BASE: &str = "https://readwise.io/api/v2";
const READER_BASE: &str = "https://readwise.io/api/v3";

// ---- Reader v3 (full-text) ----

#[derive(Debug, Deserialize)]
struct ReaderList {
    #[serde(rename = "nextPageCursor", default, deserialize_with = "de_cursor")]
    next_page_cursor: Option<String>,
    #[serde(default, deserialize_with = "de_vec_or_null")]
    results: Vec<ReaderDoc>,
}

#[derive(Debug, Deserialize)]
struct ReaderDoc {
    #[serde(default)]
    source_url: Option<String>,
    #[serde(default)]
    location: Option<String>,
    #[serde(default)]
    html_content: Option<String>,
}

// ---- tolerant decoding helpers ----
//
// Readwise's v2 export sends `nextPageCursor` as an integer (a user_book_id),
// Reader v3 sends a string. Both are accepted and normalised to a string.

/// Accept a page cursor sent as a string, an integer or null.
pub(crate) fn de_cursor<'de, D>(d: D) -> std::result::Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(match Option::<serde_json::Value>::deserialize(d)? {
        Some(serde_json::Value::String(s)) if !s.is_empty() => Some(s),
        Some(serde_json::Value::Number(n)) => Some(n.to_string()),
        _ => None,
    })
}

/// Accept a location sent as an integer, a float or a string; anything else
/// (null, object) becomes None rather than failing the whole page.
fn de_location<'de, D>(d: D) -> std::result::Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(match Option::<serde_json::Value>::deserialize(d)? {
        Some(serde_json::Value::Number(n)) => Some(match n.as_i64() {
            Some(i) => i.to_string(),
            None => n.to_string(),
        }),
        Some(serde_json::Value::String(s)) if !s.is_empty() => Some(s),
        _ => None,
    })
}

/// Accept an array or null as a Vec (null -> empty).
pub(crate) fn de_vec_or_null<'de, D, T>(d: D) -> std::result::Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(d)?.unwrap_or_default())
}

/// Accept a string or null as a String (null -> "").
fn de_string_or_null<'de, D>(d: D) -> std::result::Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<String>::deserialize(d)?.unwrap_or_default())
}

/// Decode a JSON body, naming the failing field path on error. The error
/// quotes at most ~200 chars of the body and never any headers.
pub(crate) fn decode_json<T: serde::de::DeserializeOwned>(what: &str, body: &str) -> Result<T> {
    let de = &mut serde_json::Deserializer::from_str(body);
    serde_path_to_error::deserialize(de).map_err(|e| {
        let path = e.path().to_string();
        let snippet: String = body.chars().take(200).collect();
        anyhow::anyhow!(
            "{} response did not decode at field `{}`: {} (body starts: {:?})",
            what,
            path,
            e.inner(),
            snippet
        )
    })
}

/// Step a paginated loop: None ends it, a new cursor continues it, and a
/// cursor already seen in this run is an error. A repeat means the API is
/// looping; stopping quietly would report success over pages never read.
pub(crate) fn advance_cursor(
    seen: &mut std::collections::HashSet<String>,
    next: Option<String>,
    what: &str,
) -> Result<Option<String>> {
    match next {
        None => Ok(None),
        Some(c) => {
            if !seen.insert(c.clone()) {
                bail!(
                    "{} pagination returned cursor {} twice; sync stopped before completing",
                    what,
                    c
                );
            }
            Ok(Some(c))
        }
    }
}

// ---- v2 export endpoint (the correct sync endpoint: 240/min, nested
// highlights, supports updatedAfter + pageCursor). The LIST endpoints used
// previously are throttled to 20/min and caused 429s. ----

#[derive(Debug, Deserialize, serde::Serialize)]
struct ExportResponse {
    #[serde(rename = "nextPageCursor", default, deserialize_with = "de_cursor")]
    next_page_cursor: Option<String>,
    #[serde(default, deserialize_with = "de_vec_or_null")]
    results: Vec<ExportBook>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct ExportBook {
    user_book_id: u64,
    #[serde(default)]
    is_deleted: bool,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    source_url: Option<String>,
    #[serde(default)]
    readwise_url: Option<String>,
    #[serde(default)]
    unique_url: Option<String>,
    #[serde(default, deserialize_with = "de_vec_or_null")]
    highlights: Vec<ExportHighlight>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct ExportTag {
    #[serde(default, deserialize_with = "de_string_or_null")]
    name: String,
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct ExportHighlight {
    id: u64,
    #[serde(default)]
    is_deleted: bool,
    #[serde(default, deserialize_with = "de_string_or_null")]
    text: String,
    #[serde(default)]
    note: Option<String>,
    #[serde(default, deserialize_with = "de_location")]
    location: Option<String>,
    #[serde(default)]
    location_type: Option<String>,
    #[serde(default)]
    highlighted_at: Option<String>,
    #[serde(default)]
    updated_at: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    readwise_url: Option<String>,
    #[serde(default, deserialize_with = "de_vec_or_null")]
    tags: Vec<ExportTag>,
}

pub struct ReadwiseClient {
    client: Client,
    api_key: String,
}

impl ReadwiseClient {
    pub fn new(api_key: String) -> Self {
        ReadwiseClient {
            client: Client::new(),
            api_key,
        }
    }

    /// GET with auth, retrying on 429 honouring the Retry-After header (the
    /// Readwise-recommended backoff). Export is 240/min so this is rare, but a
    /// fast paginated sync or the Reader endpoint can still trip it.
    async fn get_with_retry(&self, url: &str) -> Result<reqwest::Response> {
        let mut attempts = 0;
        loop {
            let resp = self
                .client
                .get(url)
                .header("Authorization", format!("Token {}", self.api_key))
                .send()
                .await?;

            if resp.status().as_u16() == 429 && attempts < 5 {
                attempts += 1;
                let wait = resp
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.trim().parse::<u64>().ok())
                    .unwrap_or(60)
                    .clamp(1, 120);
                tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
                continue;
            }
            return Ok(resp);
        }
    }

    /// Bulk/incremental import via the export endpoint. Pass `updated_after`
    /// (ISO 8601) for an incremental sync; None for a full export. Follows
    /// `nextPageCursor` until the API stops returning one.
    pub async fn import_export(&self, updated_after: Option<&str>) -> Result<ExportBatch> {
        let now = Utc::now().to_rfc3339();
        let mut books: Vec<ExportBook> = Vec::new();
        let mut cursor: Option<String> = None;
        let mut seen = std::collections::HashSet::new();

        loop {
            let url = export_url(cursor.as_deref(), updated_after);
            let resp = self.get_with_retry(&url).await?;
            if !resp.status().is_success() {
                bail!("Readwise export error: {}", resp.status());
            }
            let body = resp.text().await?;
            let page: ExportResponse = decode_json("Readwise export", &body)?;
            books.extend(page.results);
            match advance_cursor(&mut seen, page.next_page_cursor, "Readwise export")? {
                Some(c) => cursor = Some(c),
                None => break,
            }
        }

        let raw_json = serde_json::to_string(&serde_json::json!({
            "fetched_at": now,
            "updated_after": updated_after,
            "books": &books,
        }))
        .unwrap_or_else(|_| "{}".to_string());

        let mut batch = build_batch(&books, &now);
        batch.raw_json = raw_json;
        Ok(batch)
    }

    /// Fetch full article bodies from Reader (v3) → map of source_url → Markdown.
    pub async fn fetch_reader_fulltext(&self) -> Result<std::collections::HashMap<String, String>> {
        let mut map = std::collections::HashMap::new();
        let mut cursor: Option<String> = None;
        let mut seen = std::collections::HashSet::new();

        loop {
            let mut url = format!("{}/list/?withHtmlContent=true", READER_BASE);
            if let Some(c) = &cursor {
                url.push_str(&format!("&pageCursor={}", c));
            }

            let resp = self.get_with_retry(&url).await?;
            if !resp.status().is_success() {
                bail!("Reader API error: {}", resp.status());
            }

            let body = resp.text().await?;
            let page: ReaderList = decode_json("Reader list", &body)?;
            for doc in page.results {
                if doc.location.as_deref() == Some("feed") {
                    continue;
                }
                let (Some(src), Some(html)) = (doc.source_url, doc.html_content) else {
                    continue;
                };
                if html.trim().is_empty() {
                    continue;
                }
                let md = htmd::convert(&html).unwrap_or(html);
                map.insert(src, md);
            }

            match advance_cursor(&mut seen, page.next_page_cursor, "Reader list")? {
                Some(c) => cursor = Some(c),
                None => break,
            }
        }

        Ok(map)
    }
}

/// One export run turned into archive records.
#[derive(Debug, Default)]
pub struct ExportBatch {
    pub works: Vec<Work>,
    pub highlights: Vec<(Highlight, String, Option<String>)>,
    /// Highlight ids (`rw_highlight_N`) Readwise reports as deleted.
    pub deleted_ids: std::collections::HashSet<String>,
    pub raw_json: String,
}

fn export_url(cursor: Option<&str>, updated_after: Option<&str>) -> String {
    let mut url = format!("{}/export/?", READWISE_BASE);
    if let Some(c) = cursor {
        url.push_str(&format!("pageCursor={}&", c));
    }
    if let Some(after) = updated_after {
        url.push_str(&format!("updatedAfter={}", urlencoding(after)));
    }
    url
}

/// Turn decoded export books into Works + Highlights. Deleted highlights are
/// not imported; their ids are returned so the archive can drop them.
fn build_batch(books: &[ExportBook], now: &str) -> ExportBatch {
    let mut batch = ExportBatch::default();
    for b in books {
        if b.is_deleted {
            for h in &b.highlights {
                batch.deleted_ids.insert(format!("rw_highlight_{}", h.id));
            }
            continue;
        }
        let title = b.title.clone().unwrap_or_else(|| "Untitled".to_string());
        let work_id = format!("rw_book_{}", b.user_book_id);
        batch.works.push(Work {
            id: work_id.clone(),
            slug: make_slug(b.author.as_deref(), &title, &b.user_book_id.to_string()),
            title: title.clone(),
            author: b.author.clone(),
            kind: category_to_type(b.category.as_deref()),
            source_system: "readwise".to_string(),
            source_id: Some(b.user_book_id.to_string()),
            url: b.source_url.clone().or_else(|| b.unique_url.clone()),
            imported_at: now.to_string(),
            updated_at: now.to_string(),
            source_data: serde_json::json!({
                "readwise_url": b.readwise_url,
                "user_book_id": b.user_book_id,
            }),
        });

        for h in &b.highlights {
            let id = format!("rw_highlight_{}", h.id);
            if h.is_deleted {
                batch.deleted_ids.insert(id);
                continue;
            }
            let tags = h
                .tags
                .iter()
                .map(|t| t.name.clone())
                .filter(|n| !n.is_empty())
                .collect();
            batch.highlights.push((
                Highlight {
                    id,
                    container_id: work_id.clone(),
                    text: h.text.clone(),
                    note: h.note.clone().filter(|n| !n.is_empty()),
                    created_at: h.highlighted_at.clone(),
                    updated_at: h.updated_at.clone(),
                    tags,
                    location: h.location.clone(),
                    location_type: h.location_type.clone(),
                    annotation_color: None,
                    annotation_type: None,
                    format: "plain".to_string(),
                    source_data: serde_json::json!({
                        "readwise_url": h.readwise_url.clone().or_else(|| h.url.clone()),
                        "source_highlight_id": h.id,
                    }),
                },
                title.clone(),
                b.author.clone(),
            ));
        }
    }
    batch
}

/// Incremental exports return, per book, only the highlights changed since
/// `updatedAfter`. Rendering a work file from that subset would drop every
/// older highlight, so the archive is written from the union: highlights
/// already known for the work (from the index), replaced by any incoming
/// version with the same id, minus anything Readwise reports deleted.
/// Output is in reading order (numeric location, then highlighted_at).
pub fn merge_incremental(
    known: Vec<Highlight>,
    incoming: &[&Highlight],
    deleted: &std::collections::HashSet<String>,
) -> Vec<Highlight> {
    let incoming_ids: std::collections::HashSet<&str> =
        incoming.iter().map(|h| h.id.as_str()).collect();
    let mut out: Vec<Highlight> = known
        .into_iter()
        .filter(|h| !incoming_ids.contains(h.id.as_str()) && !deleted.contains(&h.id))
        .collect();
    out.extend(
        incoming
            .iter()
            .filter(|h| !deleted.contains(&h.id))
            .map(|h| (*h).clone()),
    );
    sort_reading_order(&mut out);
    out
}

/// The one order every Readwise work file is written in, whichever path
/// (full or incremental) wrote it: numeric location (parsed as f64, so
/// "9.5" < "10"), highlights with no numeric location last, then
/// created_at, then id.
pub fn sort_reading_order(list: &mut [Highlight]) {
    fn loc(h: &Highlight) -> Option<f64> {
        h.location
            .as_deref()
            .and_then(|l| l.trim().parse::<f64>().ok())
            .filter(|f| f.is_finite())
    }
    list.sort_by(|a, b| {
        let by_loc = match (loc(a), loc(b)) {
            (Some(x), Some(y)) => x.total_cmp(&y),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        };
        by_loc
            .then_with(|| a.created_at.cmp(&b.created_at))
            .then_with(|| a.id.cmp(&b.id))
    });
}

fn category_to_type(category: Option<&str>) -> String {
    match category.unwrap_or("articles") {
        "books" => "book",
        "articles" => "article",
        "tweets" => "tweet",
        "podcasts" => "podcast",
        "supplementals" => "supplemental",
        other => other,
    }
    .to_string()
}

/// Minimal percent-encoding for the updatedAfter timestamp (`:` and `+`).
fn urlencoding(s: &str) -> String {
    s.replace(':', "%3A").replace('+', "%2B")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shape of the live v2 export as of 2026-09 (anonymised): integer
    /// `nextPageCursor`, `is_deleted`, `book_tags`/tag ids, `color`,
    /// `end_location: null`, plus a float location and a null tag list.
    const PAGE_2026_09: &str = include_str!("fixtures/readwise_export_page_2026-09.json");

    #[test]
    fn decodes_the_2026_09_export_shape_with_an_integer_cursor() {
        let page: ExportResponse = decode_json("Readwise export", PAGE_2026_09).unwrap();
        assert_eq!(page.next_page_cursor.as_deref(), Some("51234567"));
        assert_eq!(page.results.len(), 2);
        assert_eq!(
            page.results[1].highlights[0].location.as_deref(),
            Some("42.5")
        );
    }

    #[test]
    fn the_pre_fix_shape_fails_on_the_cursor_and_the_error_names_it() {
        // The 0.5.6 struct: cursor as Option<String>.
        #[derive(Debug, Deserialize)]
        #[allow(dead_code)]
        struct Old {
            #[serde(rename = "nextPageCursor")]
            next_page_cursor: Option<String>,
        }
        let err = decode_json::<Old>("Readwise export", PAGE_2026_09)
            .unwrap_err()
            .to_string();
        assert!(err.contains("`nextPageCursor`"), "{err}");
        assert!(
            err.chars().count() < 400,
            "error must stay short (body snippet capped at 200 chars): {err}"
        );
    }

    #[test]
    fn decode_errors_name_the_nested_field_path() {
        let bad =
            r#"{"nextPageCursor":null,"results":[{"user_book_id":1,"highlights":[{"id":"x"}]}]}"#;
        let err = decode_json::<ExportResponse>("Readwise export", bad)
            .unwrap_err()
            .to_string();
        assert!(err.contains("results[0].highlights[0].id"), "{err}");
    }

    #[test]
    fn a_null_or_missing_cursor_ends_pagination() {
        let p: ExportResponse =
            decode_json("x", r#"{"nextPageCursor":null,"results":[]}"#).unwrap();
        assert!(p.next_page_cursor.is_none());
        let p: ExportResponse = decode_json("x", r#"{"results":[]}"#).unwrap();
        assert!(p.next_page_cursor.is_none());
        let p: ReaderList = decode_json("x", r#"{"nextPageCursor":"abc","results":[]}"#).unwrap();
        assert_eq!(p.next_page_cursor.as_deref(), Some("abc"));
    }

    #[test]
    fn export_url_carries_the_cursor_and_updated_after() {
        let u = export_url(Some("51234567"), Some("2026-07-22T06:42:13+00:00"));
        assert_eq!(
            u,
            "https://readwise.io/api/v2/export/?pageCursor=51234567&updatedAfter=2026-07-22T06%3A42%3A13%2B00%3A00"
        );
    }

    #[test]
    fn build_batch_skips_deleted_highlights_and_reports_them() {
        let page: ExportResponse = decode_json("x", PAGE_2026_09).unwrap();
        let b = build_batch(&page.results, "2026-09-27T00:00:00+00:00");
        assert_eq!(b.works.len(), 2);
        let ids: Vec<&str> = b.highlights.iter().map(|(h, _, _)| h.id.as_str()).collect();
        assert_eq!(ids, ["rw_highlight_900000001", "rw_highlight_900000003"]);
        assert!(b.deleted_ids.contains("rw_highlight_900000002"));
        let (first, _, _) = &b.highlights[0];
        assert_eq!(first.tags, ["favorite"]);
        assert_eq!(first.note, None);
        assert_eq!(first.location.as_deref(), Some("1234"));
    }

    fn hl(id: &str, loc: &str, text: &str) -> Highlight {
        Highlight {
            id: id.into(),
            container_id: "rw_book_1".into(),
            text: text.into(),
            note: None,
            created_at: None,
            updated_at: None,
            tags: vec![],
            location: Some(loc.into()),
            location_type: None,
            annotation_color: None,
            annotation_type: None,
            format: "plain".into(),
            source_data: serde_json::Value::Null,
        }
    }

    #[test]
    fn incremental_merge_keeps_older_highlights_and_applies_changes() {
        let known = vec![
            hl("a", "10", "old a"),
            hl("b", "20", "old b"),
            hl("c", "30", "c"),
        ];
        let changed_b = hl("b", "20", "new b");
        let new_d = hl("d", "15", "d");
        let deleted: std::collections::HashSet<String> = ["c".to_string()].into();
        let out = merge_incremental(known, &[&changed_b, &new_d], &deleted);
        let got: Vec<(&str, &str)> = out
            .iter()
            .map(|h| (h.id.as_str(), h.text.as_str()))
            .collect();
        assert_eq!(got, [("a", "old a"), ("d", "d"), ("b", "new b")]);
    }

    #[test]
    fn a_repeated_cursor_fails_the_run_instead_of_reporting_success() {
        let mut seen = std::collections::HashSet::new();
        assert_eq!(
            advance_cursor(&mut seen, Some("1".into()), "t").unwrap(),
            Some("1".into())
        );
        assert_eq!(
            advance_cursor(&mut seen, Some("2".into()), "t").unwrap(),
            Some("2".into())
        );
        // A-B-A loop, not only an immediate repeat.
        let err = advance_cursor(&mut seen, Some("1".into()), "Readwise export")
            .unwrap_err()
            .to_string();
        assert!(err.contains("cursor 1 twice"), "{err}");
        assert_eq!(advance_cursor(&mut seen, None, "t").unwrap(), None);
    }

    #[test]
    fn reading_order_is_numeric_with_unlocated_last_and_stable_ties() {
        let mut none = hl("n", "", "no location");
        none.location = None;
        let mut text_loc = hl("t", "chapter 3", "text location");
        text_loc.created_at = Some("2026-01-01".into());
        let mut tie_late = hl("z", "5", "tie late");
        tie_late.created_at = Some("2026-02-01".into());
        let mut tie_early = hl("y", "5", "tie early");
        tie_early.created_at = Some("2026-01-01".into());
        let tie_id_b = hl("b7", "7", "same loc same date b");
        let tie_id_a = hl("a7", "7", "same loc same date a");
        let mut list = vec![
            none,
            hl("ten", "10", "10"),
            tie_late,
            hl("nine5", "9.5", "9.5"),
            tie_id_b,
            text_loc,
            tie_id_a,
            hl("two", "2", "2"),
            tie_early,
        ];
        sort_reading_order(&mut list);
        let ids: Vec<&str> = list.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids, ["two", "y", "z", "a7", "b7", "nine5", "ten", "n", "t"]);
    }
}
