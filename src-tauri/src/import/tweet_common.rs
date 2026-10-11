// Shared tweet → (Work, Highlight) construction for X-sourced tweets, so the
// file importer (x.rs) and the Readwise Reader importer (readwise_tweets.rs)
// produce identical, dedup-compatible records keyed by native tweet id.

use crate::models::{Highlight, Work};
use scout_archive::markdown::make_slug;

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
    /// When set, used as the highlight body verbatim (the Reader importer
    /// pre-builds structure-aware markdown). The file/birdclaw path leaves
    /// this `None` and `body()` assembles the body from the fields.
    pub body_markdown: Option<String>,
}

/// Twitter's snowflake epoch (2010-11-04T01:42:54.657Z), in ms.
const SNOWFLAKE_EPOCH_MS: u64 = 1_288_834_974_657;
/// IDs below this were handed out one by one before the snowflake era
/// (they reached about 29.7 billion in November 2010) and carry no time. A
/// snowflake ID passes this value 7.2 seconds after the epoch.
const FIRST_SNOWFLAKE_ID: u64 = 30_000_000_000;

/// The UTC date (`YYYY-MM-DD`) a tweet was posted, read from its snowflake
/// ID: `(id >> 22) + epoch` ms. None for a non-numeric ID and for an ID from
/// before the snowflake era.
pub fn snowflake_date(tweet_id: &str) -> Option<String> {
    let id: u64 = tweet_id.trim().parse().ok()?;
    if id < FIRST_SNOWFLAKE_ID {
        return None;
    }
    let ms = i64::try_from((id >> 22) + SNOWFLAKE_EPOCH_MS).ok()?;
    chrono::DateTime::from_timestamp_millis(ms).map(|d| d.format("%Y-%m-%d").to_string())
}

/// The date for a tweet's highlight line: the source's own when it gave one,
/// else the posting date from the ID, else none.
pub fn tweet_date(source_date: Option<&str>, tweet_id: &str) -> Option<String> {
    source_date
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .map(String::from)
        .or_else(|| snowflake_date(tweet_id))
}

