# Scheduled Syncs + Built-in Readwise Tweet Import — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let any Highlight Scout user import saved tweets from Readwise Reader and schedule recurring per-source syncs (Readwise highlights, Readwise tweets, Zotero) that run while the app is resident.

**Architecture:** A small `sync` registry enumerates schedulable sources and dispatches to existing async importers; an in-app tokio task ticks every 5 min and runs due sources (sequential, guarded). A new Readwise Reader tweet importer reuses a shared tweet→records helper so Readwise tweets and birdclaw tweets dedupe by native tweet ID. Per-source `{enabled, interval_hours, last_sync}` plus a user-controlled `autostart_enabled` live in `config.toml`; a new Settings "Sync" tab edits them.

**Tech Stack:** Rust (Tauri 2, tokio, reqwest, rusqlite, serde_json, sha1), React + TypeScript (Vite), Tailwind.

**Spec:** `docs/superpowers/specs/2026-06-21-scheduled-syncs-design.md`

**Branch:** `scheduled-syncs` (already created off `main`).

---

### Task 1: Version bump to 0.5.0

**Files:**
- Modify: `src/version.ts`
- Modify: `src-tauri/tauri.conf.json`

- [ ] **Step 1: Bump `src/version.ts`** — set `APP_VERSION = "0.5.0"` and prepend a RELEASE_NOTES entry:

```ts
export const APP_VERSION = "0.5.0";

export const RELEASE_NOTES: Array<{ version: string; notes: string[] }> = [
  {
    version: "0.5.0",
    notes: [
      "Import your saved tweets from Readwise (Settings → Import → \"Readwise saved tweets\"): full text, images and links.",
      "Scheduled syncs (Settings → Sync): run Readwise highlights, Readwise tweets and Zotero on a recurring schedule while the app is open.",
      "Launch at login is now an opt-in setting (Settings → Sync) instead of always on.",
    ],
  },
  // ...existing entries unchanged below...
```

- [ ] **Step 2: Bump `src-tauri/tauri.conf.json`** — change `"version": "0.4.7"` to `"version": "0.5.0"`.

- [ ] **Step 3: Commit**

```bash
git add src/version.ts src-tauri/tauri.conf.json
git commit -m "chore: bump version to 0.5.0"
```

---

### Task 2: Config fields for scheduling + autostart

**Files:**
- Modify: `src-tauri/src/config.rs`

- [ ] **Step 1: Write the failing test** — append to the bottom of `src-tauri/src/config.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_sync_fields_round_trip() {
        let mut c = Config::default();
        c.readwise_tweets_sync_enabled = true;
        c.readwise_tweets_sync_interval_hours = 6;
        c.readwise_tweets_last_sync = "2026-06-21T00:00:00Z".into();
        c.zotero_sync_enabled = true;
        c.zotero_sync_interval_hours = 24;
        c.autostart_enabled = true;

        // serialize → parse must preserve the fields
        let text = serialize(&c);
        let parsed = parse_config_text(&text);
        assert!(parsed.readwise_tweets_sync_enabled);
        assert_eq!(parsed.readwise_tweets_sync_interval_hours, 6);
        assert_eq!(parsed.readwise_tweets_last_sync, "2026-06-21T00:00:00Z");
        assert!(parsed.zotero_sync_enabled);
        assert_eq!(parsed.zotero_sync_interval_hours, 24);
        assert!(parsed.autostart_enabled);
    }
}
```

- [ ] **Step 2: Add fields to the `Config` struct** (`config.rs`), after `readwise_last_sync`:

```rust
    #[serde(default)]
    pub readwise_sync_enabled: bool,
    #[serde(default)]
    pub readwise_sync_interval_hours: u32,
    #[serde(default)]
    pub readwise_tweets_sync_enabled: bool,
    #[serde(default)]
    pub readwise_tweets_sync_interval_hours: u32,
    #[serde(default)]
    pub readwise_tweets_last_sync: String,
    #[serde(default)]
    pub zotero_sync_enabled: bool,
    #[serde(default)]
    pub zotero_sync_interval_hours: u32,
    #[serde(default)]
    pub zotero_last_sync: String,
    #[serde(default)]
    pub autostart_enabled: bool,
```

- [ ] **Step 3: Add to `Default`** impl (each: `false` / `0` / `String::new()`):

