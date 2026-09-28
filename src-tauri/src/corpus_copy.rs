//! What the quick finder shows and copies for one archive passage.
//!
//! The citation text is the engine's (`scout cite`): this module adds no
//! citation wording. It only
//! - renders the engine's Markdown citation for the other paste targets:
//!   HTML with `archive` / `public` as link words ([`citation_html`]), and
//!   plain text that keeps only the public URL, in angle brackets, and never
//!   a local `writeflex://` / `file://` link ([`citation_plain`]),
//! - reads the paragraph before a writing passage for the reading pane
//!   ([`context_before`]), from the source file the engine names,
//! - counts the highlights corpus's works per source system for the rail
//!   ([`source_counts`]), from the engine's index, read-only.
//!
//! Sources and indexes are only read.

use regex::Regex;
use rusqlite::{Connection, OpenFlags};
use scout_corpus::{CitedPassage, CorpusKind, DocumentUnit, Engine};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::OnceLock;

use crate::corpus::{self, Answer, CorpusError};

/// One passage as the reading pane shows it and ⌘⇧C copies it.
#[derive(Debug, Clone, Serialize)]
pub struct PassageView {
    /// `scout cite`: the whole passage, original text, with its citation.
    pub cited: CitedPassage,
    /// `cited.citation.markdown` as HTML, for rich-text paste targets.
    pub html: String,
    /// `cited.citation.markdown` as plain text, for targets that take no
    /// formatting: the public URL only, never a local link.
    pub plain: String,
    /// The paragraph before the passage (writing only), for context.
    pub context_before: Option<String>,
}

/// Cite a passage and prepare it for the reading pane and the clipboard.
pub fn passage(engine: &Engine, passage_id: &str) -> Result<Answer<PassageView>, CorpusError> {
    let a = corpus::cite(engine, passage_id)?;
    let cited = a.body;
    let prose = engine
        .registry()
        .get(&cited.corpus)
        .map(|c| c.kind == CorpusKind::MarkdownFolder && c.document_unit == DocumentUnit::File)
        .unwrap_or(false);
    let context_before = if prose {
        std::fs::read_to_string(&cited.path)
            .ok()
            .and_then(|t| context_before(&t, cited.line_start))
    } else {
        None
    };
    Ok(Answer {
        body: PassageView {
            html: citation_html(&cited.citation.markdown),
            plain: citation_plain(&cited.citation.markdown),
            context_before,
            cited,
        },
        notes: a.notes,
    })
}

const CONTEXT_MAX: usize = 700;

/// The non-blank paragraph that ends just before 1-based `line_start`,
/// skipping frontmatter and headings. `None` when there is none.
pub fn context_before(text: &str, line_start: usize) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    // Lines inside a leading frontmatter block never count as context.
    let body_from = if lines.first().map(|l| l.trim_end()) == Some("---") {
        lines
            .iter()
            .skip(1)
            .position(|l| l.trim_end() == "---")
            .map(|i| i + 2)
            .unwrap_or(lines.len())
    } else {
        0
    };
    let mut end = line_start.saturating_sub(1).min(lines.len());
    while end > body_from && lines[end - 1].trim().is_empty() {
        end -= 1;
    }
    let mut start = end;
    while start > body_from && !lines[start - 1].trim().is_empty() {
        start -= 1;
    }
    if start == end {
        return None;
    }
    let para: Vec<&str> = lines[start..end].iter().map(|l| l.trim()).collect();
    if para.iter().all(|l| l.starts_with('#')) {
        return None;
    }
    let joined = para.join(" ");
    let count = joined.chars().count();
    Some(if count > CONTEXT_MAX {
        let tail: String = joined.chars().skip(count - CONTEXT_MAX).collect();
        format!("…{}", tail.trim_start())
    } else {
        joined
    })
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

fn link_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\[([^\]\n]+)\]\(([^)\s]+)\)").unwrap())
}

fn strong_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\*\*([^*\n]+)\*\*").unwrap())
}

fn em_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\*([^*\n]+)\*").unwrap())
}

/// Links the rich copy keeps as links; anything else stays text.
fn safe_href(url: &str) -> bool {
    let l = url.to_ascii_lowercase();
    ["https://", "http://", "writeflex://", "file://", "zotero://", "mailto:"]
        .iter()
        .any(|p| l.starts_with(p))
}

fn emphasis(escaped: &str) -> String {
    let s = strong_re().replace_all(escaped, "<strong>$1</strong>");
    em_re().replace_all(&s, "<em>$1</em>").into_owned()
}

