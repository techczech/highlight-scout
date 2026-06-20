// X (Twitter) importer (ADR-0013): reads the birdclaw-produced compact
// `saved.jsonl` (likes + bookmarks). One saved tweet = one Work (work_type
// "tweet") + one Highlight, keyed by the native tweet ID (`x-{tweet_id}`) so
// re-import after birdclaw enriches authors/threads upserts rather than
// duplicating. Quoted/parent context is embedded into the highlight body when
// present. Own tweets are excluded upstream (they are writing, not highlights).

use anyhow::{bail, Result};
use chrono::Utc;
use serde::Deserialize;

use crate::import::archive::make_slug;
use crate::models::{Highlight, Work};

#[derive(Debug, Deserialize)]
struct SavedTweet {
    tweet_id: String,
    #[serde(default)]
    saved_as: Option<String>, // "likes" | "bookmarks"
    text: String,
    #[serde(default)]
    author_handle: Option<String>,
    #[serde(default)]
    author_name: Option<String>,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    reply_to_id: Option<String>,
    #[serde(default)]
    parent_text: Option<String>,
    #[serde(default)]
    parent_handle: Option<String>,
    #[serde(default)]
    quoted_tweet_id: Option<String>,
    #[serde(default)]
    quoted_text: Option<String>,
    #[serde(default)]
    quoted_handle: Option<String>,
    #[serde(default)]
    images: Vec<String>,
    #[serde(default)]
    article_urls: Vec<String>,
}

/// A tweet has no title; use a one-line, length-capped preview of its text.
fn truncate_title(text: &str) -> String {
    let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() <= 70 {
        one_line
    } else {
        let t: String = one_line.chars().take(70).collect();
        format!("{}…", t.trim_end())
    }
}

fn handle_label(h: &Option<String>) -> String {
    h.as_deref()
        .map(|s| format!("@{}", s))
        .unwrap_or_else(|| "someone".to_string())
}

/// Embed reply-parent and quoted context (when present) after the tweet body as
/// labelled Markdown blockquotes, so the highlight reads in context and the
/// context is searchable.
fn body_with_context(t: &SavedTweet) -> String {
    let mut out = t.text.trim().to_string();
    if let Some(p) = t.parent_text.as_deref().filter(|s| !s.trim().is_empty()) {
        out.push_str(&format!(
            "\n\n— Replying to {}:\n> {}",
            handle_label(&t.parent_handle),
            p.trim().replace('\n', "\n> ")
        ));
    }
    if let Some(q) = t.quoted_text.as_deref().filter(|s| !s.trim().is_empty()) {
        out.push_str(&format!(
            "\n\n— Quoting {}:\n> {}",
            handle_label(&t.quoted_handle),
            q.trim().replace('\n', "\n> ")
        ));
    }
    for a in &t.article_urls {
        out.push_str(&format!("\n\n🔗 {}", a));
    }
    for img in &t.images {
        out.push_str(&format!("\n\n![image]({})", img));
    }
    out
}

