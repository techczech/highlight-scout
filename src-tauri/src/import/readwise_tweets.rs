// Imports the user's saved tweets from Readwise Reader (category=tweet) with
// full text. The Reader list API gives html_content (full thread/note text),
// image_url, author and source_url. We parse those into the shared tweet shape
// (tweet_common::make_records) so they dedupe with file-imported X tweets.

use anyhow::{bail, Result};
use chrono::Utc;
use serde::Deserialize;

use crate::import::tweet_common::{make_records, TweetInput};
use crate::models::{Highlight, Work};

const READER_BASE: &str = "https://readwise.io/api/v3";

#[derive(Debug, Deserialize)]
struct ReaderList {
    #[serde(rename = "nextPageCursor")]
    next_page_cursor: Option<String>,
    results: Vec<ReaderDoc>,
}

#[derive(Debug, Deserialize)]
struct ReaderDoc {
    source_url: Option<String>,
    author: Option<String>,
    image_url: Option<String>,
    title: Option<String>,
    html_content: Option<String>,
}

/// Extract the numeric tweet id from a status URL.
pub fn tweet_id(source_url: &str) -> Option<String> {
    let i = source_url.find("/status/")? + "/status/".len();
    let rest = &source_url[i..];
    let id: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if id.is_empty() { None } else { Some(id) }
}

/// Extract the @handle from a status URL.
pub fn handle(source_url: &str) -> Option<String> {
    let host = source_url.split("://").nth(1)?;
    let after_host = host.split_once('/')?.1; // "<handle>/status/..."
    let h = after_host.split('/').next()?;
    if h.is_empty() { None } else { Some(h.to_string()) }
}

/// Strip HTML to readable text, collect /media/ image URLs and external links.
pub fn parse_html(h: &str) -> (String, Vec<String>, Vec<String>) {
    let imgs: Vec<String> = regex_all(h, "<img", "src=\"", "\"")
        .into_iter().filter(|u| u.contains("/media/")).collect();
    let links: Vec<String> = regex_all(h, "<a", "href=\"", "\"")
        .into_iter()
        .filter(|u| !u.contains("twitter.com") && !u.contains("x.com")
            && !u.contains("t.co/") && !u.contains("pbs.twimg"))
        .collect();
    // text: turn block tags into newlines, drop the rest, unescape entities
    let mut t = String::with_capacity(h.len());
    let mut in_tag = false;
    let lower = h.to_lowercase();
    let bytes = h.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            // emit newline for block-closing tags
            if lower[i..].starts_with("<br") || lower[i..].starts_with("</p")
                || lower[i..].starts_with("</div") || lower[i..].starts_with("</h") {
                t.push('\n');
            }
            in_tag = true;
        } else if bytes[i] == b'>' {
            in_tag = false;
        } else if !in_tag {
            t.push(bytes[i] as char);
        }
        i += 1;
    }
    let t = html_unescape(&t);
    let t = collapse_blank_lines(&t).trim().to_string();
    (t, dedup(imgs), dedup(links))
}

fn regex_all(h: &str, tag: &str, attr: &str, end: &str) -> Vec<String> {
    // find each `<tag ... attr"VALUE"` occurrence; lightweight, no regex crate
    let mut out = Vec::new();
    let mut idx = 0;
    let hl = h.to_lowercase();
    while let Some(rel) = hl[idx..].find(tag) {
        let start = idx + rel;
        let tag_end = h[start..].find('>').map(|e| start + e).unwrap_or(h.len());
        if let Some(a) = h[start..tag_end].find(attr) {
            let vstart = start + a + attr.len();
            if let Some(ve) = h[vstart..tag_end].find(end) {
                out.push(h[vstart..vstart + ve].to_string());
            }
        }
        idx = tag_end + 1;
        if idx >= h.len() { break; }
    }
    out
}

fn dedup(v: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    v.into_iter().filter(|s| seen.insert(s.clone())).collect()
}

fn html_unescape(s: &str) -> String {
    s.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">")
     .replace("&quot;", "\"").replace("&#39;", "'").replace("&nbsp;", " ")
}