/// Inline Markdown (links, `**strong**`, `*em*`) to HTML; everything else
/// is escaped text.
fn inline(md: &str) -> String {
    let mut out = String::new();
    let mut last = 0;
    for m in link_re().captures_iter(md) {
        let whole = m.get(0).unwrap();
        out.push_str(&emphasis(&esc(&md[last..whole.start()])));
        let (text, url) = (&m[1], &m[2]);
        if safe_href(url) {
            out.push_str(&format!(
                "<a href=\"{}\">{}</a>",
                esc(url),
                emphasis(&esc(text))
            ));
        } else {
            out.push_str(&emphasis(&esc(whole.as_str())));
        }
        last = whole.end();
    }
    out.push_str(&emphasis(&esc(&md[last..])));
    out
}

/// The engine's Markdown citation (`> quote` lines, a blank line, then the
/// `— Author, *Title*, date · [archive](…) · [public](…)` line) as HTML: a
/// blockquote of paragraphs, then the attribution paragraph.
pub fn citation_html(markdown: &str) -> String {
    let mut out = String::new();
    let mut quote: Vec<Vec<&str>> = vec![];
    let mut para: Vec<&str> = vec![];
    let flush_quote = |quote: &mut Vec<Vec<&str>>, out: &mut String| {
        let paras: Vec<String> = quote
            .drain(..)
            .filter(|p| !p.is_empty())
            .map(|p| {
                let lines: Vec<String> = p.iter().map(|l| inline(l)).collect();
                format!("<p>{}</p>", lines.join("<br>"))
            })
            .collect();
        if !paras.is_empty() {
            out.push_str("<blockquote>");
            out.push_str(&paras.concat());
            out.push_str("</blockquote>");
        }
    };
    let flush_para = |para: &mut Vec<&str>, out: &mut String| {
        if !para.is_empty() {
            let lines: Vec<String> = para.drain(..).map(inline).collect();
            out.push_str(&format!("<p>{}</p>", lines.join("<br>")));
        }
    };
    for line in markdown.lines() {
        if let Some(rest) = line.strip_prefix('>') {
            flush_para(&mut para, &mut out);
            let rest = rest.strip_prefix(' ').unwrap_or(rest);
            if quote.is_empty() {
                quote.push(vec![]);
            }
            if rest.trim().is_empty() {
                quote.push(vec![]);
            } else {
                quote.last_mut().unwrap().push(rest);
            }
        } else if line.trim().is_empty() {
            flush_quote(&mut quote, &mut out);
            flush_para(&mut para, &mut out);
        } else {
            flush_quote(&mut quote, &mut out);
            para.push(line);
        }
    }
    flush_quote(&mut quote, &mut out);
    flush_para(&mut para, &mut out);
    out
}

/// A link the plain copy keeps: a public web page. Local links (`writeflex://`,
/// `file://`, `zotero://`) mean nothing outside this Mac and are dropped.
fn public_href(url: &str) -> bool {
    let l = url.to_ascii_lowercase();
    l.starts_with("https://") || l.starts_with("http://")
}

fn strip_emphasis(s: &str) -> String {
    let s = strong_re().replace_all(s, "$1");
    em_re().replace_all(&s, "$1").into_owned()
}

/// One attribution line as plain text: emphasis dropped; each ` · `-separated
/// link part becomes `<url>` when public and disappears otherwise.
fn plain_attribution(line: &str) -> String {
    let parts: Vec<String> = line
        .split(" · ")
        .filter_map(|part| {
            let mut out = String::new();
            let mut last = 0;
            let mut links = 0;
            let mut kept = 0;
            for m in link_re().captures_iter(part) {
                let whole = m.get(0).unwrap();
                out.push_str(&strip_emphasis(&part[last..whole.start()]));
                links += 1;
                if public_href(&m[2]) {
                    out.push_str(&format!("<{}>", &m[2]));
                    kept += 1;
                }
                last = whole.end();
            }
            out.push_str(&strip_emphasis(&part[last..]));
            let out = out.trim().to_string();
            // A part that was only local links is dropped with its separator.
            (!(links > 0 && kept == 0 && out.is_empty())).then_some(out)
        })
        .filter(|p| !p.is_empty())
        .collect();
    parts.join(" · ")
}

