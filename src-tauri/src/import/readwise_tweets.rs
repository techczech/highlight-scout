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

/// Collect distinct /media/ image URLs from html (for source_data metadata).
pub fn media_images(html: &str) -> Vec<String> {
    let dom = match tl::parse(html, tl::ParserOptions::default()) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let parser = dom.parser();
    let mut out: Vec<String> = Vec::new();
    for img in dom.query_selector("img").into_iter().flatten() {
        if let Some(tag) = img.get(parser).and_then(|n| n.as_tag()) {
            if let Some(Some(src)) = tag.attributes().get("src") {
                let s = src.as_utf8_str().to_string();
                if s.contains("/media/") && !out.contains(&s) {
                    out.push(s);
                }
            }
        }
    }
    out
}

fn html_unescape(s: &str) -> String {
    s.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">")
     .replace("&quot;", "\"").replace("&#39;", "'").replace("&nbsp;", " ")
}

/// Parse Reader tweet html_content into a complete markdown body:
/// thread `<hr>` → `---`, rw-embedded-tweet `<article>` → `>` blockquote,
/// `/media/` `<img>` → inline `![image](url)`. Pure; never panics.
pub fn parse_tweet_html(html: &str) -> String {
    let dom = match tl::parse(html, tl::ParserOptions::default()) {
        Ok(dom) => dom,
        Err(_) => return html_unescape(html.trim()),
    };
    let parser = dom.parser();
    let mut out = String::new();
    for child in dom.children() {
        if let Some(node) = child.get(parser) {
            walk(node, parser, &mut out);
        }
    }
    normalise(&out)
}

const THREAD_SEP: &str = "\n\n---\n\n";

/// True when a tag carries `class` containing the given member token.
fn has_class(tag: &tl::HTMLTag, member: &str) -> bool {
    tag.attributes().is_class_member(member)
}

/// Returns the `src`/`href`-style attribute value as an owned string.
fn attr(tag: &tl::HTMLTag, key: &str) -> Option<String> {
    tag.attributes()
        .get(key)
        .flatten()
        .map(|b| b.as_utf8_str().to_string())
}

fn tag_name(tag: &tl::HTMLTag) -> String {
    tag.name().as_utf8_str().to_lowercase()
}

/// Walk a top-level (block-context) node, appending markdown to `out`.
fn walk(node: &tl::Node, parser: &tl::Parser, out: &mut String) {
    match node {
        tl::Node::Raw(b) => out.push_str(&html_unescape(&b.as_utf8_str())),
        tl::Node::Comment(_) => {}
        tl::Node::Tag(tag) => {
            let name = tag_name(tag);
            match name.as_str() {
                "hr" => {
                    if has_class(tag, "twitter-thread-delimiter") {
                        out.push_str(THREAD_SEP);
                    }
                }
                "article" => {
                    if has_class(tag, "rw-embedded-tweet") {
                        let bq = render_blockquote(tag, parser);
                        out.push('\n');
                        out.push('\n');
                        out.push_str(&bq);
                        out.push('\n');
                        out.push('\n');
                    }
                }
                "svg" | "figure" => {
                    // figure may wrap an <img>; svg never has media. Recurse figure only.
                    if name == "figure" {
                        for &id in tag.children().top().iter() {
                            if let Some(child) = id.get(parser) {
                                walk(child, parser, out);
                            }
                        }
                    }
                }
                "img" => {
                    if let Some(src) = attr(tag, "src") {
                        if src.contains("/media/") {
                            out.push_str(&format!("![image]({})", src));
                        }
                    }
                }
                "p" => {
                    let inner = inner_markdown(tag, parser);
                    let inner = inner.trim();
                    if !inner.is_empty() {
                        out.push_str(inner);
                    }
                    out.push_str("\n\n");
                }
                _ => {
                    // block wrapper (div, etc.): recurse children in block context.
                    for &id in tag.children().top().iter() {
                        if let Some(child) = id.get(parser) {
                            walk(child, parser, out);
                        }
                    }
                }
            }
        }
    }
}

/// Render the inline (and nested-block) content of a tag to a markdown string.
fn inner_markdown(tag: &tl::HTMLTag, parser: &tl::Parser) -> String {
    let mut s = String::new();
    for &id in tag.children().top().iter() {
        if let Some(node) = id.get(parser) {
            inline_node(node, parser, &mut s);
        }
    }
    s
}

