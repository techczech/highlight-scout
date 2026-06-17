use anyhow::{bail, Result};
use chrono::Utc;
use reqwest::Client;
use serde::Deserialize;

use crate::import::archive::make_slug;
use crate::models::{Highlight, Work};

const READWISE_BASE: &str = "https://readwise.io/api/v2";
const READER_BASE: &str = "https://readwise.io/api/v3";

// ---- Reader v3 (full-text) ----

#[derive(Debug, Deserialize)]
struct ReaderList {
    #[serde(rename = "nextPageCursor")]
    next_page_cursor: Option<String>,
    results: Vec<ReaderDoc>,
}

#[derive(Debug, Deserialize)]
struct ReaderDoc {
    source_url: Option<String>,
    location: Option<String>,
    html_content: Option<String>,
}

// ---- v2 export endpoint (the correct sync endpoint: 240/min, nested
// highlights, supports updatedAfter + pageCursor). The LIST endpoints used
// previously are throttled to 20/min and caused 429s. ----

#[derive(Debug, Deserialize, serde::Serialize)]
struct ExportResponse {
    #[serde(rename = "nextPageCursor")]
    next_page_cursor: Option<String>,
    results: Vec<ExportBook>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct ExportBook {
    user_book_id: u64,
    title: Option<String>,
    author: Option<String>,
    category: Option<String>,
    source_url: Option<String>,
    readwise_url: Option<String>,
    unique_url: Option<String>,
    highlights: Vec<ExportHighlight>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct ExportTag {
    name: String,
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct ExportHighlight {
    id: u64,
    text: String,
    note: Option<String>,
    location: Option<i64>,
    location_type: Option<String>,
    highlighted_at: Option<String>,
    updated_at: Option<String>,
    url: Option<String>,
    readwise_url: Option<String>,
    #[serde(default)]
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

    /// Bulk/incremental import via the export endpoint. Pass `updated_after`
    /// (ISO 8601) for an incremental sync; None for a full export. Returns
    /// (works, highlights+meta, raw_json).
    pub async fn import_export(
        &self,
        updated_after: Option<&str>,
    ) -> Result<(Vec<Work>, Vec<(Highlight, String, Option<String>)>, String)> {
        let now = Utc::now().to_rfc3339();
        let mut books: Vec<ExportBook> = Vec::new();
        let mut cursor: Option<String> = None;

        loop {
            let mut url = format!("{}/export/?", READWISE_BASE);
            if let Some(c) = &cursor {
                url.push_str(&format!("pageCursor={}&", c));
            }
            if let Some(after) = updated_after {
                url.push_str(&format!("updatedAfter={}", urlencoding(after)));
            }

            let resp = self
                .client
                .get(&url)
                .header("Authorization", format!("Token {}", self.api_key))
                .send()
                .await?;

            if resp.status().as_u16() == 429 {
                bail!("Readwise rate limit (429) — wait a minute and retry");
            }
            if !resp.status().is_success() {
                bail!("Readwise export error: {}", resp.status());
            }

            let page: ExportResponse = resp.json().await?;
            books.extend(page.results);
            match page.next_page_cursor {
                Some(c) if !c.is_empty() => cursor = Some(c),
                _ => break,
            }
        }

        let raw_json = serde_json::to_string(&serde_json::json!({
            "fetched_at": now,
            "updated_after": updated_after,
            "books": &books,
        }))
        .unwrap_or_else(|_| "{}".to_string());

        let mut works = Vec::new();
        let mut highlights = Vec::new();

        for b in &books {
            let title = b.title.clone().unwrap_or_else(|| "Untitled".to_string());
            let work_id = format!("rw_book_{}", b.user_book_id);
            works.push(Work {
                id: work_id.clone(),
                slug: make_slug(b.author.as_deref(), &title, &b.user_book_id.to_string()),
                title: title.clone(),
                author: b.author.clone(),
                work_type: category_to_type(b.category.as_deref()),
                source_system: "readwise".to_string(),
                source_id: Some(b.user_book_id.to_string()),
                url: b.source_url.clone().or_else(|| b.unique_url.clone()),
                imported_at: now.clone(),
                updated_at: now.clone(),
                source_data: serde_json::json!({
                    "readwise_url": b.readwise_url,
                    "user_book_id": b.user_book_id,
                }),
            });

            for h in &b.highlights {
                let tags = h.tags.iter().map(|t| t.name.clone()).collect();
                highlights.push((
                    Highlight {
                        id: format!("rw_highlight_{}", h.id),
                        work_id: work_id.clone(),
                        text: h.text.clone(),
                        note: h.note.clone().filter(|n| !n.is_empty()),
                        highlighted_at: h.highlighted_at.clone(),
                        updated_at: h.updated_at.clone(),
                        tags,
                        location: h.location.map(|l| l.to_string()),
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

        Ok((works, highlights, raw_json))
    }

    /// Fetch full article bodies from Reader (v3) → map of source_url → Markdown.
    pub async fn fetch_reader_fulltext(
        &self,
    ) -> Result<std::collections::HashMap<String, String>> {
        let mut map = std::collections::HashMap::new();
        let mut cursor: Option<String> = None;

        loop {
            let mut url = format!("{}/list/?withHtmlContent=true", READER_BASE);
            if let Some(c) = &cursor {
                url.push_str(&format!("&pageCursor={}", c));
            }

            let resp = self
                .client
                .get(&url)
                .header("Authorization", format!("Token {}", self.api_key))
                .send()
                .await?;

            if !resp.status().is_success() {
                bail!("Reader API error: {}", resp.status());
            }

            let page: ReaderList = resp.json().await?;
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

            match page.next_page_cursor {
                Some(c) if !c.is_empty() => cursor = Some(c),
                _ => break,
            }
        }

        Ok(map)
    }
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