pub fn import(path: &str) -> Result<(Vec<Work>, Vec<(Highlight, String, Option<String>)>)> {
    let content = std::fs::read_to_string(path)?;
    let now = Utc::now().to_rfc3339();

    let mut works = Vec::new();
    let mut highlights = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(t) = serde_json::from_str::<SavedTweet>(line) else {
            continue; // skip a malformed line rather than fail the whole import
        };
        if t.text.trim().is_empty() {
            continue;
        }

        let wid = format!("x-w-{}", t.tweet_id);
        let title = truncate_title(&t.text);
        let author = t.author_handle.clone();

        if seen.insert(wid.clone()) {
            works.push(Work {
                id: wid.clone(),
                slug: make_slug(author.as_deref(), &title, &t.tweet_id),
                title: title.clone(),
                author: author.clone(),
                work_type: "tweet".to_string(),
                source_system: "x".to_string(),
                source_id: Some(t.tweet_id.clone()),
                url: t.url.clone(),
                imported_at: now.clone(),
                updated_at: now.clone(),
                source_data: serde_json::json!({
                    "author_name": t.author_name,
                    "saved_as": t.saved_as,
                }),
            });
        }

        let saved_tag = match t.saved_as.as_deref() {
            Some("bookmarks") => "bookmark",
            Some("likes") => "like",
            _ => "x",
        };

        highlights.push((
            Highlight {
                id: format!("x-{}", t.tweet_id),
                work_id: wid,
                text: body_with_context(&t),
                note: None,
                highlighted_at: t.created_at.clone(),
                updated_at: Some(now.clone()),
                tags: vec![saved_tag.to_string()],
                location: None,
                location_type: None,
                annotation_color: None,
                annotation_type: None,
                format: "plain".to_string(),
                source_data: serde_json::json!({
                    "tweet_id": t.tweet_id,
                    "saved_as": t.saved_as,
                    "reply_to_id": t.reply_to_id,
                    "quoted_tweet_id": t.quoted_tweet_id,
                    "url": t.url,
                    "images": t.images,
                    "article_urls": t.article_urls,
                }),
            },
            title,
            author,
        ));
    }

    if highlights.is_empty() {
        bail!("No saved tweets found in {}", path);
    }
    Ok((works, highlights))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_tmp(name: &str, body: &str) -> String {
        let p = std::env::temp_dir().join(name);
        std::fs::write(&p, body).unwrap();
        p.to_str().unwrap().to_string()
    }

    #[test]
    fn imports_like_and_bookmark_with_context() {
        let l1 = r#"{"tweet_id":"111","saved_as":"likes","text":"hello world","author_handle":"alice","author_name":"Alice","created_at":"2024-01-02T00:00:00.000Z","url":"https://x.com/alice/status/111"}"#;
        let l2 = r#"{"tweet_id":"222","saved_as":"bookmarks","text":"my comment","author_handle":"bob","created_at":"2024-03-04T00:00:00.000Z","url":"https://x.com/bob/status/222","quoted_tweet_id":"999","quoted_text":"the original claim","quoted_handle":"carol","images":["https://pbs.twimg.com/media/AAA.jpg"],"article_urls":["https://example.com/post"]}"#;
        let path = write_tmp("hs-x-test.jsonl", &format!("{}\n{}\n", l1, l2));

        let (works, hls) = import(&path).unwrap();
        assert_eq!(works.len(), 2);
        assert_eq!(hls.len(), 2);

        // native tweet-id keying + tweet work type
        assert_eq!(works[0].id, "x-w-111");
        assert_eq!(works[0].work_type, "tweet");
        assert_eq!(works[0].source_system, "x");
        assert_eq!(hls[0].0.id, "x-111");
        assert_eq!(hls[0].0.highlighted_at.as_deref(), Some("2024-01-02T00:00:00.000Z"));
        assert_eq!(hls[0].0.tags, vec!["like".to_string()]);

        // quoted context embedded into the bookmark's body
        let bm_body = &hls[1].0.text;
        assert!(bm_body.contains("my comment"));
        assert!(bm_body.contains("Quoting @carol"));
        assert!(bm_body.contains("the original claim"));
        assert!(bm_body.contains("https://example.com/post")); // article link embedded
        assert!(bm_body.contains("https://pbs.twimg.com/media/AAA.jpg")); // image embedded
        assert_eq!(hls[1].0.tags, vec!["bookmark".to_string()]);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn author_less_tweet_imports_with_anonymous_url() {
        let line = r#"{"tweet_id":"333","saved_as":"likes","text":"orphan tweet","author_handle":null,"created_at":null,"url":"https://x.com/i/web/status/333"}"#;
        let path = write_tmp("hs-x-test2.jsonl", &format!("{}\n", line));
        let (works, hls) = import(&path).unwrap();
        assert_eq!(works[0].author, None);
        assert_eq!(hls[0].0.highlighted_at, None);
        assert_eq!(works[0].url.as_deref(), Some("https://x.com/i/web/status/333"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn skips_blank_and_malformed_lines() {
        let body = "\n{not json}\n{\"tweet_id\":\"444\",\"text\":\"ok\"}\n";
        let path = write_tmp("hs-x-test3.jsonl", body);
        let (_w, hls) = import(&path).unwrap();
        assert_eq!(hls.len(), 1);
        assert_eq!(hls[0].0.id, "x-444");
        let _ = std::fs::remove_file(&path);
    }
}