```rust
            readwise_sync_enabled: false,
            readwise_sync_interval_hours: 0,
            readwise_tweets_sync_enabled: false,
            readwise_tweets_sync_interval_hours: 0,
            readwise_tweets_last_sync: String::new(),
            zotero_sync_enabled: false,
            zotero_sync_interval_hours: 0,
            zotero_last_sync: String::new(),
            autostart_enabled: false,
```

- [ ] **Step 4: Extend `serialize`** — append these lines to the format string and args (booleans as `true`/`false`, ints bare, strings quoted):

```rust
         readwise_sync_enabled = {}\n\
         readwise_sync_interval_hours = {}\n\
         readwise_tweets_sync_enabled = {}\n\
         readwise_tweets_sync_interval_hours = {}\n\
         readwise_tweets_last_sync = \"{}\"\n\
         zotero_sync_enabled = {}\n\
         zotero_sync_interval_hours = {}\n\
         zotero_last_sync = \"{}\"\n\
         autostart_enabled = {}\n",
        // ...existing args..., then:
        config.readwise_sync_enabled,
        config.readwise_sync_interval_hours,
        config.readwise_tweets_sync_enabled,
        config.readwise_tweets_sync_interval_hours,
        config.readwise_tweets_last_sync,
        config.zotero_sync_enabled,
        config.zotero_sync_interval_hours,
        config.zotero_last_sync,
        config.autostart_enabled
```

- [ ] **Step 5: Refactor the parse loop into a testable function.** In `load()`, the line-parsing loop currently mutates a local `config`. Extract it to `pub(crate) fn parse_config_text(content: &str) -> Config` that starts from `Config::default()`, runs the existing `match key { ... }`, and returns the config. Have `load()` call it: `let mut config = parse_config_text(&content);`. Then add these match arms inside it:

```rust
                "readwise_sync_enabled" => config.readwise_sync_enabled = val == "true",
                "readwise_sync_interval_hours" => config.readwise_sync_interval_hours = val.parse().unwrap_or(0),
                "readwise_tweets_sync_enabled" => config.readwise_tweets_sync_enabled = val == "true",
                "readwise_tweets_sync_interval_hours" => config.readwise_tweets_sync_interval_hours = val.parse().unwrap_or(0),
                "readwise_tweets_last_sync" => config.readwise_tweets_last_sync = val.to_string(),
                "zotero_sync_enabled" => config.zotero_sync_enabled = val == "true",
                "zotero_sync_interval_hours" => config.zotero_sync_interval_hours = val.parse().unwrap_or(0),
                "zotero_last_sync" => config.zotero_last_sync = val.to_string(),
                "autostart_enabled" => config.autostart_enabled = val == "true",
```

(The env-var fallback for `readwise_api_key` stays in `load()`, after the call.)

- [ ] **Step 6: Run the test**

Run: `cd src-tauri && cargo test --lib config::tests::new_sync_fields_round_trip`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/config.rs
git commit -m "feat(config): per-source sync schedule + autostart fields"
```

---

### Task 3: Extract shared tweet→records helper

Both `import/x.rs` and the new Readwise-tweets importer must produce identical `Work`/`Highlight` shapes (so the same tweet dedupes). Extract the construction into one helper.

**Files:**
- Create: `src-tauri/src/import/tweet_common.rs`
- Modify: `src-tauri/src/import/mod.rs`
- Modify: `src-tauri/src/import/x.rs`

- [ ] **Step 1: Create `import/tweet_common.rs`:**

```rust
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
```

- [ ] **Step 2: Register the module** — add to `src-tauri/src/import/mod.rs`: `pub mod tweet_common;`

- [ ] **Step 3: Refactor `import/x.rs` to use it.** Replace the per-row construction in `x::import`'s loop body (the `Work {...}` / `Highlight {...}` building) with:

```rust
        let t = crate::import::tweet_common::TweetInput {
            tweet_id: t.tweet_id.clone(), text: t.text.clone(),
            author_handle: t.author_handle.clone(), author_name: t.author_name.clone(),
            created_at: t.created_at.clone(), url: t.url.clone(),
            images: t.images.clone(), article_urls: t.article_urls.clone(),
            saved_as: t.saved_as.clone(),
            reply_to_id: t.reply_to_id.clone(), parent_text: t.parent_text.clone(),
            parent_handle: t.parent_handle.clone(), quoted_tweet_id: t.quoted_tweet_id.clone(),
            quoted_text: t.quoted_text.clone(), quoted_handle: t.quoted_handle.clone(),
        };
        let (work, highlight, title) = crate::import::tweet_common::make_records(&t, &now);
        let author = work.author.clone();
        if seen.insert(work.id.clone()) { works.push(work); }
        highlights.push((highlight, title, author));