/// Append a single node's content in inline context.
fn inline_node(node: &tl::Node, parser: &tl::Parser, s: &mut String) {
    match node {
        tl::Node::Raw(b) => s.push_str(&html_unescape(&b.as_utf8_str())),
        tl::Node::Comment(_) => {}
        tl::Node::Tag(tag) => {
            let name = tag_name(tag);
            match name.as_str() {
                "em" | "i" => {
                    let inner = inner_markdown(tag, parser);
                    let inner = inner.trim();
                    if !inner.is_empty() {
                        s.push('*');
                        s.push_str(inner);
                        s.push('*');
                    }
                }
                "strong" | "b" => {
                    let inner = inner_markdown(tag, parser);
                    let inner = inner.trim();
                    if !inner.is_empty() {
                        s.push_str("**");
                        s.push_str(inner);
                        s.push_str("**");
                    }
                }
                "br" => s.push('\n'),
                "svg" => {}
                "img" => {
                    if let Some(src) = attr(tag, "src") {
                        if src.contains("/media/") {
                            s.push_str(&format!("![image]({})", src));
                        }
                    }
                }
                "a" => {
                    let href = attr(tag, "href").unwrap_or_default();
                    let text = inner_markdown(tag, parser);
                    let text = text.trim();
                    let is_twitter = href.contains("x.com")
                        || href.contains("twitter.com")
                        || href.contains("t.co")
                        || href.contains("pbs.twimg");
                    if is_twitter {
                        // self/nav link: emit text only, drop bare truncated URLs.
                        if !text.is_empty() && !looks_like_bare_url(text) {
                            s.push_str(text);
                        }
                    } else if !href.is_empty() && !text.is_empty() {
                        s.push_str(&format!("[{}]({})", text, href));
                    } else if !text.is_empty() {
                        s.push_str(text);
                    }
                }
                "p" => {
                    // nested paragraph in inline context → paragraph break around it.
                    let inner = inner_markdown(tag, parser);
                    let inner = inner.trim();
                    if !inner.is_empty() {
                        if !s.is_empty() && !s.ends_with('\n') {
                            s.push_str("\n\n");
                        }
                        s.push_str(inner);
                        s.push_str("\n\n");
                    }
                }
                _ => {
                    // figure, span, div, etc.: recurse children inline.
                    for &id in tag.children().top().iter() {
                        if let Some(child) = id.get(parser) {
                            inline_node(child, parser, s);
                        }
                    }
                }
            }
        }
    }
}

/// True when text looks like a truncated/bare tweet URL (e.g. `x.com/foo…`).
fn looks_like_bare_url(text: &str) -> bool {
    let t = text.trim();
    t.starts_with("x.com/")
        || t.starts_with("twitter.com/")
        || t.starts_with("http://")
        || t.starts_with("https://")
        || t.starts_with("pic.twitter.com/")
        || t.starts_with("t.co/")
}

/// Render an `rw-embedded-tweet` article as a `>`-prefixed blockquote.
fn render_blockquote(article: &tl::HTMLTag, parser: &tl::Parser) -> String {
    let mut sections: Vec<String> = Vec::new();

    // Find header / main / footer children.
    let mut header_md = String::new();
    let mut main_md = String::new();
    let mut footer_md = String::new();

    for &id in article.children().top().iter() {
        if let Some(tl::Node::Tag(tag)) = id.get(parser) {
            match tag_name(tag).as_str() {
                "header" => header_md = render_header(tag, parser),
                "main" => main_md = inner_markdown(tag, parser),
                "footer" => footer_md = render_footer(tag, parser),
                _ => {}
            }
        }
    }

    if !header_md.trim().is_empty() {
        sections.push(header_md.trim().to_string());
    }
    let main_clean = collapse_inner(main_md.trim());
    if !main_clean.is_empty() {
        sections.push(main_clean);
    }
    if !footer_md.trim().is_empty() {
        sections.push(footer_md.trim().to_string());
    }

    let body = sections.join("\n\n");
    // Prefix every line with `> ` (blank lines → `>`).
    let mut quoted = String::new();
    for line in body.lines() {
        if line.trim().is_empty() {
            quoted.push('>');
        } else {
            quoted.push_str("> ");
            quoted.push_str(line);
        }
        quoted.push('\n');
    }
    quoted.trim_end().to_string()
}

/// Header → `**<name>** <@handle>` from the two author `<a>` inner texts.
fn render_header(header: &tl::HTMLTag, parser: &tl::Parser) -> String {
    let mut links: Vec<String> = Vec::new();
    collect_author_links(header, parser, &mut links);
    match (links.first(), links.get(1)) {
        (Some(name), Some(handle)) => format!("**{}** {}", name.trim(), handle.trim()),
        (Some(name), None) => format!("**{}**", name.trim()),
        _ => String::new(),
    }
}

