//! Reading a work file's frontmatter to find tweets by key, and the one
//! edit the tweet importer makes to it (`updated_at`).
//!
//! A file has an identity for the importer in two cases only:
//!
//! - its frontmatter is a valid YAML mapping: the key is the parsed
//!   top-level `source_system` and `source_id`;
//! - its frontmatter is not valid YAML (the app writes unquoted titles such
//!   as `title: @someone …`), but it is flat: every non-empty line is a
//!   `key: value` line at column 0 whose value, if it opens a quote or a flow
//!   collection, closes it on that line, and opens no block scalar; and
//!   `source_system` and `source_id` each appear exactly once, as simple
//!   values.
//!
//! Any other frontmatter is `Reading::Unreadable`: the file holds no key as
//! far as the importer is concerned and is reported by path. A leading UTF-8
//! BOM is skipped. At most `FRONTMATTER_CAP` bytes are read for it.

use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use yaml_rust2::{Yaml, YamlLoader};

/// The most bytes of a file read for its frontmatter. One that has not
/// closed by then is unreadable.
pub const FRONTMATTER_CAP: usize = 256 * 1024;

const BOM: &[u8] = b"\xEF\xBB\xBF";

/// What the start of a file holds.
pub enum Reading {
    /// The file does not open with a `---` line.
    NoFrontmatter,
    /// It opens with one, but what follows is not a frontmatter the importer
    /// can take an identity from (see the module comment).
    Unreadable,
    Readable(Frontmatter),
}

impl Reading {
    pub fn readable(self) -> Option<Frontmatter> {
        match self {
            Reading::Readable(fm) => Some(fm),
            _ => None,
        }
    }
}

/// A readable frontmatter's top-level scalar fields.
pub struct Frontmatter(Fields);

enum Fields {
    Yaml(Yaml),
    /// The `key: value` lines of a flat frontmatter that is not valid YAML.
    Lines(Vec<(String, String)>),
}

impl Frontmatter {
    /// A top-level scalar field as text. None for a missing, empty or
    /// non-scalar value, and (in a flat frontmatter) for a repeated key.
    pub fn get(&self, field: &str) -> Option<String> {
        let v = match &self.0 {
            Fields::Yaml(doc) => scalar(&doc[field])?,
            Fields::Lines(lines) => {
                let mut values = lines.iter().filter(|(k, _)| k == field).map(|(_, v)| v);
                let v = values.next()?;
                if values.next().is_some() {
                    return None;
                }
                unquoted(v).to_string()
            }
        };
        Some(v.trim().to_string()).filter(|v| !v.is_empty())
    }

    /// `source_system:source_id`, when the file gives both.
    pub fn key(&self) -> Option<String> {
        Some(format!(
            "{}:{}",
            self.get("source_system")?,
            self.get("source_id")?
        ))
    }
}

fn scalar(v: &Yaml) -> Option<String> {
    match v {
        Yaml::String(s) | Yaml::Real(s) => Some(s.clone()),
        Yaml::Integer(i) => Some(i.to_string()),
        Yaml::Boolean(b) => Some(b.to_string()),
        _ => None,
    }
}

fn unquoted(v: &str) -> &str {
    ['"', '\'']
        .iter()
        .find_map(|q| v.strip_prefix(*q).and_then(|r| r.strip_suffix(*q)))
        .unwrap_or(v)
}

fn key_line() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^([A-Za-z_][A-Za-z0-9_]*):(?: (.*))?$").unwrap())
}

fn block_scalar_header() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[|>][0-9+\-]{0,2}\s*(#.*)?$").unwrap())
}

fn simple_identity() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[A-Za-z0-9_.\-]+$").unwrap())
}

/// Where the quoted scalar that opens `s` (with `quote`) ends: the index
/// just past its closing quote, or None when it does not close in `s`.
fn quote_end(s: &str, quote: char) -> Option<usize> {
    let mut chars = s.char_indices().skip(1).peekable();
    while let Some((i, c)) = chars.next() {
        if quote == '"' && c == '\\' {
            chars.next();
        } else if c == quote {
            // In single quotes, `''` is a quote character, not the end.
            if quote == '\'' && chars.peek().is_some_and(|(_, n)| *n == '\'') {
                chars.next();
            } else {
                return Some(i + c.len_utf8());
            }
        }
    }
    None
}