/// The engine's Markdown citation as plain text: `“quote”`, then the
/// attribution with only the public URL, as `<https://…>`, or no link at all.
/// The quote is the original text, as the engine quoted it.
pub fn citation_plain(markdown: &str) -> String {
    let mut quote: Vec<&str> = vec![];
    let mut attribution: Vec<String> = vec![];
    for line in markdown.lines() {
        if let Some(rest) = line.strip_prefix('>') {
            quote.push(rest.strip_prefix(' ').unwrap_or(rest));
        } else if !line.trim().is_empty() {
            attribution.push(plain_attribution(line));
        }
    }
    while quote.last().is_some_and(|l| l.trim().is_empty()) {
        quote.pop();
    }
    let mut out = String::new();
    if !quote.is_empty() {
        out.push('“');
        out.push_str(&quote.join("\n"));
        out.push_str("”\n");
    }
    for a in attribution {
        out.push_str(&a);
        out.push('\n');
    }
    out
}

/// Works per source system (`readwise`, `x`, `zotero`) in a highlights
/// corpus index, opened read-only. Empty when the index is missing or its
/// schema differs.
pub fn source_counts(index_path: &Path) -> BTreeMap<String, u64> {
    let run = || -> rusqlite::Result<BTreeMap<String, u64>> {
        let conn = Connection::open_with_flags(
            index_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        let mut st = conn.prepare(
            "SELECT source, COUNT(*) FROM documents WHERE source IS NOT NULL AND source != '' GROUP BY source",
        )?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        let mut out = BTreeMap::new();
        for r in rows {
            let (s, n) = r?;
            out.insert(s, n.max(0) as u64);
        }
        Ok(out)
    };
    if !index_path.exists() {
        return BTreeMap::new();
    }
    run().unwrap_or_default()
}

/// One corpus in the rail: its size in documents (pieces, tweets, works).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CorpusCount {
    pub corpus: String,
    pub docs: u64,
    pub indexed: bool,
    /// Per source system, for a highlights archive; empty otherwise.
    pub sources: BTreeMap<String, u64>,
}