/// Collect inner text of `<a>` descendants whose href is NOT a status link
/// (those are the author name + @handle; the trailing icon link is a status link).
fn collect_author_links(tag: &tl::HTMLTag, parser: &tl::Parser, out: &mut Vec<String>) {
    for &id in tag.children().top().iter() {
        if let Some(tl::Node::Tag(child)) = id.get(parser) {
            if tag_name(child) == "a" {
                let href = attr(child, "href").unwrap_or_default();
                if !href.contains("/status/") {
                    let text = child.inner_text(parser);
                    let text = html_unescape(text.trim());
                    if !text.is_empty() {
                        out.push(text);
                    }
                }
            } else {
                collect_author_links(child, parser, out);
            }
        }
    }
}

/// Footer → `_<link text>_`.
fn render_footer(footer: &tl::HTMLTag, parser: &tl::Parser) -> String {
    let text = footer.inner_text(parser);
    let text = html_unescape(text.trim());
    if text.is_empty() {
        String::new()
    } else {
        // collapse internal whitespace runs to single spaces
        let text: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
        format!("_{}_", text)
    }
}

/// Collapse 3+ newlines to 2 within a segment.
fn collapse_inner(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut nl = 0;
    for ch in s.chars() {
        if ch == '\n' {
            nl += 1;
            if nl <= 2 {
                out.push('\n');
            }
        } else {
            nl = 0;
            out.push(ch);
        }
    }
    out.trim().to_string()
}