/// True when the flow collection that opens `s` (`{` or `[`) is closed
/// again within `s`, quotes inside it respected.
fn flow_closes(s: &str) -> bool {
    let mut depth = 0usize;
    let mut i = 0;
    while let Some(c) = s[i..].chars().next() {
        match c {
            '{' | '[' => depth += 1,
            '}' | ']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return true;
                }
            }
            '"' | '\'' => match quote_end(&s[i..], c) {
                Some(end) => {
                    i += end;
                    continue;
                }
                None => return false,
            },
            _ => {}
        }
        i += c.len_utf8();
    }
    false
}

/// True when a value on a `key: value` line cannot run on to the next line:
/// a quote or a flow collection it opens with is closed on that line
/// (whatever follows it there), and it is not a block scalar header.
fn closes_on_its_line(value: &str) -> bool {
    match value.chars().next() {
        Some('{') | Some('[') => flow_closes(value),
        Some(q @ ('"' | '\'')) => quote_end(value, q).is_some(),
        Some('|') | Some('>') => !block_scalar_header().is_match(value),
        _ => true,
    }
}

/// The fields of a frontmatter that is not valid YAML, when it is flat and
/// names `source_system` and `source_id` exactly once each (module comment).
fn flat_lines(inner: &str) -> Option<Vec<(String, String)>> {
    let mut lines = Vec::new();
    for line in inner.lines().filter(|l| !l.is_empty()) {
        let caps = key_line().captures(line)?;
        let value = caps.get(2).map_or("", |m| m.as_str()).trim();
        if !closes_on_its_line(value) {
            return None;
        }
        lines.push((caps[1].to_string(), value.to_string()));
    }
    for identity in ["source_system", "source_id"] {
        let mut values = lines.iter().filter(|(k, _)| k == identity).map(|(_, v)| v);
        let value = unquoted(values.next()?);
        if values.next().is_some() || !simple_identity().is_match(value) {
            return None;
        }
    }
    Some(lines)
}

fn is_delimiter(line: &[u8]) -> bool {
    line.trim_ascii_end() == b"---"
}

/// Read the frontmatter at the start of a file's bytes (the whole file or
/// any prefix of it that reaches the closing `---` line).
pub fn read(bytes: &[u8]) -> Reading {
    let bytes = bytes.strip_prefix(BOM).unwrap_or(bytes);
    let cut_short = bytes.len() > FRONTMATTER_CAP;
    let bytes = &bytes[..bytes.len().min(FRONTMATTER_CAP)];
    let mut lines = bytes.split_inclusive(|b| *b == b'\n');
    if !lines.next().is_some_and(is_delimiter) {
        return Reading::NoFrontmatter;
    }
    let mut inner = Vec::new();
    for line in lines {
        // A last line the cap cut through is not a line.
        let whole = line.ends_with(b"\n") || !cut_short;
        if whole && is_delimiter(line) {
            return parse(&inner);
        }
        let line = line.strip_suffix(b"\n").unwrap_or(line);
        inner.extend_from_slice(line.strip_suffix(b"\r").unwrap_or(line));
        inner.push(b'\n');
    }
    Reading::Unreadable
}

/// From the text between the `---` lines.
fn parse(inner: &[u8]) -> Reading {
    let Ok(inner) = std::str::from_utf8(inner) else {
        return Reading::Unreadable;
    };
    let fields = match YamlLoader::load_from_str(inner) {
        Ok(docs) => match docs.into_iter().next() {
            Some(doc @ Yaml::Hash(_)) => Some(Fields::Yaml(doc)),
            _ => None,
        },
        Err(_) => flat_lines(inner).map(Fields::Lines),
    };
    match fields {
        Some(f) => Reading::Readable(Frontmatter(f)),
        None => Reading::Unreadable,
    }
}

/// `read` for a file on disk, reading no further than the frontmatter's
/// closing line or the cap.
fn read_file(path: &Path) -> std::io::Result<Reading> {
    // One byte past the cap (and the BOM) tells `read` the file went on.
    let limit = (FRONTMATTER_CAP + BOM.len() + 1) as u64;
    let mut reader = BufReader::new(fs::File::open(path)?.take(limit));
    let mut buf = Vec::new();
    let mut first = true;
    loop {
        let start = buf.len();
        if reader.read_until(b'\n', &mut buf)? == 0 {
            break;
        }
        let line = &buf[start..];
        if first {
            first = false;
            if !is_delimiter(line.strip_prefix(BOM).unwrap_or(line)) {
                return Ok(Reading::NoFrontmatter);
            }
        } else if is_delimiter(line) {
            break;
        }
    }
    Ok(read(&buf))
}

