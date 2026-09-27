//! Folding two rendered blocks of the same highlight into one, for the
//! duplicate-work merge.
//!
//! Two plain (quote) blocks are the same highlight when their quote text is
//! equal after whitespace normalisation and their `highlighted_at` date is
//! equal or missing on one side. The fold keeps the older block's text, the
//! union of tags (older first), the longer note (both, in order, when they
//! differ and neither contains the other) and any other metadata the older
//! block lacks from the newer one. Blocks this module cannot parse and
//! re-render byte for byte (images, latex, anything unusual) never fold.

/// Metadata keys in the order scout-archive renders them.
const META_KEYS: [&str; 5] = ["highlighted_at", "tags", "color", "type", "format"];

#[derive(Debug, Clone, PartialEq)]
struct Block {
    /// Quote lines as rendered (`> …`).
    quote: Vec<String>,
    meta: Vec<(String, String)>,
    note: Option<String>,
}

pub fn normalise(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

impl Block {
    fn parse(text: &str) -> Option<Block> {
        let lines: Vec<&str> = text.lines().collect();
        let mut i = 0;
        let mut quote = Vec::new();
        while i < lines.len() && (lines[i].starts_with("> ") || lines[i] == ">") {
            quote.push(lines[i].to_string());
            i += 1;
        }
        if quote.is_empty() || lines.get(i) != Some(&"") {
            return None;
        }
        i += 1;
        let mut meta = Vec::new();
        if let Some(parsed) = lines.get(i).and_then(|l| parse_meta(l)) {
            meta = parsed;
            i += 1;
        }
        let note = if i < lines.len() {
            if !lines[i].is_empty() {
                return None;
            }
            Some(lines[i + 1..].join("\n"))
        } else {
            None
        };
        let b = Block { quote, meta, note };
        (b.render() == text).then_some(b)
    }

    fn render(&self) -> String {
        let mut out = String::new();
        for l in &self.quote {
            out.push_str(l);
            out.push('\n');
        }
        out.push('\n');
        let parts: Vec<String> = META_KEYS
            .iter()
            .filter_map(|k| self.get(k).map(|v| format!("{k}: {v}")))
            .collect();
        if !parts.is_empty() {
            out.push_str(&parts.join(" | "));
            out.push('\n');
        }
        if let Some(n) = &self.note {
            out.push('\n');
            out.push_str(n);
            out.push('\n');
        }
        out
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.meta
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    fn quote_key(&self) -> String {
        normalise(
            &self
                .quote
                .iter()
                .map(|l| l.strip_prefix("> ").unwrap_or(""))
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }

    fn tags(&self) -> Vec<String> {
        self.get("tags")
            .map(|t| t.split(", ").map(String::from).collect())
            .unwrap_or_default()
    }

    fn same_highlight(&self, other: &Block) -> bool {
        self.quote_key() == other.quote_key()
            && match (self.get("highlighted_at"), other.get("highlighted_at")) {
                (Some(a), Some(b)) => a == b,
                _ => true,
            }
    }
}

/// A metadata line: `key: value` parts joined by ` | `, every key known.
fn parse_meta(line: &str) -> Option<Vec<(String, String)>> {
    line.split(" | ")
        .map(|part| {
            let (k, v) = part.split_once(": ")?;
            META_KEYS
                .contains(&k)
                .then(|| (k.to_string(), v.to_string()))
        })
        .collect()
}

fn merge_notes(older: Option<&str>, newer: Option<&str>) -> Option<String> {
    match (older, newer) {
        (None, n) => n.map(String::from),
        (o, None) => o.map(String::from),
        (Some(o), Some(n)) => {
            let (no, nn) = (normalise(o), normalise(n));
            Some(if no.contains(&nn) {
                o.to_string()
            } else if nn.contains(&no) {
                n.to_string()
            } else {
                format!("{o}\n\n{n}")
            })
        }
    }
}

/// If `newer` is the same highlight as `older`, the single block that
/// replaces both. None when they are different highlights or either block
/// does not parse.
pub fn fold(older: &str, newer: &str) -> Option<String> {
    let (o, n) = (Block::parse(older)?, Block::parse(newer)?);
    if !o.same_highlight(&n) {
        return None;
    }
    let mut tags = o.tags();
    for t in n.tags() {
        if !tags.contains(&t) {
            tags.push(t);
        }
    }
    let meta = META_KEYS
        .iter()
        .filter_map(|&k| {
            let v = if k == "tags" {
                (!tags.is_empty()).then(|| tags.join(", "))
            } else {
                o.get(k).or_else(|| n.get(k)).map(String::from)
            };
            v.map(|v| (k.to_string(), v))
        })
        .collect();
    let merged = Block {
        quote: o.quote.clone(),
        meta,
        note: merge_notes(o.note.as_deref(), n.note.as_deref()),
    };
    Some(merged.render())
}

/// The normalised quote text of a block, if it is a quote block.
pub fn quote_text(block: &str) -> Option<String> {
    let lines: Vec<&str> = block
        .lines()
        .take_while(|l| l.starts_with("> ") || *l == ">")
        .collect();
    (!lines.is_empty()).then(|| {
        normalise(
            &lines
                .iter()
                .map(|l| l.strip_prefix("> ").unwrap_or(""))
                .collect::<Vec<_>>()
                .join("\n"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_render_round_trip() {
        for b in [
            "> a quote\n\nhighlighted_at: 2025-03-13 | tags: education\n",
            "> two\n> lines\n\n",
            "> q\n\nhighlighted_at: 2025-03-13\n\nA note.\n",
            "> q\n\n\nA note without metadata.\n",
        ] {
            assert_eq!(Block::parse(b).unwrap().render(), b);
        }
        assert!(Block::parse("![](../assets/x.png)\n\n").is_none());
    }

    #[test]
    fn the_naep_pair_folds_to_one_block_with_the_tags() {
        let older = "> NAEP scores fell.\n\nhighlighted_at: 2025-03-13\n";
        let newer = "> NAEP  scores fell.\n\nhighlighted_at: 2025-03-13 | tags: education\n";
        assert_eq!(
            fold(older, newer).as_deref(),
            Some("> NAEP scores fell.\n\nhighlighted_at: 2025-03-13 | tags: education\n")
        );
    }

    #[test]
    fn tags_union_keeps_the_older_order() {
        let older = "> q\n\nhighlighted_at: 2025-03-13 | tags: b, a\n";
        let newer = "> q\n\nhighlighted_at: 2025-03-13 | tags: a, c | color: yellow\n";
        assert_eq!(
            fold(older, newer).as_deref(),
            Some("> q\n\nhighlighted_at: 2025-03-13 | tags: b, a, c | color: yellow\n")
        );
    }

    #[test]
    fn different_dates_are_different_highlights() {
        let older = "> q\n\nhighlighted_at: 2025-03-13\n";
        let newer = "> q\n\nhighlighted_at: 2025-04-01\n";
        assert_eq!(fold(older, newer), None);
        // A missing date on one side still matches.
        assert!(fold(older, "> q\n\ntags: x\n").is_some());
    }

    #[test]
    fn differing_notes_are_both_kept_and_a_contained_note_is_not_doubled() {
        let older = "> q\n\nhighlighted_at: 2025-03-13\n\nFirst thought.\n";
        let newer = "> q\n\nhighlighted_at: 2025-03-13\n\nSecond thought.\n";
        assert_eq!(
            fold(older, newer).as_deref(),
            Some("> q\n\nhighlighted_at: 2025-03-13\n\nFirst thought.\n\nSecond thought.\n")
        );
        let longer = "> q\n\nhighlighted_at: 2025-03-13\n\nFirst thought, extended.\n";
        let shorter = "> q\n\nhighlighted_at: 2025-03-13\n\nFirst thought\n";
        assert_eq!(fold(shorter, longer).as_deref(), Some(longer));
        assert_eq!(fold(longer, shorter).as_deref(), Some(longer));
    }

    #[test]
    fn different_quotes_never_fold() {
        assert_eq!(fold("> a\n\n", "> b\n\n"), None);
    }
}