/// A tweet's own text made comparable across sources: HTML entities
/// resolved and whitespace collapsed. Nothing is taken off it.
pub fn comparable(own: &str) -> String {
    own.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// What X serves in place of a tweet, as comparable text: the two short
/// messages, and the whole no-script page (which is longer than the short
/// form allows) under either domain.
pub const ERROR_PAGE_TEXTS: [&str; 4] = [
    "JavaScript is not available.",
    "Something went wrong. Try reloading.",
    "JavaScript is not available. We’ve detected that JavaScript is disabled in this browser. Please enable JavaScript or switch to a supported browser to continue using twitter.com. You can see a list of supported browsers in our Help Center. Help Center",
    "JavaScript is not available. We’ve detected that JavaScript is disabled in this browser. Please enable JavaScript or switch to a supported browser to continue using x.com. You can see a list of supported browsers in our Help Center. Help Center",
];

/// True for an incoming body that is a page saved in place of the tweet:
/// its own text, made comparable, is one of `ERROR_PAGE_TEXTS`, or starts
/// with one and is under 200 characters. Asked of incoming text only (such
/// a tweet is not written); text already in the archive is never judged.
pub fn is_error_page(incoming_body: &str) -> bool {
    let t = comparable(split_context(incoming_body).0);
    ERROR_PAGE_TEXTS
        .iter()
        .any(|e| t == *e || (t.starts_with(e) && t.chars().count() < 200))
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
    h.as_deref()
        .map(|s| format!("@{}", s))
        .unwrap_or_else(|| "someone".to_string())
}

/// The openings of the context paragraphs `body` appends after a tweet's own
/// text (a parent, a quoted tweet, article links, images), and of the quoted
/// tweets the Reader importer renders as a blockquote.
const CONTEXT_OPENINGS: [&str; 5] = ["— Replying to ", "— Quoting ", "🔗 ", "![image](", ">"];

/// A highlight body split into the tweet's own text and the context after
/// it: everything from the first paragraph that opens as context does.
/// The context is empty when there is none.
pub fn split_context(body: &str) -> (&str, &str) {
    let mut offset = 0;
    let mut after_blank = true;
    for line in body.split_inclusive('\n') {
        if after_blank && CONTEXT_OPENINGS.iter().any(|o| line.starts_with(o)) {
            return (body[..offset].trim_end(), &body[offset..]);
        }
        after_blank = line.trim().is_empty();
        offset += line.len();
    }
    (body.trim_end(), "")
}

fn body(t: &TweetInput) -> String {
    if let Some(b) = &t.body_markdown {
        return b.clone();
    }
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

/// Build the Work + Highlight (+ work title) for one saved tweet. Caller
/// de-duplicates Works by `Work.id`. `now` is an RFC3339 timestamp.
/// The highlight is dated by `tweet_date`.
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
        kind: "tweet".to_string(),
        source_system: "x".to_string(),
        source_id: Some(t.tweet_id.clone()),
        url: t.url.clone(),
        imported_at: now.to_string(),
        updated_at: now.to_string(),
        source_data: serde_json::json!({ "author_name": t.author_name, "saved_as": t.saved_as }),
    };
    let highlight = Highlight {
        id: format!("x-{}", t.tweet_id),
        container_id: wid,
        text: body(t),
        note: None,
        created_at: tweet_date(t.created_at.as_deref(), &t.tweet_id),
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
    fn prebuilt_body_markdown_is_used_verbatim() {
        let t = TweetInput {
            tweet_id: "1".into(),
            text: "ignored when body_markdown is set".into(),
            images: vec!["https://pbs.twimg.com/media/AAA.jpg".into()],
            body_markdown: Some("PREBUILT\n\n---\n\n> quote".into()),
            ..Default::default()
        };
        let (_w, h, _title) = make_records(&t, "2026-06-21T00:00:00Z");
        assert_eq!(h.text, "PREBUILT\n\n---\n\n> quote");
        assert!(!h.text.contains("![image]"));
    }

    #[test]
    fn snowflake_ids_give_the_posting_date() {
        // Checked against real tweets: the first is from 2023-11-05.
        assert_eq!(
            snowflake_date("1721081463234609266").as_deref(),
            Some("2023-11-05")
        );
        assert_eq!(
            snowflake_date("2064394151441863006").as_deref(),
            Some("2026-06-09")
        );
    }

    #[test]
    fn ids_from_before_the_snowflake_era_and_non_numeric_ids_have_no_date() {
        for id in ["20", "1234567890", "29700000000", "", "abc", "12x", "-5"] {
            assert_eq!(snowflake_date(id), None, "{id}");
        }
        // u64::MAX parses and must not panic.
        let _ = snowflake_date("18446744073709551615");
        assert_eq!(snowflake_date("99999999999999999999999"), None);
    }

    #[test]
    fn the_source_date_wins_and_the_id_fills_a_missing_one() {
        let mut t = TweetInput {
            tweet_id: "1721081463234609266".into(),
            text: "hello".into(),
            created_at: Some("2024-01-02T00:00:00.000Z".into()),
            ..Default::default()
        };
        let (_, h, _) = make_records(&t, "2026-06-21T00:00:00Z");
        assert_eq!(h.created_at.as_deref(), Some("2024-01-02T00:00:00.000Z"));
        for missing in [None, Some(String::new()), Some("  ".to_string())] {
            t.created_at = missing;
            let (_, h, _) = make_records(&t, "2026-06-21T00:00:00Z");
            assert_eq!(h.created_at.as_deref(), Some("2023-11-05"));
        }
        t.tweet_id = "1234567890".into();
        let (_, h, _) = make_records(&t, "2026-06-21T00:00:00Z");
        assert_eq!(h.created_at, None);
    }

    #[test]
    fn a_body_splits_into_the_tweets_own_text_and_its_context() {
        let t = TweetInput {
            tweet_id: "222".into(),
            text: "my comment\n\nsecond paragraph".into(),
            quoted_text: Some("the original".into()),
            quoted_handle: Some("carol".into()),
            images: vec!["https://pbs.twimg.com/media/AAA.jpg".into()],
            ..Default::default()
        };
        let (_, h, _) = make_records(&t, "2026-06-21T00:00:00Z");
        let (own, context) = split_context(&h.text);
        assert_eq!(own, "my comment\n\nsecond paragraph");
        assert!(
            context.starts_with("— Quoting @carol:\n> the original"),
            "{context}"
        );
        assert!(context.ends_with("![image](https://pbs.twimg.com/media/AAA.jpg)"));
        assert_eq!(format!("{own}\n\n{context}"), h.text);
        assert_eq!(
            split_context("just text\n> not after a blank line"),
            ("just text\n> not after a blank line", "")
        );
        assert_eq!(split_context("![image](u)"), ("", "![image](u)"));
    }

    #[test]
    fn error_pages_are_recognised() {
        assert!(is_error_page(
            "JavaScript is not available.\n\nWe’ve detected that JavaScript is disabled in this browser."
        ));
        assert!(is_error_page("Something went wrong. Try reloading."));
        assert!(is_error_page(
            "Something went wrong.  Try reloading.\n\nRetry\n\n![image](https://x.com/a.png)"
        ));
        for tweet in [
            "I wrote some JavaScript today",
            // r2 P1: prose that opens like an error message, or discusses one.
            "Something went",
            "Something went wrong with our deployment. Here is the complete incident analysis…",
            "Why does X tell me JavaScript is disabled in this browser when it is not?",
            "something went wrong. try reloading.",
        ] {
            assert!(!is_error_page(tweet), "{tweet}");
        }
        // The short form stops at 200 characters.
        let long = format!("Something went wrong. Try reloading. {}", "x".repeat(170));
        assert!(!is_error_page(&long));
    }

    #[test]
    fn builds_tweet_records_with_native_id_and_embedded_context() {
        let t = TweetInput {
            tweet_id: "222".into(),
            text: "my comment".into(),
            author_handle: Some("bob".into()),
            saved_as: Some("bookmarks".into()),
            quoted_text: Some("the original".into()),
            quoted_handle: Some("carol".into()),
            images: vec!["https://pbs.twimg.com/media/AAA.jpg".into()],
            article_urls: vec!["https://example.com/post".into()],
            ..Default::default()
        };
        let (w, h, _title) = make_records(&t, "2026-06-21T00:00:00Z");
        assert_eq!(w.id, "x-w-222");
        assert_eq!(w.kind, "tweet");
        assert_eq!(w.source_system, "x");
        assert_eq!(h.id, "x-222");
        assert_eq!(h.tags, vec!["bookmark".to_string()]);
        assert!(h.text.contains("Quoting @carol"));
        assert!(h.text.contains("https://example.com/post"));
        assert!(h.text.contains("pbs.twimg.com/media/AAA.jpg"));
    }
}