```

(Keep `x.rs`'s `SavedTweet` struct, JSONL line parsing, blank/malformed skipping, and its own tests. Delete the now-unused `truncate_title`/`handle_label`/`body_with_context` from `x.rs`.)

- [ ] **Step 4: Run tests**

Run: `cd src-tauri && cargo test --lib import::tweet_common && cargo test --lib import::x`
Expected: PASS (both the new helper test and x.rs's existing 3 tests)

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/import/tweet_common.rs src-tauri/src/import/mod.rs src-tauri/src/import/x.rs
git commit -m "refactor(import): shared tweet_common::make_records; x.rs uses it"
```

---

### Task 4: Readwise Reader tweet importer (parsing)

**Files:**
- Create: `src-tauri/src/import/readwise_tweets.rs`
- Modify: `src-tauri/src/import/mod.rs`

- [ ] **Step 1: Create `import/readwise_tweets.rs` with pure parse helpers + tests:**

```rust
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
```

- [ ] **Step 2: Add the API fetch + build function** (same file, below the helpers):

```rust
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

    if highlights.is_empty() { bail!("No Readwise saved tweets found (is the Readwise API key set?)"); }
    Ok((works, highlights))
}
```

- [ ] **Step 3: Register module** — add to `import/mod.rs`: `pub mod readwise_tweets;`

- [ ] **Step 4: Run tests**

Run: `cd src-tauri && cargo test --lib import::readwise_tweets`
Expected: PASS (2 parse tests)

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/import/readwise_tweets.rs src-tauri/src/import/mod.rs
git commit -m "feat(import): Readwise Reader saved-tweets importer"
```

---

### Task 5: `import_readwise_tweets` command + register

**Files:**
- Modify: `src-tauri/src/commands/import.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add the command** in `commands/import.rs` (after `run_import`). It reads the key + cursor, runs the importer, persists, and stores the new cursor:

```rust
#[tauri::command]
pub async fn import_readwise_tweets(
    state: tauri::State<'_, AppState>,
    window: tauri::WebviewWindow,
) -> Result<ImportStatus, String> {
    let started = std::time::Instant::now();
    let result = async {
        let cfg = state.config();
        if cfg.readwise_api_key.is_empty() {
            return Err("No Readwise API key configured. Open Settings (⌘,).".to_string());
        }
        let after = cfg.readwise_tweets_last_sync.clone();
        let updated_after = if after.is_empty() { None } else { Some(after.as_str()) };
        let sync_start = chrono::Utc::now().to_rfc3339();
        progress(&window, "Importing saved tweets from Readwise…", 0, 0);
        let (works, h) = crate::import::readwise_tweets::import(&cfg.readwise_api_key, updated_after)
            .await.map_err(|e| e.to_string())?;
        let status = persist(&state, "x", &works, &h, None, &window)?;
        if let Ok(mut c) = state.config.write() {
            c.readwise_tweets_last_sync = sync_start;
            let _ = crate::config::save(&c);
        }
        Ok::<ImportStatus, String>(status)
    }
    .await;
    log_outcome("readwise-tweets", started, &result);
    result
}
```

- [ ] **Step 2: Register** in `lib.rs` `invoke_handler!` list, after `commands::import::import_x`:

```rust
            commands::import::import_readwise_tweets,
```

- [ ] **Step 3: Build to verify it compiles**

Run: `cd src-tauri && cargo build`
Expected: Finished (no errors)

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/import.rs src-tauri/src/lib.rs
git commit -m "feat(commands): import_readwise_tweets command"
```

---

### Task 6: Sync registry + is-due logic

**Files:**
- Create: `src-tauri/src/sync/mod.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod sync;`)

- [ ] **Step 1: Create `sync/mod.rs` with the registry, a pure `is_due`, and the dispatcher:**

```rust
// Schedulable-source registry + the "is this source due?" rule. The scheduler
// (lib.rs) and the Settings Sync tab both drive off SCHEDULABLE.

use chrono::{DateTime, Utc};

use crate::config::Config;
use crate::models::ImportStatus;
use crate::AppState;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SyncSourceId { ReadwiseHighlights, ReadwiseTweets, Zotero }

