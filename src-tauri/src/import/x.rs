// X (Twitter) importer (ADR-0013): reads the birdclaw-produced compact
// `saved.jsonl` (likes + bookmarks). One saved tweet = one Work (work_type
// "tweet") + one Highlight, keyed by the native tweet ID (`x-{tweet_id}`) so
// re-import after birdclaw enriches authors/threads upserts rather than
// duplicating. Quoted/parent context is embedded into the highlight body when
// present. Own tweets are excluded upstream (they are writing, not highlights).

use anyhow::{bail, Result};
use chrono::Utc;
use serde::Deserialize;

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

        let t = crate::import::tweet_common::TweetInput {
            tweet_id: t.tweet_id.clone(), text: t.text.clone(),
            author_handle: t.author_handle.clone(), author_name: t.author_name.clone(),
            created_at: t.created_at.clone(), url: t.url.clone(),
            images: t.images.clone(), article_urls: t.article_urls.clone(),
            saved_as: t.saved_as.clone(),
            reply_to_id: t.reply_to_id.clone(), parent_text: t.parent_text.clone(),
            parent_handle: t.parent_handle.clone(), quoted_tweet_id: t.quoted_tweet_id.clone(),
            quoted_text: t.quoted_text.clone(), quoted_handle: t.quoted_handle.clone(),
            body_markdown: None,
        };
        let (work, highlight, title) = crate::import::tweet_common::make_records(&t, &now);
        let author = work.author.clone();
        if seen.insert(work.id.clone()) { works.push(work); }
        highlights.push((highlight, title, author));
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
