//! Reading a work file's frontmatter to find tweets by key, and the one
//! edit the tweet importer makes to it (`updated_at`).
//!
//! A file has an identity for the importer in two cases only:
//!
//! - its frontmatter is a valid YAML mapping: the key is the parsed
//!   top-level `source_system` and `source_id`;
//! - its frontmatter is not valid YAML (the app writes unquoted titles such
//!   as `title: @someone …`), but what is left of it without the lines that
//!   begin at column 0 with `title:` or `author:` (the two keys the app
//!   writes unquoted free text into) is one valid YAML mapping in which
//!   `source_system` and `source_id` are top-level scalars. The YAML parser
//!   decides: it rejects a repeated key and anything left open, and a title
//!   that ran on to further lines leaves a remainder it does not accept.
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
    /// A frontmatter that is not valid YAML: the mapping parsed from it
    /// without its `title:` and `author:` lines, and the text of those lines.
    Lenient {
        doc: Yaml,
        title: Vec<String>,
        author: Vec<String>,
    },
}

impl Frontmatter {
    /// A top-level scalar field as text. None for a missing, empty or
    /// non-scalar value. In a frontmatter that is not valid YAML, `title`
    /// and `author` are their lines' text as written (less one pair of
    /// enclosing quotes), and None when the key has more than one line.
    pub fn get(&self, field: &str) -> Option<String> {
        let v = match &self.0 {
            Fields::Yaml(doc) => scalar(&doc[field])?,
            Fields::Lenient { doc, title, author } => match field {
                "title" => unquoted(sole(title)?).to_string(),
                "author" => unquoted(sole(author)?).to_string(),
                _ => scalar(&doc[field])?,
            },
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

fn sole(values: &[String]) -> Option<&str> {
    match values {
        [one] => Some(one.trim()),
        _ => None,
    }
}

/// The identity of a frontmatter that is not valid YAML (module comment):
/// its lines without those that begin at column 0 with `title:` or
/// `author:`, when the YAML parser reads them as one mapping that gives
/// `source_system` and `source_id` as scalars.
fn lenient(inner: &str) -> Option<Fields> {
    let (mut title, mut author, mut rest) = (Vec::new(), Vec::new(), String::new());
    for line in inner.lines() {
        if let Some(value) = line.strip_prefix("title:") {
            title.push(value.to_string());
        } else if let Some(value) = line.strip_prefix("author:") {
            author.push(value.to_string());
        } else {
            rest.push_str(line);
            rest.push('\n');
        }
    }
    let mut docs = YamlLoader::load_from_str(&rest).ok()?.into_iter();
    let (Some(doc @ Yaml::Hash(_)), None) = (docs.next(), docs.next()) else {
        return None;
    };
    scalar(&doc["source_system"])?;
    scalar(&doc["source_id"])?;
    Some(Fields::Lenient { doc, title, author })
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
        Err(_) => lenient(inner),
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
            // Not valid YAML (the app writes such titles).
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
        // r2 P1: the YAML loader rejects the duplicate keys, with or without
        // the title line.
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
    fn an_anchored_unclosed_collection_gives_no_identity() {
        // r3 P1: `&n [` is left open; the parser refuses it, with the title
        // line (a valid one, or one the app wrote unquoted) taken out or not.
        for title in ["A manually edited work", "@alice said"] {
            let inner = format!("title: {title}\nnote: &n [\nsource_system: x\nsource_id: 123\n");
            assert!(unreadable(&inner), "{inner:?}");
            assert_eq!(key_of(&inner), None);
        }
        for value in ["!!seq [", "&a \"open", "&a !t {"] {
            let inner = format!("title: @alice\nnote: {value}\nsource_system: x\nsource_id: 123\n");
            assert!(unreadable(&inner), "{inner:?}");
        }
    }

    #[test]
    fn an_unquoted_title_the_app_wrote_leaves_the_file_its_identity() {
        // The shape the app writes: the title opens a quote it never closes.
        let inner = "title: @someone said: \"x\nauthor: @someone\nwork_type: tweet\nsource_system: x\nsource_id: \"1904944923159445914\"\nurl: https://x.com/someone/status/1904944923159445914\nsource_data: {\"a\":1}\n";
        let fm = read(format!("---\n{inner}---\n\n> q\n").as_bytes())
            .readable()
            .unwrap();
        assert_eq!(fm.key().as_deref(), Some("x:1904944923159445914"));
        assert_eq!(fm.get("title").as_deref(), Some("@someone said: \"x"));
        assert_eq!(fm.get("author").as_deref(), Some("@someone"));
        assert_eq!(fm.get("work_type").as_deref(), Some("tweet"));
        assert_eq!(fm.get("source_data"), None);
    }

    #[test]
    fn a_title_that_runs_on_to_further_lines_gives_no_identity() {
        for inner in [
            "title: @someone said: \"x\n  and more\"\nsource_system: x\nsource_id: \"1\"\n",
            "title: @alice\n  more\nsource_system: x\nsource_id: \"1\"\n",
            "title: @alice\n  more: text\nsource_system: x\nsource_id: \"1\"\n",
            "title: \"x\nand more\"\nauthor: @b\nsource_system: x\nsource_id: \"1\"\n",
            "title: |\n  line one\n  line two\nauthor: @b\nsource_system: x\nsource_id: \"1\"\n",
            "author: @b\ntitle: >-\n  key: value\nsource_system: x\nsource_id: \"1\"\n",
            "author: @b\nsource_system: x\nsource_id: \"1\"\ntitle: >-\n  key: value\n",
        ] {
            assert!(unreadable(inner), "{inner:?}");
            assert_eq!(key_of(inner), None);
        }
    }

    #[test]
    fn a_frontmatter_the_parser_does_not_accept_without_its_title_is_unreadable() {
        for inner in [
            // a quote, a flow sequence and a flow mapping left open
            "title: @alice\nnote: \"open\nsource_system: x\nsource_id: \"1\"\n",
            "title: @alice\nnote: \"open\\\"\nsource_system: x\nsource_id: \"1\"\n",
            "title: @alice\nnote: 'open\nsource_system: x\nsource_id: \"1\"\n",
            "title: @alice\ntags: [a,\nsource_system: x\nsource_id: \"1\"\n",
            "title: @alice\ntags: [a, \"]\nsource_system: x\nsource_id: \"1\"\n",
            "title: @alice\nnote: 'it''s open\nsource_system: x\nsource_id: \"1\"\n",
            "title: @alice\nnote: {a: [b}\nsource_system: x\nsource_id: \"1\"\n",
            // a line that is not `key: value`
            "title: @alice\nsource_system:x\nsource_id: \"1\"\n",
            // an identity that is missing, or is not one scalar
            "title: @alice\nsource_system: x\nsource_id: \"1\" \"2\"\n",
            "title: @alice\nsource_system: x\nsource_id: [1]\n",
            "title: @alice\nsource_system: x\n",
            // more than one document
            "title: @alice\nsource_system: x\nsource_id: 1\n...\nsource_id: 2\n",
            // valid YAML that is not a mapping
            "- a\n- b\n",
            "just text\n",
            "",
        ] {
            assert!(unreadable(inner), "{inner:?}");
        }
        // What the parser does accept once the title is out: a comment, and
        // an empty block scalar (the lines after it are top-level keys).
        for inner in [
            "title: @alice\n# a comment\nsource_system: x # tweets\nsource_id: \"1\"\n",
            "title: @alice\nnote: |\nsource_system: x\nsource_id: \"1\"\n",
        ] {
            assert_eq!(key_of(inner).as_deref(), Some("x:1"), "{inner:?}");
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