pub const SCHEDULABLE: [SyncSourceId; 3] =
    [SyncSourceId::ReadwiseHighlights, SyncSourceId::ReadwiseTweets, SyncSourceId::Zotero];

impl SyncSourceId {
    pub fn key(self) -> &'static str {
        match self { Self::ReadwiseHighlights => "readwise", Self::ReadwiseTweets => "readwise_tweets", Self::Zotero => "zotero" }
    }
    pub fn enabled(self, c: &Config) -> bool {
        match self { Self::ReadwiseHighlights => c.readwise_sync_enabled, Self::ReadwiseTweets => c.readwise_tweets_sync_enabled, Self::Zotero => c.zotero_sync_enabled }
    }
    pub fn interval_hours(self, c: &Config) -> u32 {
        match self { Self::ReadwiseHighlights => c.readwise_sync_interval_hours, Self::ReadwiseTweets => c.readwise_tweets_sync_interval_hours, Self::Zotero => c.zotero_sync_interval_hours }
    }
    pub fn last_sync(self, c: &Config) -> &str {
        match self { Self::ReadwiseHighlights => &c.readwise_last_sync, Self::ReadwiseTweets => &c.readwise_tweets_last_sync, Self::Zotero => &c.zotero_last_sync }
    }
}

/// Pure rule: a source is due if enabled, interval>0, and (no last_sync, or
/// now - last_sync >= interval hours).
pub fn is_due(id: SyncSourceId, c: &Config, now: DateTime<Utc>) -> bool {
    if !id.enabled(c) || id.interval_hours(c) == 0 { return false; }
    let last = id.last_sync(c);
    if last.is_empty() { return true; }
    match DateTime::parse_from_rfc3339(last) {
        Ok(t) => now.signed_duration_since(t.with_timezone(&Utc)).num_minutes() >= (id.interval_hours(c) as i64) * 60,
        Err(_) => true,
    }
}

