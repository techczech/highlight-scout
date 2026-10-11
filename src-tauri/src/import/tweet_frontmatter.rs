//! Reading a work file's frontmatter to find tweets by key, and the one
//! edit the tweet importer makes to it (`updated_at`).
//!
//! Identity is read with a YAML parse, so quoting, comments and the order of
//! the fields do not matter. The app's own writer is not a strict YAML
//! emitter (a title that opens with `@`, or holds a backslash inside
//! quotes, is not valid YAML), so a frontmatter the parser rejects is read
//! by its `key: value` lines instead. That second reading only ever applies
//! to text that is not valid YAML.

use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use yaml_rust2::{Yaml, YamlLoader};

/// The frontmatter of a work file, read for its scalar fields.
pub struct Frontmatter {
    yaml: Option<Yaml>,
    lines: Vec<(String, String)>,
}

impl Frontmatter {
    /// From the text between the `---` lines.
    pub fn parse(inner: &str) -> Frontmatter {
        let yaml = YamlLoader::load_from_str(inner)
            .ok()
            .and_then(|docs| docs.into_iter().next())
            .filter(|d| matches!(d, Yaml::Hash(_)));
        let lines = if yaml.is_some() {
            vec![]
        } else {
            lenient_lines(inner)
        };
        Frontmatter { yaml, lines }
    }

    /// From a whole file's text; None when it does not open with frontmatter.
    pub fn of_file(content: &str) -> Option<Frontmatter> {
        inner_text(content).map(|t| Frontmatter::parse(&t))
    }

    /// A top-level scalar field as text. None for a missing, empty or
    /// non-scalar value.
    pub fn get(&self, field: &str) -> Option<String> {
        let v = match &self.yaml {
            Some(doc) => scalar(&doc[field])?,
            None => self
                .lines
                .iter()
                .find(|(k, _)| k == field)
                .map(|(_, v)| v.clone())?,
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

/// `key: value` lines at column 0, values unquoted, for a frontmatter that
/// is not valid YAML.
fn lenient_lines(inner: &str) -> Vec<(String, String)> {
    inner
        .lines()
        .filter_map(|l| {
            let (k, v) = l.split_once(':')?;
            if k.is_empty() || k.starts_with(char::is_whitespace) {
                return None;
            }
            let v = v.trim();
            let unquoted = ['"', '\'']
                .iter()
                .find_map(|q| v.strip_prefix(*q).and_then(|r| r.strip_suffix(*q)))
                .unwrap_or(v);
            Some((k.trim().to_string(), unquoted.to_string()))
        })
        .collect()
}

/// The text between a file's opening `---` line and the next `---` line,
/// whatever the line endings.
fn inner_text(content: &str) -> Option<String> {
    let mut lines = content.lines();
    if lines.next()?.trim_end() != "---" {
        return None;
    }
    let mut inner = String::new();
    for line in lines {
        if line.trim_end() == "---" {
            return Some(inner);
        }
        inner.push_str(line);
        inner.push('\n');
    }
    None
}

/// A file's frontmatter, reading no further into the file than its end.
fn read(path: &Path) -> Option<Frontmatter> {
    let mut reader = BufReader::new(fs::File::open(path).ok()?);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    if line.trim_end() != "---" {
        return None;
    }
    let mut inner = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line).ok()? == 0 {
            return None;
        }
        if line.trim_end() == "---" {
            return Some(Frontmatter::parse(&inner));
        }
        inner.push_str(line.trim_end_matches(['\n', '\r']));
        inner.push('\n');
    }
}

/// Every work file in `dir` whose frontmatter names `source_system`, by its
/// `source_system:source_id` key, paths in name order. The frontmatter of
/// every `.md` file is parsed; nothing is filtered out beforehand.
pub fn scan(dir: &Path, source_system: &str) -> HashMap<String, Vec<PathBuf>> {
    let mut by_key: HashMap<String, Vec<PathBuf>> = HashMap::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return by_key;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".md") || crate::archive_meta::is_temp_write_name(&name) {
            continue;
        }
        let path = entry.path();
        let Some(fm) = read(&path) else { continue };
        if fm.get("source_system").as_deref() != Some(source_system) {
            continue;
        }
        if let Some(key) = fm.key() {
            by_key.entry(key).or_default().push(path);
        }
    }
    for paths in by_key.values_mut() {
        paths.sort();
    }
    by_key
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

    #[test]
    fn identity_is_found_however_the_yaml_spells_it() {
        for inner in [
            "title: A\nsource_system: x\nsource_id: \"1904944923159445914\"\n",
            "source_id: '1904944923159445914'\ntitle: A\nsource_system: \"x\"\n",
            "source_system: x # the tweet importer\nsource_id: 1904944923159445914 # numeric\n",
            "\"source_system\": 'x'\n\"source_id\": >-\n  1904944923159445914\n",
            // Not valid YAML (the app writes such titles): read by its lines.
            "title: @alice said\nsource_system: x\nsource_id: \"1904944923159445914\"\n",
            "title: \"a \\ b\"\nsource_system: 'x'\nsource_id: '1904944923159445914'\n",
        ] {
            assert_eq!(
                Frontmatter::parse(inner).key().as_deref(),
                Some("x:1904944923159445914"),
                "{inner}"
            );
        }
        assert_eq!(
            Frontmatter::parse("title: A\nsource_system: x\n").key(),
            None
        );
        // A field inside a block scalar is text, not identity.
        let fm = Frontmatter::parse(
            "title: |\n  source_system: x\n  source_id: 5\nsource_system: zotero\nsource_id: ABC\n",
        );
        assert_eq!(fm.key().as_deref(), Some("zotero:ABC"));
    }

    #[test]
    fn frontmatter_is_read_with_either_line_ending_and_only_when_closed() {
        let fm = Frontmatter::of_file(
            "---\r\nsource_system: x\r\nsource_id: \"7\"\r\n---\r\n\r\n> q\r\n",
        )
        .unwrap();
        assert_eq!(fm.key().as_deref(), Some("x:7"));
        assert!(Frontmatter::of_file("no frontmatter\n").is_none());
        assert!(Frontmatter::of_file("---\nsource_system: x\n").is_none());
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