/// The work files of one source system, found by reading every `.md` file's
/// frontmatter.
#[derive(Default)]
pub struct Scan {
    /// Files by their `source_system:source_id` key, paths in name order.
    pub by_key: HashMap<String, Vec<PathBuf>>,
    /// Files that open with `---` but whose frontmatter gives the importer
    /// no identity, or that could not be opened: they hold no key here.
    pub unreadable: Vec<PathBuf>,
}

/// Every work file in `dir` whose frontmatter names `source_system`.
pub fn scan(dir: &Path, source_system: &str) -> Scan {
    let mut out = Scan::default();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".md") || crate::archive_meta::is_temp_write_name(&name) {
            continue;
        }
        let path = entry.path();
        match read_file(&path) {
            Ok(Reading::NoFrontmatter) => {}
            Ok(Reading::Unreadable) | Err(_) => out.unreadable.push(path),
            Ok(Reading::Readable(fm)) => {
                if fm.get("source_system").as_deref() != Some(source_system) {
                    continue;
                }
                if let Some(key) = fm.key() {
                    out.by_key.entry(key).or_default().push(path);
                }
            }
        }
    }
    for paths in out.by_key.values_mut() {
        paths.sort();
    }
    out.unreadable.sort();
    out
}

fn plain_timestamp() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[0-9A-Za-z:+.\-]+$").unwrap())
}

fn flat_line() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[A-Za-z_][A-Za-z0-9_]*: \S").unwrap())
}