/// The rail's counts: every registry corpus with its indexed document count
/// (the `index status` figure, so it agrees with the CLI) and, for a
/// highlights archive, its works per source.
pub fn counts(engine: &Engine) -> Result<Vec<CorpusCount>, CorpusError> {
    let st = corpus::status(engine)?.body;
    Ok(st
        .corpora
        .iter()
        .map(|s| {
            let highlights = engine
                .registry()
                .get(&s.corpus)
                .map(|c| c.kind == CorpusKind::HighlightScoutArchive)
                .unwrap_or(false);
            CorpusCount {
                corpus: s.corpus.clone(),
                docs: s.docs as u64,
                indexed: s.exists,
                sources: if highlights && s.exists {
                    source_counts(Path::new(&s.index_path))
                } else {
                    BTreeMap::new()
                },
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::fixture::*;
    use crate::corpus::IndexKeeper;
    use scout_corpus::api::SearchQuery;

    #[test]
    fn writing_citation_has_the_full_title_and_both_links_in_markdown_and_html() {
        let fx = Fixture::new("copy-cite");
        write(
            &fx.sources[0],
            "essays/2016-repaved-paths.md",
            "---\ntitle: \"Repaved paths and generative metaphors: Expressing human purposes with technology\"\n\
             date: 2016-06-23\ngenre: essay\npublished_url: https://medium.com/metaphor-hacker/repaved-paths\n---\n\
             An oft-quoted example is of Stanford not building any paths on their new campus.\n\
             They simply looked at paths trodden in the grass.\n\n\
             But being open to repaving once new paths are trodden is the *most* important thing.\n",
        );
        IndexKeeper::default()
            .refresh(|| Ok(fx.engine()), &|_| {})
            .unwrap();
        let before = fx.snapshot();

        let r = corpus::search(
            &fx.engine(),
            &SearchQuery {
                query: "repaving".into(),
                in_: vec!["writing".into()],
                ..Default::default()
            },
        )
        .unwrap();
        let pid = r.body.results[0].hits[0].passage_id.clone();
        let v = passage(&fx.engine(), &pid).unwrap().body;

        let md = &v.cited.citation.markdown;
        assert!(
            md.starts_with("> But being open to repaving once new paths are trodden is the *most* important thing.\n\n"),
            "{md}"
        );
        let attribution = md.lines().last().unwrap();
        assert!(attribution.starts_with(
            "— Dominik Lukeš, *Repaved paths and generative metaphors: Expressing human purposes with technology*, 23 June 2016 · [archive](writeflex://open?path="
        ), "{attribution}");
        assert!(
            attribution.ends_with(" · [public](https://medium.com/metaphor-hacker/repaved-paths)"),
            "{attribution}"
        );

        // Rich text: the same citation, as HTML, links kept.
        assert!(v.html.starts_with(
            "<blockquote><p>But being open to repaving once new paths are trodden is the <em>most</em> important thing.</p></blockquote>\
             <p>— Dominik Lukeš, <em>Repaved paths and generative metaphors: Expressing human purposes with technology</em>, 23 June 2016 · <a href=\"writeflex://open?path="
        ), "{}", v.html);
        assert!(v
            .html
            .ends_with(" · <a href=\"https://medium.com/metaphor-hacker/repaved-paths\">public</a></p>"));
        // Plain text: the public URL only, never the local path.
        assert_eq!(
            v.plain,
            "“But being open to repaving once new paths are trodden is the *most* important thing.”\n\
             — Dominik Lukeš, Repaved paths and generative metaphors: Expressing human purposes with technology, 23 June 2016 · <https://medium.com/metaphor-hacker/repaved-paths>\n"
        );
        assert_no_visible_local_link(&v);

        // Context: the paragraph before, frontmatter never.
        assert_eq!(
            v.context_before.as_deref(),
            Some("An oft-quoted example is of Stanford not building any paths on their new campus. They simply looked at paths trodden in the grass.")
        );

        // Tweets and highlights get no context paragraph.
        let r = corpus::search(
            &fx.engine(),
            &SearchQuery {
                query: "generative".into(),
                in_: vec!["tweets".into(), "highlights".into()],
                ..Default::default()
            },
        )
        .unwrap();
        for d in &r.body.results {
            let v = passage(&fx.engine(), &d.hits[0].passage_id).unwrap().body;
            assert_eq!(v.context_before, None, "{}", d.corpus);
            assert!(v.html.starts_with("<blockquote><p>"), "{}", v.html);
            assert_no_visible_local_link(&v);
        }

        // Counts: indexed documents per corpus; works per source for highlights.
        let c = counts(&fx.engine()).unwrap();
        let by: Vec<(&str, u64)> = c.iter().map(|c| (c.corpus.as_str(), c.docs)).collect();
        assert_eq!(by, vec![("writing", 2), ("tweets", 1), ("highlights", 1)]);
        assert_eq!(c[2].sources.get("zotero"), Some(&1));
        assert!(c[0].sources.is_empty());

        assert_eq!(fx.snapshot(), before);
    }

    /// The text a reader sees in each format: Markdown without its link
    /// targets, HTML without its tags, plain as it is.
    fn visible(md: &str, html: &str, plain: &str) -> [String; 3] {
        let md_text = Regex::new(r"\]\([^)\s]+\)").unwrap().replace_all(md, "]");
        let html_text = Regex::new(r"<[^>]*>").unwrap().replace_all(html, "");
        [md_text.into_owned(), html_text.into_owned(), plain.to_string()]
    }

    fn assert_no_visible_local_link(v: &PassageView) {
        for (i, text) in visible(&v.cited.citation.markdown, &v.html, &v.plain).iter().enumerate() {
            for scheme in ["writeflex://", "file://", "zotero://"] {
                assert!(!text.contains(scheme), "format {i} shows a raw {scheme} URL: {text}");
            }
        }
    }

    // Golden: the three forms the engine writes (writing, tweet, highlight),
    // as `scout cite` prints them in Markdown.
    const WRITING_MD: &str = "> Q one.\n>\n> Q two.\n\n— Dominik Lukeš, *Full Title: Sub*, 23 June 2016 · [archive](writeflex://open?path=%2Fa.md&line=3) · [public](https://medium.com/x/repaved)\n";
    const WRITING_LOCAL_MD: &str = "> Q.\n\n— Dominik Lukeš, *Full Title*, 23 June 2016 · [archive](writeflex://open?path=%2Fa.md&line=3)\n";
    const TWEET_MD: &str = "> Q\n\n— Dominik Lukeš (@techczech), tweet, 26 July 2025 · [public](https://x.com/techczech/status/1)\n";
    const HIGHLIGHT_MD: &str = "> Q\n\n— George Lakoff, *Metaphors We Live By*, 1980 · [highlight](file:///a/b%20c.md)\n";

    #[test]
    fn golden_markdown_html_and_plain_for_every_citation_form() {
        let cases: [(&str, &str, &str); 4] = [
            (
                WRITING_MD,
                "<blockquote><p>Q one.</p><p>Q two.</p></blockquote><p>— Dominik Lukeš, <em>Full Title: Sub</em>, 23 June 2016 · <a href=\"writeflex://open?path=%2Fa.md&amp;line=3\">archive</a> · <a href=\"https://medium.com/x/repaved\">public</a></p>",
                "“Q one.\n\nQ two.”\n— Dominik Lukeš, Full Title: Sub, 23 June 2016 · <https://medium.com/x/repaved>\n",
            ),
            (
                WRITING_LOCAL_MD,
                "<blockquote><p>Q.</p></blockquote><p>— Dominik Lukeš, <em>Full Title</em>, 23 June 2016 · <a href=\"writeflex://open?path=%2Fa.md&amp;line=3\">archive</a></p>",
                "“Q.”\n— Dominik Lukeš, Full Title, 23 June 2016\n",
            ),
            (
                TWEET_MD,
                "<blockquote><p>Q</p></blockquote><p>— Dominik Lukeš (@techczech), tweet, 26 July 2025 · <a href=\"https://x.com/techczech/status/1\">public</a></p>",
                "“Q”\n— Dominik Lukeš (@techczech), tweet, 26 July 2025 · <https://x.com/techczech/status/1>\n",
            ),
            (
                HIGHLIGHT_MD,
                "<blockquote><p>Q</p></blockquote><p>— George Lakoff, <em>Metaphors We Live By</em>, 1980 · <a href=\"file:///a/b%20c.md\">highlight</a></p>",
                "“Q”\n— George Lakoff, Metaphors We Live By, 1980\n",
            ),
        ];
        for (md, html, plain) in cases {
            assert_eq!(citation_html(md), html, "{md}");
            assert_eq!(citation_plain(md), plain, "{md}");
            for (i, text) in visible(md, html, plain).iter().enumerate() {
                assert!(!text.contains("writeflex://") && !text.contains("file://"), "format {i}: {text}");
            }
        }
        // Markdown keeps the words as link text, the URLs behind them.
        assert!(WRITING_MD.contains(" · [archive](writeflex://") && WRITING_MD.contains(" · [public](https://"));
    }

    #[test]
    fn the_visible_text_check_catches_a_raw_local_url() {
        // The engine's own plain form prints the local path; the check must fail on it.
        let engine_plain = "“Q.”\n— A, T · archive: writeflex://open?path=%2Fa.md · public: https://e.x\n";
        let [_, _, plain] = visible(WRITING_MD, &citation_html(WRITING_MD), engine_plain);
        assert!(plain.contains("writeflex://"));
        let [md, html, _] = visible("— A · writeflex://open?path=x", "<p>writeflex://open?path=x</p>", "");
        assert!(md.contains("writeflex://") && html.contains("writeflex://"));
    }

    #[test]
    fn html_escapes_text_and_drops_unsafe_links() {
        assert_eq!(
            citation_html("> a <b> & [x](javascript:alert(1))\n>\n> second **bold**\n\n— A, *T* · [public](https://e.x/?a=1&b=\"2\")\n"),
            "<blockquote><p>a &lt;b&gt; &amp; [x](javascript:alert(1))</p><p>second <strong>bold</strong></p></blockquote>\
             <p>— A, <em>T</em> · <a href=\"https://e.x/?a=1&amp;b=&quot;2&quot;\">public</a></p>"
        );
        assert_eq!(citation_html("> line one\n> line two\n\n— B\n"), "<blockquote><p>line one<br>line two</p></blockquote><p>— B</p>");
    }

    #[test]
    fn context_skips_blank_lines_headings_and_frontmatter() {
        let t = "---\ntitle: x\n---\n# Heading\n\nFirst para\ncontinues.\n\n\nThe passage.\n";
        assert_eq!(context_before(t, 10).as_deref(), Some("First para continues."));
        assert_eq!(context_before(t, 6), None); // only the heading before
        assert_eq!(context_before("---\ntitle: x\n---\nThe passage.\n", 4), None);
        let long = format!("{}\n\nP.\n", "word ".repeat(300));
        let c = context_before(&long, 3).unwrap();
        assert!(c.starts_with('…') && c.chars().count() <= CONTEXT_MAX + 1);
    }

    #[test]
    fn source_counts_is_empty_for_a_missing_index() {
        assert!(source_counts(Path::new("/nonexistent/highlights.sqlite")).is_empty());
    }
}
