// Shared tweet → (Work, Highlight) construction for X-sourced tweets, so the
// file importer (x.rs) and the Readwise Reader importer (readwise_tweets.rs)
// produce identical, dedup-compatible records keyed by native tweet id.

use crate::import::archive::make_slug;
use crate::models::{Highlight, Work};

#[derive(Default)]
pub struct TweetInput {
    pub tweet_id: String,
    pub text: String,
    pub author_handle: Option<String>,
    pub author_name: Option<String>,
    pub created_at: Option<String>,
    pub url: Option<String>,
    pub images: Vec<String>,
    pub article_urls: Vec<String>,
    pub saved_as: Option<String>,
    pub reply_to_id: Option<String>,
    pub parent_text: Option<String>,
    pub parent_handle: Option<String>,
    pub quoted_tweet_id: Option<String>,
    pub quoted_text: Option<String>,
    pub quoted_handle: Option<String>,
}

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
    h.as_deref().map(|s| format!("@{}", s)).unwrap_or_else(|| "someone".to_string())
}

fn body(t: &TweetInput) -> String {
    let mut out = t.text.trim().to_string();
    if let Some(p) = t.parent_text.as_deref().filter(|s| !s.trim().is_empty()) {
        out.push_str(&format!("\n\n— Replying to {}:\n> {}", handle_label(&t.parent_handle), p.trim().replace('\n', "\n> ")));
    }
    if let Some(q) = t.quoted_text.as_deref().filter(|s| !s.trim().is_empty()) {
        out.push_str(&format!("\n\n— Quoting {}:\n> {}", handle_label(&t.quoted_handle), q.trim().replace('\n', "\n> ")));
    }
    for a in &t.article_urls { out.push_str(&format!("\n\n🔗 {}", a)); }
    for img in &t.images { out.push_str(&format!("\n\n![image]({})", img)); }
    out
}

/// Build the Work + Highlight (+ work title) for one saved tweet. Caller
/// de-duplicates Works by `Work.id`. `now` is an RFC3339 timestamp.
pub fn make_records(t: &TweetInput, now: &str) -> (Work, Highlight, String) {
    let wid = format!("x-w-{}", t.tweet_id);
    let title = truncate_title(&t.text);
    let saved_tag = match t.saved_as.as_deref() {
        Some("bookmarks") => "bookmark",
        Some("likes") => "like",
        _ => "x",
    };
    let work = Work {
        id: wid.clone(),
        slug: make_slug(t.author_handle.as_deref(), &title, &t.tweet_id),
        title: title.clone(),
        author: t.author_handle.clone(),
        work_type: "tweet".to_string(),
        source_system: "x".to_string(),
        source_id: Some(t.tweet_id.clone()),
        url: t.url.clone(),
        imported_at: now.to_string(),
        updated_at: now.to_string(),
        source_data: serde_json::json!({ "author_name": t.author_name, "saved_as": t.saved_as }),
    };
    let highlight = Highlight {
        id: format!("x-{}", t.tweet_id),
        work_id: wid,
        text: body(t),
        note: None,
        highlighted_at: t.created_at.clone(),
        updated_at: Some(now.to_string()),
        tags: vec![saved_tag.to_string()],
        location: None,
        location_type: None,
        annotation_color: None,
        annotation_type: None,
        format: "plain".to_string(),
        source_data: serde_json::json!({
            "tweet_id": t.tweet_id, "saved_as": t.saved_as,
            "reply_to_id": t.reply_to_id, "quoted_tweet_id": t.quoted_tweet_id,
            "url": t.url, "images": t.images, "article_urls": t.article_urls,
        }),
    };
    (work, highlight, title)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_tweet_records_with_native_id_and_embedded_context() {
        let t = TweetInput {
            tweet_id: "222".into(), text: "my comment".into(),
            author_handle: Some("bob".into()), saved_as: Some("bookmarks".into()),
            quoted_text: Some("the original".into()), quoted_handle: Some("carol".into()),
            images: vec!["https://pbs.twimg.com/media/AAA.jpg".into()],
            article_urls: vec!["https://example.com/post".into()],
            ..Default::default()
        };
        let (w, h, _title) = make_records(&t, "2026-06-21T00:00:00Z");
        assert_eq!(w.id, "x-w-222");
        assert_eq!(w.work_type, "tweet");
        assert_eq!(w.source_system, "x");
        assert_eq!(h.id, "x-222");
        assert_eq!(h.tags, vec!["bookmark".to_string()]);
        assert!(h.text.contains("Quoting @carol"));
        assert!(h.text.contains("https://example.com/post"));
        assert!(h.text.contains("pbs.twimg.com/media/AAA.jpg"));
    }
}
