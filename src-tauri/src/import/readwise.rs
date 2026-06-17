use anyhow::{bail, Result};
use reqwest::Client;
use serde::Deserialize;
use chrono::Utc;

use crate::import::archive::make_slug;
use crate::models::{Highlight, Work};

const READWISE_BASE: &str = "https://readwise.io/api/v2";

#[derive(Debug, Deserialize)]
struct Paginated<T> {
    next: Option<String>,
    results: Vec<T>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct RwBook {
    id: u64,
    title: String,
    author: Option<String>,
    category: String,
    source_url: Option<String>,
    cover_image_url: Option<String>,
    highlights_url: Option<String>,
    asin: Option<String>,
    updated: String,
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct RwTag {
    name: String,
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct RwHighlight {
    id: u64,
    text: String,
    note: Option<String>,
    location: Option<i64>,
    location_type: Option<String>,
    highlighted_at: Option<String>,
    updated_at: Option<String>,
    url: Option<String>,
    book_id: u64,
    tags: Vec<RwTag>,
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

    async fn fetch_books(&self) -> Result<Vec<RwBook>> {
        let mut all = Vec::new();
        let mut url = format!("{}/books/?page_size=1000", READWISE_BASE);
        loop {
            let resp = self
                .client
                .get(&url)
                .header("Authorization", format!("Token {}", self.api_key))
                .send()
                .await?;

            if !resp.status().is_success() {
                bail!("Readwise API error: {}", resp.status());
            }

            let page: Paginated<RwBook> = resp.json().await?;
            all.extend(page.results);
            match page.next {
                Some(next) => url = next,
                None => break,
            }
        }
        Ok(all)
    }

    async fn fetch_highlights(&self) -> Result<Vec<RwHighlight>> {
        let mut all = Vec::new();
        let mut url = format!("{}/highlights/?page_size=1000", READWISE_BASE);
        loop {
            let resp = self
                .client
                .get(&url)
                .header("Authorization", format!("Token {}", self.api_key))
                .send()
                .await?;

            if !resp.status().is_success() {
                bail!("Readwise API error: {}", resp.status());
            }

            let page: Paginated<RwHighlight> = resp.json().await?;
            all.extend(page.results);
            match page.next {
                Some(next) => url = next,
                None => break,
            }
        }
        Ok(all)
    }

    pub async fn import_all(
        &self,
    ) -> Result<(
        Vec<Work>,
        Vec<(Highlight, String, Option<String>)>,
        String,
    )> {
        let now = Utc::now().to_rfc3339();

        let rw_books = self.fetch_books().await?;
        let rw_highlights = self.fetch_highlights().await?;

        // Raw import-batch snapshot in source shape (ADR-0001 provenance).
        let raw_json = serde_json::to_string(&serde_json::json!({
            "fetched_at": now,
            "books": &rw_books,
            "highlights": &rw_highlights,
        }))
        .unwrap_or_else(|_| "{}".to_string());

        // Build works
        let works: Vec<Work> = rw_books
            .iter()
            .map(|b| {
                let slug = make_slug(b.author.as_deref(), &b.title, &b.id.to_string());
                let source_data = serde_json::json!({
                    "readwise_book_id": b.id,
                    "cover_image_url": b.cover_image_url,
                    "highlights_url": b.highlights_url,
                    "asin": b.asin,
                });
                Work {
                    id: format!("rw-book-{}", b.id),
                    slug,
                    title: b.title.clone(),
                    author: b.author.clone(),
                    work_type: rw_category_to_type(&b.category).to_string(),
                    source_system: "readwise".to_string(),
                    source_id: Some(b.id.to_string()),
                    url: b.source_url.clone(),
                    imported_at: now.clone(),
                    updated_at: b.updated.clone(),
                    source_data,
                }
            })
            .collect();

        // book_id -> (work_id, title, author)
        let work_map: std::collections::HashMap<u64, (String, String, Option<String>)> = works
            .iter()
            .filter_map(|w| {
                let book_id: u64 = w.source_id.as_ref()?.parse().ok()?;
                Some((book_id, (w.id.clone(), w.title.clone(), w.author.clone())))
            })
            .collect();

        // Build highlights
        let highlights_with_meta: Vec<(Highlight, String, Option<String>)> = rw_highlights
            .into_iter()
            .filter_map(|h| {
                let (work_id, title, author) = work_map.get(&h.book_id)?.clone();
                let tags = h.tags.iter().map(|t| t.name.clone()).collect();
                let source_data = serde_json::json!({
                    "readwise_highlight_id": h.id,
                    "readwise_url": h.url,
                });
                Some((
                    Highlight {
                        id: format!("rw-hl-{}", h.id),
                        work_id,
                        text: h.text,
                        note: h.note.filter(|n| !n.is_empty()),
                        highlighted_at: h.highlighted_at,
                        updated_at: h.updated_at,
                        tags,
                        location: h.location.map(|l| l.to_string()),
                        location_type: h.location_type,
                        annotation_color: None,
                        annotation_type: None,
                        format: "plain".to_string(),
                        source_data,
                    },
                    title,
                    author,
                ))
            })
            .collect();

        Ok((works, highlights_with_meta, raw_json))
    }
}

fn rw_category_to_type(category: &str) -> &str {
    match category {
        "books" => "book",
        "articles" => "article",
        "tweets" => "tweet",
        "podcasts" => "podcast",
        _ => "article",
    }
}