/// `head` (a work file's frontmatter with both `---` lines, as
/// `archive_meta::split_file_records` returns it) with `updated_at` set to
/// `value`. None, and nothing is to be changed, unless `updated_at` is one
/// plain single-line scalar and the edit provably alters nothing else:
/// when the frontmatter is valid YAML, it must parse afterwards to the same
/// mapping with only that value different; when it is not, every line must
/// be a flat `key: value` line.
pub fn set_updated_at(head: &str, value: &str) -> Option<String> {
    const PREFIX: &str = "updated_at: ";
    if !plain_timestamp().is_match(value) || head.contains('\r') {
        return None;
    }
    let lines: Vec<&str> = head.lines().collect();
    if lines.len() < 2 || lines[0] != "---" || lines[lines.len() - 1] != "---" {
        return None;
    }
    let inner = &lines[1..lines.len() - 1];
    let mut at = inner
        .iter()
        .enumerate()
        .filter(|(_, l)| l.starts_with(PREFIX));
    let (i, line) = at.next()?;
    if at.next().is_some() {
        return None;
    }
    let old = &line[PREFIX.len()..];
    if !plain_timestamp().is_match(old) {
        return None;
    }
    let mut edited: Vec<String> = inner.iter().map(|l| l.to_string()).collect();
    edited[i] = format!("{PREFIX}{value}");

    let load = |ls: &[String]| {
        YamlLoader::load_from_str(&(ls.join("\n") + "\n"))
            .ok()
            .and_then(|d| d.into_iter().next())
    };
    let before: Vec<String> = inner.iter().map(|l| l.to_string()).collect();
    match (load(&before), load(&edited)) {
        (Some(Yaml::Hash(b)), Some(Yaml::Hash(a))) => {
            let key = Yaml::String("updated_at".into());
            if scalar(b.get(&key)?)? != old || scalar(a.get(&key)?)? != value {
                return None;
            }
            let same_elsewhere = b.len() == a.len()
                && b.iter()
                    .zip(a.iter())
                    .all(|((bk, bv), (ak, av))| bk == ak && (*bk == key || bv == av));
            if !same_elsewhere {
                return None;
            }
        }
        (None, None) => {
            if !inner.iter().all(|l| flat_line().is_match(l)) {
                return None;
            }
        }
        _ => return None,
    }
    Some(format!("---\n{}\n---\n", edited.join("\n")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key_of(inner: &str) -> Option<String> {
        read(format!("---\n{inner}---\n\n> q\n").as_bytes())
            .readable()
            .and_then(|fm| fm.key())
    }

    fn unreadable(inner: &str) -> bool {
        matches!(
            read(format!("---\n{inner}---\n").as_bytes()),
            Reading::Unreadable
        )
    }

    #[test]
    fn identity_is_found_however_the_yaml_spells_it() {
        for inner in [
            "title: A\nsource_system: x\nsource_id: \"1904944923159445914\"\n",
            "source_id: '1904944923159445914'\ntitle: A\nsource_system: \"x\"\n",
            "source_system: x # the tweet importer\nsource_id: 1904944923159445914 # numeric\n",
            "\"source_system\": 'x'\n\"source_id\": >-\n  1904944923159445914\n",
            // Not valid YAML (the app writes such titles) but flat.
            "title: @alice said\nsource_system: x\nsource_id: \"1904944923159445914\"\n",
            "title: \"a \\ b\"\nsource_system: 'x'\nsource_id: '1904944923159445914'\nsource_data: {\"a\":1}\n",
            "title: >be me\n\nsource_system: x\nsource_id: \"1904944923159445914\"\n",
            // Titles the app wrote unquoted that open with a quote or a
            // bracket closed on the same line.
            "title: 'believe victims' except when…\nsource_system: x\nsource_id: \"1904944923159445914\"\n",
            "title: \"So\" they said, and left\nsource_system: x\nsource_id: \"1904944923159445914\"\n",
            "title: [AINews] Not Much: Happened\nsource_system: x\nsource_id: \"1904944923159445914\"\n",
            "title: {a: [b, \"]\"]} and on\nsource_system: x\nsource_id: \"1904944923159445914\"\n",
        ] {
            assert_eq!(
                key_of(inner).as_deref(),
                Some("x:1904944923159445914"),
                "{inner}"
            );
        }
        assert_eq!(key_of("title: A\nsource_system: x\n"), None);
        // A field inside a block scalar is text, not identity.
        assert_eq!(
            key_of("title: |\n  source_system: x\n  source_id: 5\nsource_system: zotero\nsource_id: ABC\n")
                .as_deref(),
            Some("zotero:ABC")
        );
    }

    #[test]
    fn duplicate_identity_lines_give_no_identity() {
        // r2 P1: the YAML loader rejects the duplicate keys; the flat reader
        // must not then take the first pair.
        let inner =
            "source_system: x\nsource_id: \"123\"\nsource_system: zotero\nsource_id: \"ABC\"\n";
        assert!(unreadable(inner));
        assert_eq!(key_of(inner), None);
        assert!(unreadable(
            "title: @a\nsource_system: x\nsource_id: \"1\"\nsource_id: \"1\"\n"
        ));
    }

    #[test]
    fn a_column_0_key_inside_an_unclosed_flow_mapping_gives_no_identity() {
        // r2 P1, the nested variant.
        let inner = "title: @someone\nsource_data: {\nsource_system: x\n}\nsource_system: zotero\nsource_id: \"123\"\n";
        assert!(unreadable(inner));
        assert_eq!(key_of(inner), None);
    }

    #[test]
    fn a_frontmatter_that_is_neither_yaml_nor_flat_is_unreadable() {
        for inner in [
            // an indented line
            "title: @alice\n  more\nsource_system: x\nsource_id: \"1\"\n",
            "title: @alice\n \nsource_system: x\nsource_id: \"1\"\n",
            // a quote, a flow sequence and a block scalar left open
            "title: @alice\nnote: \"open\nsource_system: x\nsource_id: \"1\"\n",
            "title: @alice\nnote: \"open\\\"\nsource_system: x\nsource_id: \"1\"\n",
            "title: @alice\nnote: 'open\nsource_system: x\nsource_id: \"1\"\n",
            "title: @alice\ntags: [a,\nsource_system: x\nsource_id: \"1\"\n",
            "title: @alice\ntags: [a, \"]\nsource_system: x\nsource_id: \"1\"\n",
            "title: @alice\nnote: 'it''s open\nsource_system: x\nsource_id: \"1\"\n",
            "title: @alice\nnote: {a: [b}\nsource_system: x\nsource_id: \"1\"\n",
            "title: @alice\nnote: |\nsource_system: x\nsource_id: \"1\"\n",
            "title: @alice\nnote: >- # folded\nsource_system: x\nsource_id: \"1\"\n",
            // a line that is not `key: value`
            "title: @alice\n# a comment\nsource_system: x\nsource_id: \"1\"\n",
            "title: @alice\nsource_system:x\nsource_id: \"1\"\n",
            // an identity that is not a simple value
            "title: @alice\nsource_system: x # tweets\nsource_id: \"1\"\n",
            "title: @alice\nsource_system: x\nsource_id: \"1\" \"2\"\n",
            // identity missing from a frontmatter that is not YAML
            "title: @alice\nsource_system: x\n",
            // valid YAML that is not a mapping
            "- a\n- b\n",
            "just text\n",
            "",
        ] {
            assert!(unreadable(inner), "{inner:?}");
        }
    }

    #[test]
    fn a_bom_and_either_line_ending_are_read_and_an_unclosed_frontmatter_is_not() {
        let crlf = "---\r\nsource_system: x\r\nsource_id: \"7\"\r\n---\r\n\r\n> q\r\n";
        assert_eq!(
            read(crlf.as_bytes()).readable().unwrap().key().as_deref(),
            Some("x:7")
        );
        let bom = [
            BOM,
            b"---\nsource_system: x\nsource_id: \"7\"\n---\n\n> q\n",
        ]
        .concat();
        assert_eq!(read(&bom).readable().unwrap().key().as_deref(), Some("x:7"));
        // Closed by the file's last line, with no line ending after it.
        assert!(read(b"---\nsource_system: x\nsource_id: \"7\"\n---")
            .readable()
            .is_some());
        assert!(matches!(read(b"no frontmatter\n"), Reading::NoFrontmatter));
        assert!(matches!(read(b""), Reading::NoFrontmatter));
        assert!(matches!(
            read(b"---\nsource_system: x\n"),
            Reading::Unreadable
        ));
        assert!(matches!(
            read(b"---\nsource_system: \xff\nsource_id: 1\n---\n"),
            Reading::Unreadable
        ));
    }

    #[test]
    fn a_frontmatter_that_does_not_close_within_the_cap_is_unreadable() {
        let dir = std::env::temp_dir().join(format!("hs-fm-cap-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let head = "---\nsource_system: x\nsource_id: \"7\"\n";
        let pad = |len: usize| format!("note: {}\n", "a".repeat(len - "note: \n".len()));

        // Closing line ends exactly at the cap: read.
        let fits = format!("{head}{}---\n", pad(FRONTMATTER_CAP - head.len() - 4));
        assert_eq!(fits.find("\n---\n").unwrap() + 5, FRONTMATTER_CAP);
        // One byte more before it: not read, in memory or from disk.
        let over = format!(
            "{head}{}---\n\n> q\n",
            pad(FRONTMATTER_CAP - head.len() - 3)
        );
        // The cap falls inside a `---x` line: that is not a closing line.
        let through = format!("{head}{}---x\n", pad(FRONTMATTER_CAP - head.len() - 3));
        assert_eq!(
            &through.as_bytes()[FRONTMATTER_CAP - 3..FRONTMATTER_CAP],
            b"---"
        );

        fs::write(dir.join("fits.md"), &fits).unwrap();
        fs::write(dir.join("over.md"), &over).unwrap();
        fs::write(dir.join("through.md"), &through).unwrap();
        assert!(read(fits.as_bytes()).readable().is_some());
        assert!(matches!(read(over.as_bytes()), Reading::Unreadable));
        assert!(matches!(read(through.as_bytes()), Reading::Unreadable));

        let found = scan(&dir, "x");
        assert_eq!(found.by_key["x:7"], [dir.join("fits.md")]);
        assert_eq!(
            found.unreadable,
            [dir.join("over.md"), dir.join("through.md")]
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn updated_at_is_set_only_when_nothing_else_can_change() {
        let head = "---\ntitle: A\nimported_at: 2026-06-20T00:00:00+00:00\nupdated_at: 2026-06-20T00:00:00+00:00\nsource_data: {\"a\":1}\n---\n";
        assert_eq!(
            set_updated_at(head, "2026-06-21T00:00:00+00:00").as_deref(),
            Some("---\ntitle: A\nimported_at: 2026-06-20T00:00:00+00:00\nupdated_at: 2026-06-21T00:00:00+00:00\nsource_data: {\"a\":1}\n---\n")
        );
        // The app's own not-quite-YAML, all flat lines: still safe.
        let sloppy = "---\ntitle: @alice said\nupdated_at: 2026-06-20T00:00:00+00:00\n---\n";
        assert!(set_updated_at(sloppy, "2026-06-21T00:00:00+00:00").is_some());
        for refused in [
            // a quoted, a folded and a missing value
            "---\ntitle: A\nupdated_at: \"2026-06-20T00:00:00+00:00\"\n---\n",
            "---\ntitle: A\nupdated_at: >\n  2026-06-20\n---\n",
            "---\ntitle: A\n---\n",
            // two such lines
            "---\nupdated_at: 2026-06-20\nupdated_at: 2026-06-21\n---\n",
            // invalid YAML with a continuation line
            "---\ntitle: @alice\n  more\nupdated_at: 2026-06-20\n---\n",
            // not LF frontmatter
            "---\r\ntitle: A\r\nupdated_at: 2026-06-20\r\n---\r\n",
        ] {
            assert_eq!(
                set_updated_at(refused, "2026-06-21T00:00:00+00:00"),
                None,
                "{refused}"
            );
        }
        assert_eq!(set_updated_at(head, "not a: timestamp"), None);
    }
}