/// Split on the thread separator, trim + drop empty segments, re-join, collapse newlines.
fn normalise(raw: &str) -> String {
    let segments: Vec<String> = raw
        .split("\n\n---\n\n")
        .map(|seg| collapse_inner(seg.trim()))
        .filter(|seg| !seg.is_empty())
        .collect();
    segments.join("\n\n---\n\n").trim().to_string()
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

    const THREAD_HTML: &str = r##"<div><p data-rw-toc-level="1" data-rw-toc-title="It's hard to overstate how disappointing academia's reaction to LLMs..."><p>It's hard to overstate how disappointing academia's reaction to LLMs have been. We got scifi level automation in less than a decade and the response has been almost entirely protectionist politics. Do you have any idea how embarrassing that is? Institutions fighting for relevance  </p></p><hr class="twitter-thread-delimiter"/><p data-rw-toc-level="1" data-rw-toc-title="It's now possible to provide every student of any income..."><p>It's now possible to provide every student of any income level and individual learning capabilities a nearly free always on personal tutor in any subject from birth. Something that would have been considered a miracle in any previous decade. Remember one laptop per child?  </p></p><hr class="twitter-thread-delimiter"/><p data-rw-toc-level="1" data-rw-toc-title="The response has been catastrophically embarrassing"><p>The response has been catastrophically embarrassing. It's so profoundly obvious that the mission isn't actually education and that who are supposed to be the state of the art are actually several decades behind luddites. It really and truly is sorting people and institutions.  </p></p><hr class="twitter-thread-delimiter"/><p data-rw-toc-level="1" data-rw-toc-title="Let me put it this way"><p>Let me put it this way</p><p>-You can be the cutting edge and not care about teaching</p><p>-Or you can care only about teaching</p><p>But you can't both be out of date and also not excited about more ways to teach. There's no self referential luddites box.  </p></p><hr class="twitter-thread-delimiter"/><p data-rw-toc-level="1" data-rw-toc-title="Also it's deeply funny watching an industry that spent a..."><p>Also it's deeply funny watching an industry that spent a decade trashing protectionism do a 180 the second their industry was threatened. Globalization is obviously good unless it's against clankers then suddenly it's lifetime bans for a single hallucinated cite.  </p></p><hr class="twitter-thread-delimiter"/><p data-rw-toc-level="1" data-rw-toc-title="Tweet by RexDouglass"><p><a href="https://x.com/RexDouglass/status/2067695321740640530" rel="nofollow">x.com/RexDouglass/st…</a> </p></p></div>"##;

    const QUOTE_HTML: &str = r##"<div><p data-rw-toc-level="1" data-rw-toc-title="This is a super exciting release..."><p>This is a super exciting release - Claude Fable 5 is the same underlying model as Mythos but with added safeguards. The benchmarks are great and it's SOTA on everything by a margin but I'll add that <em>qualitatively</em> also, this is a major-version-bump-deserving step change forward. Really looking forward to all the things people build!  </p></p>
<article class="rw-embedded-tweet" data-rw-tweet-id="2064394151441863006">
<header class="rw-embedded-tweet-header">
<div>
<figure><img src="https://pbs.twimg.com/profile_images/1950950107937185792/QOfEjFoJ.jpg"/></figure>
</div>
<div>
<span><a href="https://twitter.com/claudeai">Claude</a></span>
<span><a href="https://twitter.com/claudeai">@claudeai</a></span>
</div>
<div>
<a href="https://twitter.com/claudeai/status/2064394151441863006">
<svg aria-label="X" fill="none" height="24" viewbox="0 0 24 24" width="24" xmlns="http://www.w3.org/2000/svg"></svg>
</a>
</div>
</header>
<main>
<p><p>Fable 5 is state-of-the-art on nearly all tested benchmarks, with exceptional performance in software engineering, knowledge work, scientific research, and vision.</p><p>The longer and more complex the task, the larger Fable 5's lead over our other models.  </p><p><figure><img alt="Image" src="https://pbs.twimg.com/media/HKYwNlEWMAAJanX.png?name=orig"/></figure> </p></p>
</main>
<footer class="rw-embedded-tweet-footer" data-rw-created-timestamp="1781024894000">
<span>
<a href="https://twitter.com/claudeai/status/2064394151441863006">Posted Jun 9, 2026 at 5:08PM</a>
</span>
</footer>
</article></div>"##;

    #[test]
    fn thread_html_uses_hr_separators() {
        let md = parse_tweet_html(THREAD_HTML);
        // 5 content tweets → 4 separators; trailing self-link block dropped.
        assert_eq!(md.matches("\n---\n").count(), 4, "got:\n{md}");
        assert!(!md.trim_start().starts_with("---"));
        assert!(!md.trim_end().ends_with("---"));
        assert!(md.contains("It's hard to overstate how disappointing academia"));
        assert!(md.contains("Remember one laptop per child?"));
        assert!(md.contains("Also it's deeply funny watching an industry"));
        // the final self-link-only tweet is dropped
        assert!(!md.contains("x.com/RexDouglass/st"));
    }

    #[test]
    fn quoted_tweet_becomes_blockquote_with_inline_image_and_date() {
        let md = parse_tweet_html(QUOTE_HTML);
        // main tweet text is present and NOT quoted
        assert!(md.contains("This is a super exciting release - Claude Fable 5"));
        assert!(md.contains("*qualitatively*"), "em → *italic*; got:\n{md}");
        // quoted tweet is a blockquote with author + handle
        assert!(md.contains("> **Claude** @claudeai"), "got:\n{md}");
        assert!(md.contains("> Fable 5 is state-of-the-art on nearly all tested benchmarks"));
        // quoted image inline, inside the quote, /media/ only
        assert!(md.contains("> ![image](https://pbs.twimg.com/media/HKYwNlEWMAAJanX.png?name=orig)"));
        // avatar (profile_images) excluded everywhere
        assert!(!md.contains("profile_images"));
        // footer date as a muted line inside the quote
        assert!(md.contains("> _Posted Jun 9, 2026 at 5:08PM_"));
    }

    #[test]
    fn tweet_body_with_triple_dash_is_not_split_into_a_thread() {
        // a SINGLE tweet (no <hr>) whose text contains --- must NOT gain a thread break
        let md = parse_tweet_html("<div><p>before --- after</p></div>");
        assert!(!md.contains("\n---\n"), "no fabricated thread separator; got:\n{md}");
        assert!(
            md.contains("before --- after") || md.contains("before") && md.contains("after"),
            "got:\n{md}"
        );
    }

    #[test]
    fn falls_back_gracefully_on_plain_html() {
        let md = parse_tweet_html("<p>just a plain tweet</p>");
        assert_eq!(md.trim(), "just a plain tweet");
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

            let body = match d.html_content.as_deref() {
                Some(h) if !h.trim().is_empty() => parse_tweet_html(h),
                _ => String::new(),
            };
            let body = if body.trim().is_empty() {
                d.title.clone().unwrap_or_default()
            } else {
                body
            };
            if body.trim().is_empty() { continue; }

            let imgs = match d.html_content.as_deref() {
                Some(h) => media_images(h),
                None => Vec::new(),
            };

            let t = TweetInput {
                tweet_id: id.clone(),
                text: d.title.clone().unwrap_or_default(),
                body_markdown: Some(body),
                author_handle: handle(su),
                author_name: d.author.clone(),
                created_at: None,
                url: Some(format!("https://x.com/{}/status/{}", handle(su).unwrap_or_default(), id)),
                images: imgs,
                saved_as: Some("likes".into()),
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
