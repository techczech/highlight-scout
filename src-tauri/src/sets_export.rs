//! The three exports of a set (HS-2A): a WriteFlex piece, TalkWeaver quote
//! slides and a citation list. Pure renderers over [`ExportItem`]; the
//! commands fill the items from the engine (`scout cite`), so the citation
//! wording is the engine's, never made up here.
//!
//! - WriteFlex piece: Markdown, `# title`, then per item the quote as a
//!   blockquote, its citation line, and the item's note as a paragraph.
//! - TalkWeaver: an outline in the format TalkWeaver compiles
//!   (`outline_version: 2`, `##` section, one `###` slide per item with the
//!   `{quote}` layout, the `>` quotation and a `- ` source line). Notes are
//!   not slide content and are left out.
//! - Citation list: one numbered line per item, with its links.

use crate::sets::{SetFile, SetItem, Status};
use scout_corpus::CitedPassage;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExportKind {
    Writeflex,
    Talkweaver,
    Citations,
}

impl ExportKind {
    pub fn file_name(self, title: &str) -> String {
        let base = slug::slugify(title);
        let base = if base.is_empty() { "set".to_string() } else { base };
        match self {
            ExportKind::Writeflex => format!("{base}.md"),
            ExportKind::Talkweaver => format!("{base}-quotes.md"),
            ExportKind::Citations => format!("{base}-citations.md"),
        }
    }
}

/// One item as the exports need it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportItem {
    /// The passage's original text: current when the source resolved,
    /// otherwise the quote stored in the set.
    pub quote: String,
    pub note: String,
    pub title: String,
    pub author: Option<String>,
    pub date_display: Option<String>,
    /// The engine's Markdown citation (both links); empty when the source changed.
    pub citation_markdown: String,
}

/// An item with its engine citation; a changed source keeps its stored quote.
pub fn export_item(it: &SetItem, status: Status, cited: Option<&CitedPassage>) -> ExportItem {
    match (status, cited) {
        (Status::Changed, _) | (_, None) => ExportItem {
            quote: it.quote.clone(),
            note: it.note.clone(),
            title: it.source_path.clone(),
            author: None,
            date_display: None,
            citation_markdown: String::new(),
        },
        (_, Some(c)) => ExportItem {
            quote: c.quote.trim().to_string(),
            note: it.note.clone(),
            title: c.title.clone(),
            author: c.author.clone(),
            date_display: c.date_display.clone(),
            citation_markdown: c.citation.markdown.clone(),
        },
    }
}

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn blockquote(q: &str) -> String {
    q.trim()
        .split('\n')
        .map(|l| if l.trim().is_empty() { ">".to_string() } else { format!("> {}", l.trim_end()) })
        .collect::<Vec<_>>()
        .join("\n")
}

/// "Author, Title, date": the plain source line of a slide.
pub fn source_line(i: &ExportItem) -> String {
    let mut parts: Vec<String> = vec![];
    if let Some(a) = i.author.as_ref().filter(|a| !a.trim().is_empty()) {
        parts.push(one_line(a));
    }
    if !i.title.trim().is_empty() {
        parts.push(one_line(&i.title));
    }
    if let Some(d) = i.date_display.as_ref().filter(|d| !d.trim().is_empty()) {
        parts.push(one_line(d));
    }
    parts.join(", ")
}

pub fn writeflex_piece(set: &SetFile, items: &[ExportItem]) -> String {
    let mut o = format!("# {}\n", one_line(&set.title));
    for i in items {
        o.push('\n');
        o.push_str(&blockquote(&i.quote));
        o.push('\n');
        let cite = if i.citation_markdown.trim().is_empty() { source_line(i) } else { one_line(&i.citation_markdown) };
        if !cite.is_empty() {
            o.push('\n');
            o.push_str(&cite);
            o.push('\n');
        }
        if !i.note.trim().is_empty() {
            o.push('\n');
            o.push_str(i.note.trim());
            o.push('\n');
        }
    }
    o
}