fn collapse_blank_lines(s: &str) -> String {
    let mut out = String::new();
    let mut blanks = 0;
    for line in s.lines() {
        if line.trim().is_empty() { blanks += 1; if blanks > 2 { continue; } }
        else { blanks = 0; }
        out.push_str(line); out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_tweet_id_and_handle() {
        let u = "https://twitter.com/karpathy/status/1234567890";
        assert_eq!(tweet_id(u).as_deref(), Some("1234567890"));
        assert_eq!(handle(u).as_deref(), Some("karpathy"));
    }

    #[test]
    fn parses_html_text_media_and_links() {
        let h = r#"<p>Hello <b>world</b></p><p>see <a href="https://example.com/x">link</a></p><img src="https://pbs.twimg.com/media/AAA.jpg"><img src="https://pbs.twimg.com/profile_images/av.jpg">"#;
        let (text, imgs, links) = parse_html(h);
        assert!(text.contains("Hello world"));
        assert!(text.contains("see link"));
        assert_eq!(imgs, vec!["https://pbs.twimg.com/media/AAA.jpg".to_string()]); // avatar excluded
        assert_eq!(links, vec!["https://example.com/x".to_string()]);
    }
}

/// Fetch all Reader tweet docs (full text) and build records. `updated_after`
/// is an optional ISO timestamp for incremental sync. Returns (works, highlights).
pub async fn import(
    api_key: &str,
    updated_after: Option<&str>,
) -> Result<(Vec<Work>, Vec<(Highlight, String, Option<String>)>)> {
    let now = Utc::now().to_rfc3339();
    let client = reqwest::Client::new();
    let mut cursor: Option<String> = None;
    let mut works = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut highlights = Vec::new();

    loop {
        let mut url = format!("{}/list/?category=tweet&withHtmlContent=true", READER_BASE);
        if let Some(after) = updated_after { url.push_str(&format!("&updatedAfter={}", after.replace(':', "%3A").replace('+', "%2B"))); }
        if let Some(c) = &cursor { url.push_str(&format!("&pageCursor={}", c)); }

        // 429-aware fetch
        let page: ReaderList = loop {
            let resp = client.get(&url).header("Authorization", format!("Token {}", api_key)).send().await?;
            if resp.status().as_u16() == 429 {
                let wait = resp.headers().get("retry-after").and_then(|v| v.to_str().ok())
                    .and_then(|s| s.trim().parse::<u64>().ok()).unwrap_or(20).clamp(1, 120);
                tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
                continue;
            }
            if !resp.status().is_success() { bail!("Readwise Reader error: {}", resp.status()); }
            break resp.json().await?;
        };

        for d in &page.results {
            let Some(su) = d.source_url.as_deref() else { continue };
            let Some(id) = tweet_id(su) else { continue };
            let (mut text, mut imgs, arts) = match d.html_content.as_deref() {
                Some(h) if !h.trim().is_empty() => parse_html(h),
                _ => (String::new(), Vec::new(), Vec::new()),
            };
            if text.is_empty() { text = d.title.clone().unwrap_or_default(); }
            if text.trim().is_empty() { continue; }
            if let Some(iu) = d.image_url.as_deref() { if iu.contains("/media/") && !imgs.contains(&iu.to_string()) { imgs.insert(0, iu.to_string()); } }

            let t = TweetInput {
                tweet_id: id.clone(), text,
                author_handle: handle(su), author_name: d.author.clone(),
                created_at: None, // post-date unknown from Reader; left null (birdclaw supplies it if also present)
                url: Some(format!("https://x.com/{}/status/{}", handle(su).unwrap_or_default(), id)),
                images: imgs, article_urls: arts, saved_as: Some("likes".into()),
                ..Default::default()
            };
            let (work, highlight, title) = make_records(&t, &now);
            let author = work.author.clone();
            if seen.insert(work.id.clone()) { works.push(work); }
            highlights.push((highlight, title, author));
        }

        match page.next_page_cursor { Some(c) if !c.is_empty() => cursor = Some(c), _ => break }
    }

    Ok((works, highlights))
}