/// Run one source by reusing the existing command internals. Each updates its
/// own cursor on success (the underlying commands already do for readwise /
/// readwise_tweets; zotero is full each run so no cursor is needed).
pub async fn run_source(
    id: SyncSourceId,
    state: tauri::State<'_, AppState>,
    window: tauri::WebviewWindow,
) -> Result<ImportStatus, String> {
    match id {
        SyncSourceId::ReadwiseHighlights => crate::commands::import::run_import(state, window).await,
        SyncSourceId::ReadwiseTweets => crate::commands::import::import_readwise_tweets(state, window).await,
        SyncSourceId::Zotero => crate::commands::import::run_zotero_import(state, window).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn cfg(enabled: bool, hours: u32, last: &str) -> Config {
        let mut c = Config::default();
        c.readwise_tweets_sync_enabled = enabled;
        c.readwise_tweets_sync_interval_hours = hours;
        c.readwise_tweets_last_sync = last.to_string();
        c
    }

    #[test]
    fn due_logic() {
        let now = Utc::now();
        let id = SyncSourceId::ReadwiseTweets;
        assert!(!is_due(id, &cfg(false, 6, ""), now), "disabled → not due");
        assert!(!is_due(id, &cfg(true, 0, ""), now), "interval 0 → not due");
        assert!(is_due(id, &cfg(true, 6, ""), now), "enabled, never synced → due");
        let recent = (now - Duration::hours(2)).to_rfc3339();
        assert!(!is_due(id, &cfg(true, 6, &recent), now), "synced 2h ago, 6h interval → not due");
        let old = (now - Duration::hours(7)).to_rfc3339();
        assert!(is_due(id, &cfg(true, 6, &old), now), "synced 7h ago, 6h interval → due");
    }
}
```

- [ ] **Step 2: Make the dispatched commands reachable.** The commands (`run_import`, `import_readwise_tweets`, `run_zotero_import`) are `pub async fn` in `commands/import.rs` — confirm they're `pub`. They are. Add `mod sync;` to `lib.rs` (near the other `mod` declarations).

- [ ] **Step 3: Run the test**

Run: `cd src-tauri && cargo test --lib sync::tests::due_logic`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/sync/mod.rs src-tauri/src/lib.rs
git commit -m "feat(sync): schedulable-source registry + is_due rule"
```

---

### Task 7: In-app scheduler + is-syncing guard

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add an `is_syncing` guard to `AppState`** (so scheduled + manual runs never overlap). In `lib.rs`:

```rust
pub struct AppState {
    pub db: Mutex<Connection>,
    pub config: RwLock<config::Config>,
    pub is_syncing: std::sync::atomic::AtomicBool,
}
```

Initialize it in the `.manage(AppState { ... })` call: `is_syncing: std::sync::atomic::AtomicBool::new(false),`.

- [ ] **Step 2: Spawn the scheduler** inside `setup(move |app| { ... })`, after the window is shown:

```rust
            // In-app sync scheduler: tick every 5 min, run due sources sequentially.
            let sched_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut tick = tokio::time::interval(std::time::Duration::from_secs(300));
                loop {
                    tick.tick().await;
                    let Some(window) = sched_handle.get_webview_window("main") else { continue };
                    let state = sched_handle.state::<AppState>();
                    if state.is_syncing.load(std::sync::atomic::Ordering::SeqCst) { continue; }
                    let cfg = state.config();
                    let now = chrono::Utc::now();
                    for id in crate::sync::SCHEDULABLE {
                        if crate::sync::is_due(id, &cfg, now) {
                            state.is_syncing.store(true, std::sync::atomic::Ordering::SeqCst);
                            let _ = crate::sync::run_source(id, sched_handle.state::<AppState>(), window.clone()).await;
                            state.is_syncing.store(false, std::sync::atomic::Ordering::SeqCst);
                        }
                    }
                }
            });
```

- [ ] **Step 3: Guard manual imports too.** In `commands/import.rs` `persist()` is shared by all importers, but the simplest guard is in the scheduler (above) — manual imports set/clear the flag via a thin wrapper is overkill for v1. Leave manual imports ungated; the scheduler skips when a previous *scheduled* run is mid-flight, and `persist` holds the db mutex so writes are serialized regardless. (Document this; no code needed.)

- [ ] **Step 4: Build**

Run: `cd src-tauri && cargo build`
Expected: Finished

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(sync): in-app scheduler ticking due sources"
```

---

### Task 8: Autostart from config (user-controlled)

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Replace the forced-autostart block.** In `setup()`, the current block force-enables autostart. Replace it with config-driven enable/disable:

```rust
            // Launch-at-login follows the user's setting (default off for new installs).
            use tauri_plugin_autostart::ManagerExt;
            let autostart = app.autolaunch();
            let want = { app.state::<AppState>().config().autostart_enabled };
            let is_on = autostart.is_enabled().unwrap_or(false);
            if want && !is_on { let _ = autostart.enable(); }
            if !want && is_on { let _ = autostart.disable(); }
```

- [ ] **Step 2: Apply on settings save.** In `commands/settings.rs` `save_settings`, after writing config, reconcile autostart isn't available there (no `app` handle). Instead, add a dedicated command in `lib.rs`/`commands/settings.rs`:

```rust
#[tauri::command]
pub async fn set_autostart(enabled: bool, app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let a = app.autolaunch();
    if enabled { a.enable().map_err(|e| e.to_string()) } else { a.disable().map_err(|e| e.to_string()) }
}
```

Register `commands::settings::set_autostart` in `lib.rs` `invoke_handler!`. (The frontend calls this when the toggle changes, in addition to saving the config flag.)

- [ ] **Step 3: Build**

Run: `cd src-tauri && cargo build`
Expected: Finished

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/commands/settings.rs
git commit -m "feat(sync): user-controlled autostart (set_autostart command)"
```

---

### Task 9: Settings backend round-trips new fields

**Files:**
- Modify: `src-tauri/src/commands/settings.rs`

- [ ] **Step 1: Add fields to the `Settings` struct:**

```rust
    pub readwise_sync_enabled: bool,
    pub readwise_sync_interval_hours: u32,
    pub readwise_tweets_sync_enabled: bool,
    pub readwise_tweets_sync_interval_hours: u32,
    pub zotero_sync_enabled: bool,
    pub zotero_sync_interval_hours: u32,
    pub autostart_enabled: bool,
```

- [ ] **Step 2: Populate them in `get_settings`** (read from `c.*`), and **carry them in `save_settings`** when building `new_config` (copy from `settings.*`, preserving the existing `*_last_sync` cursors the same way `readwise_last_sync` is preserved — i.e., read the current cursors before overwriting and copy them into `new_config`).

```rust
    // in save_settings, after reading last_sync, also read existing cursors:
    let (rw_tweets_cursor, zo_cursor) = {
        let cur = state.config.read().map_err(|e| e.to_string())?;
        (cur.readwise_tweets_last_sync.clone(), cur.zotero_last_sync.clone())
    };
    // ...in new_config:
    readwise_tweets_last_sync: rw_tweets_cursor,
    zotero_last_sync: zo_cursor,
    readwise_sync_enabled: settings.readwise_sync_enabled,
    readwise_sync_interval_hours: settings.readwise_sync_interval_hours,
    readwise_tweets_sync_enabled: settings.readwise_tweets_sync_enabled,
    readwise_tweets_sync_interval_hours: settings.readwise_tweets_sync_interval_hours,
    zotero_sync_enabled: settings.zotero_sync_enabled,
    zotero_sync_interval_hours: settings.zotero_sync_interval_hours,
    autostart_enabled: settings.autostart_enabled,
```

- [ ] **Step 3: Build**

Run: `cd src-tauri && cargo build`
Expected: Finished

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/settings.rs
git commit -m "feat(settings): round-trip sync schedule + autostart fields"
```

---

### Task 10: Frontend types + API

**Files:**
- Modify: `src/types.ts`
- Modify: `src/lib/api.ts`

- [ ] **Step 1: Extend the `Settings` interface** in `types.ts`:

```ts
  readwise_sync_enabled: boolean;
  readwise_sync_interval_hours: number;
  readwise_tweets_sync_enabled: boolean;
  readwise_tweets_sync_interval_hours: number;
  zotero_sync_enabled: boolean;
  zotero_sync_interval_hours: number;
  autostart_enabled: boolean;
```

- [ ] **Step 2: Add API functions** in `api.ts`:

```ts
export async function importReadwiseTweets(): Promise<ImportStatus> {
  return invoke<ImportStatus>("import_readwise_tweets");
}

export async function setAutostart(enabled: boolean): Promise<void> {
  return invoke<void>("set_autostart", { enabled });
}
```

- [ ] **Step 3: Commit**

```bash
git add src/types.ts src/lib/api.ts
git commit -m "feat(ui): Settings sync types + importReadwiseTweets/setAutostart api"
```

---

### Task 11: Import-menu entry for Readwise tweets

**Files:**
- Modify: `src/components/Toolbar.tsx`
- Modify: `src/components/ImportMenu.tsx`
- Modify: `src/App.tsx`

- [ ] **Step 1: Add to the `ImportAction` union** in `Toolbar.tsx`: add `| "readwise-tweets"` (after `"readwise-seed"`).

- [ ] **Step 2: Add a menu item** in `ImportMenu.tsx` under "Connected sources":

```tsx
      { action: "readwise-tweets", label: "Readwise saved tweets", hint: "full text via Reader" },
```

- [ ] **Step 3: Handle it** in `App.tsx` `doImport`. Find the `readwise` case (it calls `withImport(... runImport)`) and add alongside it:

```tsx
    if (which === "readwise-tweets") {
      await withImport("Importing saved tweets from Readwise…", () => importReadwiseTweets());
      return;
    }
```

Add `importReadwiseTweets` to the `from "./lib/api"` import block.

- [ ] **Step 4: Build the frontend**

Run: `bun run build`
Expected: `tsc` + vite succeed, no type errors

- [ ] **Step 5: Commit**

```bash
git add src/components/Toolbar.tsx src/components/ImportMenu.tsx src/App.tsx
git commit -m "feat(ui): Readwise saved tweets import menu entry"
```

---

### Task 12: Settings "Sync" tab

**Files:**
- Modify: `src/components/SettingsPanel.tsx`

- [ ] **Step 1: Add "sync" to the `Tab` type and the `tabs` array** (label "Sync", place after "import"):

```tsx
type Tab = "import" | "sync" | "sources" | "view" | "shortcuts" | "about";
// in tabs: { id: "sync", label: "Sync" },
```

- [ ] **Step 2: Add the tab body** (inside the settings-loaded block, guarded by `settings &&`). A reusable row + the autostart toggle:

```tsx
          {tab === "sync" && settings && (
            <>
              <p className="text-xs text-zinc-400">Run imports automatically while Highlight Scout is open. Enable “Launch at login” to keep it running.</p>
              {([
                ["Readwise highlights", "readwise_sync_enabled", "readwise_sync_interval_hours"],
                ["Readwise saved tweets", "readwise_tweets_sync_enabled", "readwise_tweets_sync_interval_hours"],
                ["Zotero", "zotero_sync_enabled", "zotero_sync_interval_hours"],
              ] as const).map(([name, enKey, ivKey]) => (
                <div key={enKey} className="flex items-center justify-between gap-2 border-b border-zinc-100 py-2">
                  <label className="flex items-center gap-2 text-sm text-zinc-700">
                    <input type="checkbox" checked={Boolean(settings[enKey])} onChange={(e) => update({ [enKey]: e.target.checked } as Partial<Settings>)} />
                    {name}
                  </label>
                  <select className="rounded border border-zinc-200 px-2 py-1 text-xs" value={Number(settings[ivKey]) || 0}
                    onChange={(e) => update({ [ivKey]: Number(e.target.value) } as Partial<Settings>)}>
                    <option value={0}>Off</option>
                    <option value={1}>Hourly</option>
                    <option value={6}>Every 6 hours</option>
                    <option value={24}>Daily</option>
                  </select>
                </div>
              ))}
              <label className="mt-2 flex items-center gap-2 text-sm text-zinc-700">
                <input type="checkbox" checked={settings.autostart_enabled}
                  onChange={(e) => { update({ autostart_enabled: e.target.checked }); setAutostart(e.target.checked).catch(() => {}); }} />
                Launch at login (enables background syncs)
              </label>
            </>
          )}
```

- [ ] **Step 3: Show the Save button on the sync tab.** The footer currently renders Save for `tab === "sources" || tab === "view"`. Add `|| tab === "sync"`.

- [ ] **Step 4: Import `setAutostart`** in SettingsPanel from `../lib/api`.

- [ ] **Step 5: Build**

Run: `bun run build`
Expected: succeeds, no type errors

- [ ] **Step 6: Commit**

```bash
git add src/components/SettingsPanel.tsx
git commit -m "feat(ui): Settings Sync tab (per-source schedule + autostart)"
```

---

### Task 13: Full verification

- [ ] **Step 1: Backend tests + build**

Run: `cd src-tauri && cargo test --lib && cargo build`
Expected: all tests PASS (config round-trip, tweet_common, x, readwise_tweets parse, sync due_logic), build Finished.

- [ ] **Step 2: Frontend build**

Run: `bun run build`
Expected: succeeds.

- [ ] **Step 3: Manual smoke test (debug app)**

Run: `cd src-tauri && cargo run` (or `bun run tauri dev`). Then:
- Import ▾ → "Readwise saved tweets" → confirm tweets import (status toast, counts), with full text + images in results.
- Settings → Sync → enable "Readwise saved tweets" = Hourly, toggle "Launch at login" on; Save. Confirm config.toml has the fields and the launch agent is registered.
- Confirm a scheduled run fires (set interval, watch the Import Log).

- [ ] **Step 4: Final commit (release notes already in Task 1) + push branch**

```bash
git push -u origin scheduled-syncs
```

---

## Notes for the implementer

- **Patterns to follow:** every importer returns `(Vec<Work>, Vec<(Highlight, String, Option<String>)>)` and is persisted via `persist()` in `commands/import.rs`. Don't re-implement persistence.
- **Dedup invariant:** all tweet highlights are keyed `x-{tweet_id}` / works `x-w-{tweet_id}`, `source_system="x"`. Re-importing the same tweet upserts (no duplicates). Do not change this keying.
- **No `--no-verify`**, never force-push, agent `Co-Authored-By` trailer on commits.
- The Reader importer's `created_at` is intentionally null (Reader gives save-date, not post-date); the year/date for a tweet comes from a birdclaw record if the same tweet is also imported from there. This is acceptable per the spec.
- **Known hard spot (Task 6/7):** calling the `#[tauri::command]` fns (`run_import`, `import_readwise_tweets`, `run_zotero_import`) from the spawned scheduler holds a `tauri::State` across `.await`, which can fail to compile (State guard lifetime). If `cargo build` complains: snapshot the config synchronously from `app_handle.state::<AppState>()` before awaiting, and pass the already-resolved `State`/`WebviewWindow` into `run_source` without re-borrowing across iterations — or, cleanest, extract each importer's core into a plain `async fn(&AppState, &Window)` that both the command and `run_source` call. Keep the change minimal and re-run `cargo build`.