pub fn talkweaver_slides(set: &SetFile, items: &[ExportItem]) -> String {
    let title = one_line(&set.title);
    let mut o = format!("---\ntitle: {}\noutline_version: 2\n---\n# {title}\n\n## Quotes\n", serde_json::to_string(&title).unwrap());
    for (n, i) in items.iter().enumerate() {
        let heading = one_line(&i.title);
        let heading = if heading.is_empty() { format!("Quote {}", n + 1) } else { heading };
        o.push_str(&format!("\n### {heading}\n{{quote}}\n\n{}\n", blockquote(&i.quote)));
        let src = source_line(i);
        if !src.is_empty() {
            o.push_str(&format!("\n- {src}\n"));
        }
    }
    o
}

pub fn citation_list(set: &SetFile, items: &[ExportItem]) -> String {
    let mut o = format!("# {}\n\n", one_line(&set.title));
    for (n, i) in items.iter().enumerate() {
        let cite = if i.citation_markdown.trim().is_empty() { source_line(i) } else { one_line(&i.citation_markdown) };
        o.push_str(&format!("{}. {}\n", n + 1, cite));
    }
    o
}

pub fn render(kind: ExportKind, set: &SetFile, items: &[ExportItem]) -> String {
    match kind {
        ExportKind::Writeflex => writeflex_piece(set, items),
        ExportKind::Talkweaver => talkweaver_slides(set, items),
        ExportKind::Citations => citation_list(set, items),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sets::SetKind;

    fn set() -> SetFile {
        SetFile::new("Metaphor talk", SetKind::Hand, None, "2026-10-20")
    }

    fn items() -> Vec<ExportItem> {
        vec![
            ExportItem {
                quote: "But being open to repaving once new paths are trodden.".into(),
                note: "Open the talk with this".into(),
                title: "Repaved paths".into(),
                author: Some("Dominik Lukeš".into()),
                date_display: Some("23 June 2016".into()),
                citation_markdown: "Dominik Lukeš, *Repaved paths* (2016) [archive](writeflex://open?path=a.md&line=3) · [public](https://example.org/a)".into(),
            },
            ExportItem {
                quote: "Two paragraphs.\n\nThe second one.".into(),
                note: String::new(),
                title: "Air & Light".into(),
                author: None,
                date_display: None,
                citation_markdown: String::new(),
            },
        ]
    }

    #[test]
    fn writeflex_piece_golden() {
        assert_eq!(
            writeflex_piece(&set(), &items()),
            "# Metaphor talk\n\n> But being open to repaving once new paths are trodden.\n\nDominik Lukeš, *Repaved paths* (2016) [archive](writeflex://open?path=a.md&line=3) · [public](https://example.org/a)\n\nOpen the talk with this\n\n> Two paragraphs.\n>\n> The second one.\n\nAir & Light\n"
        );
    }

    #[test]
    fn talkweaver_slides_golden() {
        assert_eq!(
            talkweaver_slides(&set(), &items()),
            "---\ntitle: \"Metaphor talk\"\noutline_version: 2\n---\n# Metaphor talk\n\n## Quotes\n\n### Repaved paths\n{quote}\n\n> But being open to repaving once new paths are trodden.\n\n- Dominik Lukeš, Repaved paths, 23 June 2016\n\n### Air & Light\n{quote}\n\n> Two paragraphs.\n>\n> The second one.\n\n- Air & Light\n"
        );
    }

    #[test]
    fn citation_list_golden() {
        assert_eq!(
            citation_list(&set(), &items()),
            "# Metaphor talk\n\n1. Dominik Lukeš, *Repaved paths* (2016) [archive](writeflex://open?path=a.md&line=3) · [public](https://example.org/a)\n2. Air & Light\n"
        );
    }

    #[test]
    fn file_names_come_from_the_title() {
        assert_eq!(ExportKind::Writeflex.file_name("Metaphor talk"), "metaphor-talk.md");
        assert_eq!(ExportKind::Talkweaver.file_name("Metaphor talk"), "metaphor-talk-quotes.md");
        assert_eq!(ExportKind::Citations.file_name("???"), "set-citations.md");
    }

    #[test]
    fn a_changed_source_exports_its_stored_quote() {
        let it = SetItem { passage: "writing:a.md:3".into(), corpus: "writing".into(), source_path: "a.md".into(), quote: "Stored text".into(), pinned: false, note: "n".into(), ..Default::default() };
        let e = export_item(&it, Status::Changed, None);
        assert_eq!(e.quote, "Stored text");
        assert!(writeflex_piece(&set(), &[e]).contains("> Stored text"));
    }
}
