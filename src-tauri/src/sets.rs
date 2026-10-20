//! Sets (HS-2A): a hand-picked, ordered collection of passages for a talk,
//! kept as one Markdown file per set in `<archive_path>/sets/<id>.md`. That
//! file is the only state a set has; it is git-tracked with the archive.
//!
//! This module is the seam for everything about a set that is not the engine:
//! - the file format: [`parse`] and [`render`] are exact inverses on any file
//!   this module wrote (byte-stable: order, notes, quotes survive a reopen);
//!   frontmatter keys and prose the user adds are kept verbatim,
//! - the store: [`SetStore`] reads and writes only inside the real
//!   `<archive>/sets` folder (ids are validated, links are refused, writes
//!   are a private temp file + rename, each set's changes are serialised
//!   by [`SetStore::update`]),
//! - editing: [`apply_edit`] (move, note, remove, pin, rename; a removed
//!   saved-search match is excluded from later refreshes),
//! - resolving items against the engine through the [`Lookup`] trait: by
//!   passage id first, then by `verify-quote` on the stored quote (repairing
//!   the id), else "changed", keeping the stored quote ([`resolve`]); a
//!   passage counts as the item only when its text equals the stored quote
//!   (whitespace aside),
//! - saved-search sets: [`refresh_saved`] (pinned items stay, unpinned items
//!   follow the query),
//! - "more like these": [`seeds`] and [`filter_suggestions`] (the engine's
//!   `similar` supplies the candidates; dismissed and already-present
//!   passages are removed).
//!
//! No search, citation or similarity logic lives here: those are the
//! engine's. Sources and indexes are never written.

use scout_corpus::similar::SimilarResults;
use scout_corpus::CitedPassage;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

/// The engine takes 1-10 seeds.
pub const MAX_SEEDS: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SetKind {
    Hand,
    SavedSearch,
}

impl SetKind {
    fn word(self) -> &'static str {
        match self {
            SetKind::Hand => "hand",
            SetKind::SavedSearch => "saved-search",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetItem {
    /// The engine's passage id, `<corpus>:<path>:<line>`.
    pub passage: String,
    pub corpus: String,
    /// The passage's file within its corpus (`rel_path`).
    pub source_path: String,
    /// The original text when the item was added; never rewritten.
    pub quote: String,
    /// Saved-search sets: a pinned item stays when the matches are refreshed.
    pub pinned: bool,
    pub note: String,
    /// Anything else the user wrote in the item (prose, own `- key:` lines),
    /// kept verbatim and written back after the quote.
    #[serde(default)]
    pub extra: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetFile {
    pub title: String,
    pub created: String,
    pub updated: String,
    pub kind: SetKind,
    /// Saved-search sets only.
    pub query: Option<String>,
    pub items: Vec<SetItem>,
    /// Passage ids the user rejected as suggestions; they do not return.
    pub dismissed: Vec<String>,
    /// Saved-search sets: passage ids the user removed; a refresh does not bring them back.
    pub excluded: Vec<String>,
    /// Frontmatter lines this module does not know (the user's own keys,
    /// comments), kept verbatim and written back before the closing `---`.
    pub front_extra: Vec<String>,
    /// Prose between the frontmatter and the first item, kept verbatim.
    pub preamble: String,
}

impl SetFile {
    pub fn new(title: &str, kind: SetKind, query: Option<String>, today: &str) -> SetFile {
        SetFile {
            title: title.trim().to_string(),
            created: today.to_string(),
            updated: today.to_string(),
            kind,
            query,
            items: vec![],
            dismissed: vec![],
            excluded: vec![],
            front_extra: vec![],
            preamble: String::new(),
        }
    }
}

// ---------- the file format ----------

fn js(s: &str) -> String {
    serde_json::to_string(s).expect("a string always serialises")
}

fn unjs(v: &str) -> Result<String, String> {
    serde_json::from_str::<String>(v.trim()).map_err(|_| format!("expected a quoted string, found {v:?}"))
}

/// Lines without the blank lines at either end, joined again.
fn trim_blank_lines(lines: &[String]) -> String {
    let first = lines.iter().position(|l| !l.trim().is_empty());
    let last = lines.iter().rposition(|l| !l.trim().is_empty());
    match (first, last) {
        (Some(a), Some(b)) => lines[a..=b].join("\n"),
        _ => String::new(),
    }
}

/// The set as its file text. Item numbers are written for the reader and
/// ignored on parse; the order of the items is the file's order. What the
/// user wrote that this module does not own (frontmatter keys, prose) is
/// written back verbatim.
pub fn render(s: &SetFile) -> String {
    let mut o = String::new();
    o.push_str("---\n");
    o.push_str(&format!("title: {}\n", js(&s.title)));
    o.push_str(&format!("created: {}\n", s.created));
    o.push_str(&format!("updated: {}\n", s.updated));
    o.push_str(&format!("kind: {}\n", s.kind.word()));
    if let Some(q) = &s.query {
        o.push_str(&format!("query: {}\n", js(q)));
    }
    for (key, list) in [("dismissed", &s.dismissed), ("excluded", &s.excluded)] {
        if !list.is_empty() {
            o.push_str(&format!("{key}:\n"));
            for d in list {
                o.push_str(&format!("  - {}\n", js(d)));
            }
        }
    }
    for l in &s.front_extra {
        o.push_str(l);
        o.push('\n');
    }
    o.push_str("---\n");
    if !s.preamble.is_empty() {
        o.push('\n');
        o.push_str(&s.preamble);
        o.push('\n');
    }
    for (i, it) in s.items.iter().enumerate() {
        o.push_str(&format!("\n## {}\n\n", i + 1));
        o.push_str(&format!("- passage: {}\n", js(&it.passage)));
        o.push_str(&format!("- corpus: {}\n", it.corpus));
        o.push_str(&format!("- source_path: {}\n", js(&it.source_path)));
        if it.pinned {
            o.push_str("- pinned: true\n");
        }
        if !it.note.is_empty() {
            o.push_str(&format!("- note: {}\n", js(&it.note)));
        }
        o.push('\n');
        for line in it.quote.split('\n') {
            if line.is_empty() {
                o.push_str(">\n");
            } else {
                o.push_str(&format!("> {line}\n"));
            }
        }
        if !it.extra.is_empty() {
            o.push('\n');
            o.push_str(&it.extra);
            o.push('\n');
        }
    }
    o
}

/// An item heading: `## ` and the item's number.
fn item_heading(l: &str) -> bool {
    l.strip_prefix("## ").is_some_and(|n| !n.trim().is_empty() && n.trim().chars().all(|c| c.is_ascii_digit()))
}

/// Read a set file. Frontmatter keys and prose this module does not own are
/// kept (see [`SetFile::front_extra`], [`SetFile::preamble`],
/// [`SetItem::extra`]); a malformed item is an error naming it (a half-read
/// set would be rewritten over the file).
pub fn parse(text: &str) -> Result<SetFile, String> {
    let mut lines = text.lines();
    if lines.next().map(str::trim_end) != Some("---") {
        return Err("a set file starts with ---".into());
    }
    let mut set = SetFile::new("", SetKind::Hand, None, "");
    #[derive(PartialEq)]
    enum List {
        None,
        Dismissed,
        Excluded,
        Extra,
    }
    let mut list = List::None;
    let mut closed = false;
    for l in lines.by_ref() {
        if l.trim_end() == "---" {
            closed = true;
            break;
        }
        if let Some(rest) = l.strip_prefix("  - ") {
            match list {
                List::Dismissed => {
                    set.dismissed.push(unjs(rest)?);
                    continue;
                }
                List::Excluded => {
                    set.excluded.push(unjs(rest)?);
                    continue;
                }
                _ => {}
            }
        }
        // An unknown key's own continuation lines (indented, list items, blank) stay with it.
        if list == List::Extra && (l.starts_with(' ') || l.starts_with('\t') || l.starts_with("- ") || l.trim().is_empty()) {
            set.front_extra.push(l.to_string());
            continue;
        }
        list = List::None;
        let known = l.split_once(':').filter(|(k, _)| {
            matches!(*k, "title" | "created" | "updated" | "kind" | "query" | "dismissed" | "excluded")
        });
        let Some((k, v)) = known else {
            set.front_extra.push(l.to_string());
            list = List::Extra;
            continue;
        };
        let v = v.trim();
        match k {
            "title" => set.title = unjs(v)?,
            "created" => set.created = v.to_string(),
            "updated" => set.updated = v.to_string(),
            "kind" => {
                set.kind = match v {
                    "hand" => SetKind::Hand,
                    "saved-search" => SetKind::SavedSearch,
                    other => return Err(format!("unknown set kind {other:?}")),
                }
            }
            "query" => set.query = Some(unjs(v)?),
            "dismissed" => list = List::Dismissed,
            "excluded" => list = List::Excluded,
            _ => unreachable!("filtered above"),
        }
    }
    if !closed {
        return Err("the set's frontmatter is not closed".into());
    }
    if set.title.is_empty() {
        return Err("the set has no title".into());
    }

    #[derive(PartialEq, Clone, Copy)]
    enum Phase {
        Meta,
        Quote,
        After,
    }
    struct Cur {
        no: usize,
        it: SetItem,
        quote: Vec<String>,
        extra: Vec<String>,
        phase: Phase,
    }
    let finish = |c: Option<Cur>, set: &mut SetFile| -> Result<(), String> {
        if let Some(Cur { no, mut it, quote, extra, .. }) = c {
            if it.passage.is_empty() {
                return Err(format!("item {no} has no passage"));
            }
            it.quote = quote.join("\n");
            it.extra = trim_blank_lines(&extra);
            set.items.push(it);
        }
        Ok(())
    };
    let mut preamble: Vec<String> = vec![];
    let mut cur: Option<Cur> = None;
    let mut n = 0usize;
    for l in lines {
        if item_heading(l) {
            finish(cur.take(), &mut set)?;
            n += 1;
            cur = Some(Cur { no: n, it: SetItem::default(), quote: vec![], extra: vec![], phase: Phase::Meta });
            continue;
        }
        let Some(c) = cur.as_mut() else {
            preamble.push(l.to_string());
            continue;
        };
        let quote_line = if l == ">" { Some("") } else { l.strip_prefix("> ") };
        match c.phase {
            Phase::Meta => {
                if let Some(q) = quote_line {
                    c.phase = Phase::Quote;
                    c.quote.push(q.to_string());
                    continue;
                }
                if l.trim().is_empty() {
                    continue;
                }
                let no = c.no;
                let meta = l.strip_prefix("- ").and_then(|r| r.split_once(':'));
                match meta {
                    Some(("passage", v)) => c.it.passage = unjs(v).map_err(|e| format!("item {no}: {e}"))?,
                    Some(("corpus", v)) => c.it.corpus = v.trim().to_string(),
                    Some(("source_path", v)) => c.it.source_path = unjs(v).map_err(|e| format!("item {no}: {e}"))?,
                    Some(("pinned", v)) => c.it.pinned = v.trim() == "true",
                    Some(("note", v)) => c.it.note = unjs(v).map_err(|e| format!("item {no}: {e}"))?,
                    _ => c.extra.push(l.to_string()),
                }
            }
            Phase::Quote => {
                if let Some(q) = quote_line {
                    c.quote.push(q.to_string());
                } else {
                    c.phase = Phase::After;
                    c.extra.push(l.to_string());
                }
            }
            Phase::After => c.extra.push(l.to_string()),
        }
    }
    finish(cur.take(), &mut set)?;
    set.preamble = trim_blank_lines(&preamble);
    Ok(set)
}

// ---------- the store: only inside <archive>/sets/ ----------

/// Summary row for the rail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SetSummary {
    pub id: String,
    pub title: String,
    pub kind: SetKind,
    pub items: usize,
    pub updated: String,
}

/// The store of one archive's sets. Every path it touches is checked: the
/// `sets` folder must be a real folder directly in the (canonical) archive,
/// set files and temp files are never links (`lstat`, `O_NOFOLLOW`,
/// `O_EXCL`), and every change to one set runs under that set's lock
/// (load, edit and save together), so two edits never interleave.
pub struct SetStore {
    archive: PathBuf,
}

/// A set id is the file's stem: lower-case letters, digits, `-`, `_`.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// One lock per set file, shared by every store in the process.
fn set_lock(key: &Path) -> Arc<Mutex<()>> {
    static LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();
    let mut m = LOCKS.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    m.entry(key.to_path_buf()).or_default().clone()
}

fn is_link(p: &Path) -> bool {
    fs::symlink_metadata(p).map(|m| m.file_type().is_symlink()).unwrap_or(false)
}

/// Open for reading without following a link.
fn read_nofollow(p: &Path) -> std::io::Result<String> {
    let mut f = fs::OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW).open(p)?;
    let mut s = String::new();
    f.read_to_string(&mut s)?;
    Ok(s)
}

/// Create a file that must not exist yet (`O_EXCL`; never through a link).
fn create_exclusive(p: &Path) -> std::io::Result<fs::File> {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .custom_flags(libc::O_NOFOLLOW)
        .mode(0o644)
        .open(p)
}

impl SetStore {
    /// The store of an archive: `<archive_path>/sets`.
    pub fn new(archive_path: &str) -> SetStore {
        SetStore { archive: PathBuf::from(archive_path) }
    }

    /// The canonical `<archive>/sets`, refused when it is a link or not a
    /// folder; created when `create` and missing.
    fn dir(&self, create: bool) -> Result<PathBuf, String> {
        let archive = fs::canonicalize(&self.archive).map_err(|e| format!("archive folder: {e}"))?;
        let dir = archive.join("sets");
        match fs::symlink_metadata(&dir) {
            Ok(m) if m.file_type().is_symlink() => {
                return Err("the archive's sets folder is a link: sets are kept only in a real folder".into())
            }
            Ok(m) if !m.is_dir() => return Err("the archive's sets is not a folder".into()),
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && create => match fs::create_dir(&dir) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return self.dir(false),
                Err(e) => return Err(format!("sets folder: {e}")),
            },
            Err(e) => return Err(format!("sets folder: {e}")),
        }
        let canon = fs::canonicalize(&dir).map_err(|e| format!("sets folder: {e}"))?;
        if canon != dir {
            return Err("the archive's sets folder resolves outside the archive".into());
        }
        Ok(canon)
    }

    /// `<dir>/<id>.md` for a valid id; refused when the file is a link or not a file.
    fn file(dir: &Path, id: &str) -> Result<PathBuf, String> {
        if !valid_id(id) {
            return Err(format!("{id:?} is not a set id"));
        }
        let p = dir.join(format!("{id}.md"));
        if let Ok(m) = fs::symlink_metadata(&p) {
            if m.file_type().is_symlink() {
                return Err(format!("set {id}: the file is a link; sets are only read and written as real files"));
            }
            if !m.is_file() {
                return Err(format!("set {id}: not a file"));
            }
        }
        if p.parent() != Some(dir) {
            return Err(format!("{id:?} is not a set id"));
        }
        Ok(p)
    }

    fn read(dir: &Path, id: &str) -> Result<SetFile, String> {
        let p = Self::file(dir, id)?;
        let text = read_nofollow(&p).map_err(|e| format!("set {id}: {e}"))?;
        parse(&text).map_err(|e| format!("set {id}: {e}"))
    }

    /// Write the set: a temp file of its own (random name, `O_EXCL`) in the
    /// sets folder, flushed, then renamed over the set file. The caller holds the set's lock.
    fn write(dir: &Path, id: &str, set: &SetFile) -> Result<(), String> {
        let p = Self::file(dir, id)?;
        let tmp = dir.join(format!(".{id}.{}.tmp", uuid::Uuid::new_v4().simple()));
        let result = (|| -> std::io::Result<()> {
            let mut f = create_exclusive(&tmp)?;
            f.write_all(render(set).as_bytes())?;
            f.sync_all()?;
            Ok(())
        })();
        if let Err(e) = result {
            let _ = fs::remove_file(&tmp);
            return Err(format!("set {id}: {e}"));
        }
        if is_link(&p) {
            let _ = fs::remove_file(&tmp);
            return Err(format!("set {id}: the file is a link; sets are only read and written as real files"));
        }
        fs::rename(&tmp, &p).map_err(|e| {
            let _ = fs::remove_file(&tmp);
            format!("set {id}: {e}")
        })
    }

    pub fn load(&self, id: &str) -> Result<SetFile, String> {
        let dir = self.dir(false)?;
        Self::read(&dir, id)
    }

    /// Replace the set's file with `set`, under the set's lock.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn save(&self, id: &str, set: &SetFile) -> Result<(), String> {
        let dir = self.dir(true)?;
        let p = Self::file(&dir, id)?;
        let lock = set_lock(&p);
        let _g = lock.lock().unwrap_or_else(|e| e.into_inner());
        Self::write(&dir, id, set)
    }

    /// Load, change and save one set as one step under its lock; the file is
    /// written only when `f` changed the set. An error from `f` writes nothing.
    pub fn update<T>(&self, id: &str, f: impl FnOnce(&mut SetFile) -> Result<T, String>) -> Result<T, String> {
        let dir = self.dir(false)?;
        let p = Self::file(&dir, id)?;
        let lock = set_lock(&p);
        let _g = lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut set = Self::read(&dir, id)?;
        let before = set.clone();
        let out = f(&mut set)?;
        if set != before {
            Self::write(&dir, id, &set)?;
        }
        Ok(out)
    }

    /// A new set under an unused id taken from its title. The name is
    /// claimed with an exclusive create, so two creations never share a file.
    pub fn create(&self, set: &SetFile) -> Result<String, String> {
        let dir = self.dir(true)?;
        let mut base: String = slug::slugify(&set.title);
        base.truncate(60);
        let base = base.trim_matches('-').to_string();
        let base = if base.is_empty() { "set".to_string() } else { base };
        let mut id = base.clone();
        let mut n = 1;
        let p = loop {
            let p = Self::file(&dir, &id)?;
            match create_exclusive(&p) {
                Ok(_) => break p,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    n += 1;
                    id = format!("{base}-{n}");
                }
                Err(e) => return Err(format!("set {id}: {e}")),
            }
        };
        let lock = set_lock(&p);
        let _g = lock.lock().unwrap_or_else(|e| e.into_inner());
        if let Err(e) = Self::write(&dir, &id, set) {
            let _ = fs::remove_file(&p);
            return Err(e);
        }
        Ok(id)
    }

    /// Every set, most recently updated first. Links are skipped.
    pub fn list(&self) -> Vec<SetSummary> {
        let Ok(dir) = self.dir(false) else { return vec![] };
        let Ok(rd) = fs::read_dir(&dir) else { return vec![] };
        let mut out: Vec<SetSummary> = rd
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                if !e.file_type().ok()?.is_file() {
                    return None;
                }
                let p = e.path();
                let id = p.file_stem()?.to_str()?.to_string();
                if p.extension()?.to_str()? != "md" || !valid_id(&id) {
                    return None;
                }
                let s = parse(&read_nofollow(&p).ok()?).ok()?;
                Some(SetSummary { id, title: s.title, kind: s.kind, items: s.items.len(), updated: s.updated })
            })
            .collect();
        out.sort_by(|a, b| b.updated.cmp(&a.updated).then(a.title.cmp(&b.title)));
        out
    }
}

// ---------- editing ----------

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case")]
pub enum SetEdit {
    /// Take the item at `from` and put it at `to` (both positions in the current order).
    Move { from: usize, to: usize },
    Note { index: usize, note: String },
    Remove { index: usize },
    Pin { index: usize, pinned: bool },
    Rename { title: String },
}

pub fn apply_edit(s: &mut SetFile, e: &SetEdit, today: &str) -> Result<(), String> {
    let n = s.items.len();
    let at = |i: usize| if i < n { Ok(i) } else { Err(format!("no item {}", i + 1)) };
    match e {
        SetEdit::Move { from, to } => {
            let (from, to) = (at(*from)?, at(*to)?);
            let it = s.items.remove(from);
            s.items.insert(to, it);
        }
        SetEdit::Note { index, note } => s.items[at(*index)?].note = note.trim().replace('\n', " "),
        SetEdit::Remove { index } => {
            let it = s.items.remove(at(*index)?);
            // A saved search would bring a removed match straight back.
            if s.kind == SetKind::SavedSearch && !s.excluded.contains(&it.passage) {
                s.excluded.push(it.passage);
            }
        }
        SetEdit::Pin { index, pinned } => s.items[at(*index)?].pinned = *pinned,
        SetEdit::Rename { title } => {
            let t = title.trim();
            if t.is_empty() {
                return Err("a set needs a title".into());
            }
            s.title = t.to_string();
        }
    }
    s.updated = today.to_string();
    Ok(())
}

/// The item for a passage the user adds: the whole passage, original text.
pub fn item_from_cited(c: &CitedPassage) -> SetItem {
    SetItem {
        passage: c.passage_id.clone(),
        corpus: c.corpus.clone(),
        source_path: c.rel_path.clone(),
        quote: c.quote.trim().to_string(),
        ..Default::default()
    }
}

/// Add an item at the end; `false` when the set already holds that passage.
pub fn add_item(s: &mut SetFile, it: SetItem, today: &str) -> bool {
    if s.items.iter().any(|x| x.passage == it.passage) {
        return false;
    }
    s.dismissed.retain(|d| *d != it.passage);
    s.excluded.retain(|d| *d != it.passage);
    s.items.push(it);
    s.updated = today.to_string();
    true
}

/// Keep a rejected suggestion from coming back.
pub fn dismiss(s: &mut SetFile, passage: &str, today: &str) {
    if !s.dismissed.iter().any(|d| d == passage) {
        s.dismissed.push(passage.to_string());
        s.updated = today.to_string();
    }
}

/// Saved-search sets: pinned items stay where they are; the unpinned items
/// become `matches` (those not already pinned, in match order, neither
/// dismissed nor removed by the user), after the pinned ones. Returns
/// whether anything changed.
pub fn refresh_saved(s: &mut SetFile, matches: Vec<SetItem>, cap: usize, today: &str) -> bool {
    if s.kind != SetKind::SavedSearch {
        return false;
    }
    let before = s.items.clone();
    let mut items: Vec<SetItem> = s.items.iter().filter(|i| i.pinned).cloned().collect();
    // Unpinned items the user already annotated keep their note when they still match.
    for m in matches {
        if items.len() >= cap.max(before.iter().filter(|i| i.pinned).count()) {
            break;
        }
        if items.iter().any(|i| i.passage == m.passage) || s.dismissed.contains(&m.passage) || s.excluded.contains(&m.passage) {
            continue;
        }
        // A still-matching item keeps what the user wrote on it.
        let (note, extra) = before
            .iter()
            .find(|i| i.passage == m.passage)
            .map(|i| (i.note.clone(), i.extra.clone()))
            .unwrap_or_default();
        items.push(SetItem { note, extra, ..m });
    }
    let changed = items != before;
    if changed {
        s.items = items;
        s.updated = today.to_string();
    }
    changed
}

// ---------- resolving items against the engine ----------

/// What the engine says of one passage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub passage_id: String,
    pub rel_path: String,
    pub quote: String,
}

/// The two engine calls resolution needs; the engine implements it (see
/// `commands::sets`), tests use a table.
pub trait Lookup {
    /// `scout cite`: the passage now at this id, if there is one.
    fn cite(&self, passage_id: &str) -> Option<Found>;
    /// `scout verify-quote` in one corpus: every passage holding this text.
    fn find_quote(&self, corpus: &str, quote: &str) -> Vec<Found>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// The id resolves and the passage still holds the quote.
    Ok,
    /// The id did not hold the quote any more; found through the quote, id repaired.
    Repaired,
    /// Neither: "source changed". The stored quote is all there is.
    Changed,
}

fn norm(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A passage is the item's passage only when its text is the stored quote,
/// whitespace aside. A shortened, grown or empty passage is not: the item
/// then shows "source changed" with the quote the user saved.
fn same_text(passage: &str, quote: &str) -> bool {
    let (p, q) = (norm(passage), norm(quote));
    !q.is_empty() && p == q
}

/// Where an item's passage is now: by id first, then by its quote.
pub fn resolve(it: &SetItem, l: &dyn Lookup) -> (Status, Option<Found>) {
    if let Some(f) = l.cite(&it.passage) {
        if same_text(&f.quote, &it.quote) {
            return (Status::Ok, Some(f));
        }
    }
    let mut found = l.find_quote(&it.corpus, &it.quote);
    // The same file first: a moved line, not another document with the same words.
    found.sort_by_key(|f| f.rel_path != it.source_path);
    for f in found {
        if let Some(c) = l.cite(&f.passage_id) {
            if same_text(&c.quote, &it.quote) {
                return (Status::Repaired, Some(c));
            }
        }
    }
    (Status::Changed, None)
}

/// Write a repair back into the item.
pub fn apply_repair(it: &mut SetItem, f: &Found) {
    it.passage = f.passage_id.clone();
    it.source_path = f.rel_path.clone();
}

// ---------- more like these ----------

/// The set's seeds for the engine: the first ten items that resolved.
pub fn seeds(resolved: &[(Status, String)]) -> Vec<String> {
    resolved
        .iter()
        .filter(|(s, _)| *s != Status::Changed)
        .map(|(_, id)| id.clone())
        .take(MAX_SEEDS)
        .collect()
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Suggestion {
    pub passage_id: String,
    pub corpus: String,
    pub rel_path: String,
    pub quote: String,
    pub title: String,
    pub author: Option<String>,
    pub date_display: Option<String>,
    /// The terms the passage shares with the seeds ("why").
    pub shared_terms: Vec<String>,
    /// The title of the seed it resembles most.
    pub similar_to: String,
    pub score: f64,
}

/// The engine's suggestions in the shape the page shows.
pub fn suggestions_from(r: &SimilarResults) -> Vec<Suggestion> {
    r.results
        .iter()
        .map(|s| {
            let seed = r.seeds.iter().find(|x| x.passage_id == s.closest_seed);
            Suggestion {
                passage_id: s.passage_id.clone(),
                corpus: s.corpus.clone(),
                rel_path: s.rel_path.clone(),
                quote: s.quote.clone(),
                title: s.title.clone(),
                author: s.author.clone(),
                date_display: s.date_display.clone(),
                shared_terms: s.shared_terms.clone(),
                similar_to: seed.map(|x| x.title.clone()).unwrap_or_else(|| s.closest_seed.clone()),
                score: s.score,
            }
        })
        .collect()
}

/// Suggestions minus what the set already holds (by id or by text) and what
/// the user dismissed.
pub fn filter_suggestions(all: Vec<Suggestion>, s: &SetFile) -> Vec<Suggestion> {
    all.into_iter()
        .filter(|g| {
            !s.dismissed.contains(&g.passage_id)
                && !s.items.iter().any(|i| i.passage == g.passage_id || norm(&i.quote) == norm(&g.quote))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn item(p: &str, q: &str) -> SetItem {
        let corpus = p.split(':').next().unwrap().to_string();
        let path = p.split(':').nth(1).unwrap().to_string();
        SetItem { passage: p.into(), corpus, source_path: path, quote: q.into(), ..Default::default() }
    }

    fn sample() -> SetFile {
        let mut s = SetFile::new("Metaphor talk", SetKind::Hand, None, "2026-10-20");
        let mut a = item("writing:essays/a.md:12", "Open paths.\n\nA second paragraph with: colons, \"quotes\" and — dashes.");
        a.note = "Open the talk with this".into();
        let mut b = item("highlights:readings/works/b.md:7", "Metaphors are very powerful.");
        b.pinned = true;
        s.items = vec![a, b];
        s.dismissed = vec!["writing:x.md:3".into(), "tweets:t.md:9".into()];
        s
    }

    #[test]
    fn a_set_file_round_trips_byte_for_byte() {
        let s = sample();
        let text = render(&s);
        let back = parse(&text).unwrap();
        assert_eq!(back, s);
        assert_eq!(render(&back), text, "render(parse(x)) == x");
        // Reordering and noting, then reopening, keeps both.
        let mut s2 = back;
        apply_edit(&mut s2, &SetEdit::Move { from: 1, to: 0 }, "2026-10-21").unwrap();
        apply_edit(&mut s2, &SetEdit::Note { index: 0, note: "Then this".into() }, "2026-10-21").unwrap();
        let t2 = render(&s2);
        let again = parse(&t2).unwrap();
        assert_eq!(again.items[0].passage, "highlights:readings/works/b.md:7");
        assert_eq!(again.items[0].note, "Then this");
        assert_eq!(again.updated, "2026-10-21");
        assert_eq!(render(&again), t2);
    }

    #[test]
    fn saved_search_sets_keep_their_query() {
        let s = SetFile::new("Scaffolding", SetKind::SavedSearch, Some("scaffolding \"zone of\"".into()), "2026-10-20");
        let back = parse(&render(&s)).unwrap();
        assert_eq!(back.kind, SetKind::SavedSearch);
        assert_eq!(back.query.as_deref(), Some("scaffolding \"zone of\""));
    }

    #[test]
    fn the_format_is_golden() {
        let mut s = SetFile::new("Talk", SetKind::Hand, None, "2026-10-20");
        let mut a = item("writing:a.md:3", "Line one\n\nLine two");
        a.note = "n".into();
        s.items = vec![a];
        assert_eq!(
            render(&s),
            "---\ntitle: \"Talk\"\ncreated: 2026-10-20\nupdated: 2026-10-20\nkind: hand\n---\n\n## 1\n\n- passage: \"writing:a.md:3\"\n- corpus: writing\n- source_path: \"a.md\"\n- note: \"n\"\n\n> Line one\n>\n> Line two\n"
        );
    }

    #[test]
    fn a_malformed_set_is_an_error_not_a_blank_set() {
        assert!(parse("no frontmatter").is_err());
        assert!(parse("---\ntitle: \"x\"\n").is_err());
        assert!(parse("---\ntitle: \"x\"\nkind: weird\n---\n").is_err());
        assert!(parse("---\ntitle: \"x\"\n---\n\n## 1\n\n- corpus: writing\n\n> q\n").is_err());
    }

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("hs-sets-{name}-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn the_store_writes_only_inside_the_sets_dir() {
        let root = tmp("store");
        let store = SetStore::new(root.to_str().unwrap());
        let id = store.create(&sample()).unwrap();
        assert_eq!(id, "metaphor-talk");
        assert!(root.join("sets/metaphor-talk.md").is_file());
        // A second set with the same title gets its own file.
        assert_eq!(store.create(&sample()).unwrap(), "metaphor-talk-2");
        // Ids that could leave the dir are refused, for read and write.
        for bad in ["../x", "a/b", "..", "", "A", "x.md", "a b"] {
            assert!(store.load(bad).is_err(), "{bad}");
            assert!(store.save(bad, &sample()).is_err(), "{bad}");
        }
        // Nothing but the two sets was written anywhere under the root.
        let mut all: Vec<String> = vec![];
        fn walk(p: &Path, root: &Path, out: &mut Vec<String>) {
            for e in fs::read_dir(p).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() { walk(&p, root, out) } else { out.push(p.strip_prefix(root).unwrap().to_string_lossy().into()) }
            }
        }
        walk(&root, &root, &mut all);
        all.sort();
        assert_eq!(all, vec!["sets/metaphor-talk-2.md", "sets/metaphor-talk.md"]);
        let list = store.list();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].items, 2);
        let reopened = store.load("metaphor-talk").unwrap();
        assert_eq!(reopened, sample());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn an_unusual_title_still_gets_a_valid_id() {
        let root = tmp("slug");
        let store = SetStore::new(root.to_str().unwrap());
        let s = SetFile::new("../../Étude: «Ünï»!", SetKind::Hand, None, "2026-10-20");
        let id = store.create(&s).unwrap();
        assert!(valid_id(&id), "{id}");
        assert!(root.join("sets").join(format!("{id}.md")).is_file());
        let s = SetFile::new("???", SetKind::Hand, None, "2026-10-20");
        assert_eq!(store.create(&s).unwrap(), "set");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn edits_move_note_remove_pin_rename() {
        let mut s = sample();
        s.items.push(item("tweets:t.md:1", "A tweet"));
        apply_edit(&mut s, &SetEdit::Move { from: 2, to: 0 }, "d").unwrap();
        assert_eq!(s.items.iter().map(|i| i.corpus.as_str()).collect::<Vec<_>>(), ["tweets", "writing", "highlights"]);
        apply_edit(&mut s, &SetEdit::Remove { index: 1 }, "d").unwrap();
        assert_eq!(s.items.len(), 2);
        apply_edit(&mut s, &SetEdit::Pin { index: 0, pinned: true }, "d").unwrap();
        assert!(s.items[0].pinned);
        apply_edit(&mut s, &SetEdit::Note { index: 1, note: "  two\nlines ".into() }, "d").unwrap();
        assert_eq!(s.items[1].note, "two lines");
        apply_edit(&mut s, &SetEdit::Rename { title: " New ".into() }, "d").unwrap();
        assert_eq!(s.title, "New");
        assert!(apply_edit(&mut s, &SetEdit::Remove { index: 5 }, "d").is_err());
        assert!(apply_edit(&mut s, &SetEdit::Rename { title: " ".into() }, "d").is_err());
    }

    #[test]
    fn adding_twice_adds_once_and_clears_a_dismissal() {
        let mut s = sample();
        assert!(!add_item(&mut s, item("writing:essays/a.md:12", "x"), "d"));
        assert!(add_item(&mut s, item("writing:x.md:3", "Another"), "d"));
        assert!(!s.dismissed.contains(&"writing:x.md:3".to_string()));
        dismiss(&mut s, "writing:y.md:1", "d");
        dismiss(&mut s, "writing:y.md:1", "d");
        assert_eq!(s.dismissed.iter().filter(|d| *d == "writing:y.md:1").count(), 1);
    }

    #[derive(Default)]
    struct Table {
        by_id: HashMap<String, Found>,
        by_quote: Vec<(String, Found)>,
    }
    impl Lookup for Table {
        fn cite(&self, id: &str) -> Option<Found> {
            self.by_id.get(id).cloned()
        }
        fn find_quote(&self, _c: &str, q: &str) -> Vec<Found> {
            self.by_quote.iter().filter(|(t, _)| t == q).map(|(_, f)| f.clone()).collect()
        }
    }
    fn found(id: &str, path: &str, q: &str) -> Found {
        Found { passage_id: id.into(), rel_path: path.into(), quote: q.into() }
    }

    #[test]
    fn an_id_that_still_holds_the_quote_resolves() {
        let it = item("writing:a.md:3", "The quick fox");
        let mut t = Table::default();
        t.by_id.insert(it.passage.clone(), found("writing:a.md:3", "a.md", "The  quick fox\n"));
        let (st, f) = resolve(&it, &t);
        assert_eq!(st, Status::Ok);
        assert_eq!(f.unwrap().passage_id, "writing:a.md:3");
    }

    #[test]
    fn a_moved_line_is_repaired_through_the_quote() {
        let mut it = item("writing:a.md:3", "The quick fox");
        let mut t = Table::default();
        // Line 3 now holds other text; the quote sits at line 9 of the same file, and in another file too.
        t.by_id.insert("writing:a.md:3".into(), found("writing:a.md:3", "a.md", "Something else entirely"));
        t.by_id.insert("writing:a.md:9".into(), found("writing:a.md:9", "a.md", "The quick fox"));
        t.by_id.insert("writing:b.md:1".into(), found("writing:b.md:1", "b.md", "The quick fox"));
        t.by_quote = vec![
            ("The quick fox".into(), found("writing:b.md:1", "b.md", "")),
            ("The quick fox".into(), found("writing:a.md:9", "a.md", "")),
        ];
        let (st, f) = resolve(&it, &t);
        assert_eq!(st, Status::Repaired);
        let f = f.unwrap();
        assert_eq!(f.passage_id, "writing:a.md:9", "the same file wins");
        apply_repair(&mut it, &f);
        assert_eq!(it.passage, "writing:a.md:9");
        // The repaired set file round-trips with the new id and keeps the stored quote.
        let mut s = SetFile::new("T", SetKind::Hand, None, "d");
        s.items.push(it);
        assert_eq!(parse(&render(&s)).unwrap().items[0].quote, "The quick fox");
    }

    #[test]
    fn a_missing_id_is_repaired_and_a_lost_quote_is_reported_as_changed() {
        let it = item("writing:a.md:3", "The quick fox");
        let mut t = Table::default();
        t.by_id.insert("writing:a.md:20".into(), found("writing:a.md:20", "a.md", "The quick fox"));
        t.by_quote = vec![("The quick fox".into(), found("writing:a.md:20", "a.md", ""))];
        assert_eq!(resolve(&it, &t).0, Status::Repaired);
        let (st, f) = resolve(&it, &Table::default());
        assert_eq!(st, Status::Changed);
        assert!(f.is_none());
    }

    #[test]
    fn a_shortened_grown_or_empty_passage_is_source_changed() {
        let it = item("writing:a.md:3", "First sentence. Second sentence.");
        for now in ["First sentence.", "", "First sentence. Second sentence. A third."] {
            let mut t = Table::default();
            t.by_id.insert(it.passage.clone(), found("writing:a.md:3", "a.md", now));
            t.by_quote = vec![(it.quote.clone(), found("writing:a.md:3", "a.md", ""))];
            let (st, f) = resolve(&it, &t);
            assert_eq!(st, Status::Changed, "{now:?}");
            assert!(f.is_none());
        }
        // Whitespace alone is not a change.
        let mut t = Table::default();
        t.by_id.insert(it.passage.clone(), found("writing:a.md:3", "a.md", "First  sentence.\nSecond sentence. "));
        assert_eq!(resolve(&it, &t).0, Status::Ok);
    }

    #[test]
    fn a_removed_saved_search_match_stays_out_after_a_refresh() {
        let mut s = SetFile::new("S", SetKind::SavedSearch, Some("q".into()), "d");
        let matches = || vec![item("writing:a.md:1", "a"), item("writing:b.md:1", "b")];
        refresh_saved(&mut s, matches(), 20, "d");
        apply_edit(&mut s, &SetEdit::Remove { index: 0 }, "d").unwrap();
        assert_eq!(s.excluded, ["writing:a.md:1"]);
        refresh_saved(&mut s, matches(), 20, "d2");
        assert_eq!(s.items.iter().map(|i| i.passage.as_str()).collect::<Vec<_>>(), ["writing:b.md:1"]);
        // The exclusion is in the file.
        let back = parse(&render(&s)).unwrap();
        assert_eq!(back.excluded, ["writing:a.md:1"]);
        // Adding it by hand takes it back in.
        add_item(&mut s, item("writing:a.md:1", "a"), "d3");
        assert!(s.excluded.is_empty());
        // A hand set records no exclusions.
        let mut h = sample();
        apply_edit(&mut h, &SetEdit::Remove { index: 0 }, "d").unwrap();
        assert!(h.excluded.is_empty());
    }

    #[test]
    fn seeds_are_the_first_ten_resolved_items() {
        let rows: Vec<(Status, String)> = (0..14)
            .map(|i| (if i == 1 { Status::Changed } else { Status::Ok }, format!("w:{i}.md:1")))
            .collect();
        let s = seeds(&rows);
        assert_eq!(s.len(), 10);
        assert!(!s.contains(&"w:1.md:1".to_string()));
        assert_eq!(s[1], "w:2.md:1");
    }

    fn sug(id: &str, q: &str) -> Suggestion {
        Suggestion { passage_id: id.into(), corpus: "writing".into(), rel_path: "x.md".into(), quote: q.into(), title: "T".into(), author: None, date_display: None, shared_terms: vec!["metaphor".into()], similar_to: "Seed".into(), score: 0.5 }
    }

    #[test]
    fn suggestions_leave_out_what_is_in_the_set_or_was_dismissed() {
        let s = sample();
        let all = vec![
            sug("writing:essays/a.md:12", "different words"), // an item (by id)
            sug("writing:z.md:1", "Metaphors are very powerful."), // an item (by text)
            sug("writing:x.md:3", "dismissed one"),
            sug("writing:n.md:5", "new one"),
        ];
        let out = filter_suggestions(all, &s);
        assert_eq!(out.iter().map(|g| g.passage_id.as_str()).collect::<Vec<_>>(), ["writing:n.md:5"]);
    }

    #[test]
    fn approving_adds_and_dismissing_survives_a_reopen() {
        let root = tmp("suggest");
        let store = SetStore::new(root.to_str().unwrap());
        let id = store.create(&SetFile::new("Metaphor talk", SetKind::Hand, None, "d")).unwrap();
        let all = || vec![sug("writing:n.md:5", "new one"), sug("writing:m.md:2", "other one")];
        let mut s = store.load(&id).unwrap();
        assert_eq!(filter_suggestions(all(), &s).len(), 2);
        add_item(&mut s, item("writing:n.md:5", "new one"), "d");
        dismiss(&mut s, "writing:m.md:2", "d");
        store.save(&id, &s).unwrap();
        let reopened = store.load(&id).unwrap();
        assert!(filter_suggestions(all(), &reopened).is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    fn outside_file(root: &Path, body: &str) -> PathBuf {
        let p = root.join("outside").join("precious.md");
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, body).unwrap();
        p
    }

    #[test]
    fn a_linked_sets_folder_is_refused() {
        let root = tmp("link-dir");
        let archive = root.join("archive");
        fs::create_dir_all(&archive).unwrap();
        let elsewhere = root.join("elsewhere");
        fs::create_dir_all(&elsewhere).unwrap();
        std::os::unix::fs::symlink(&elsewhere, archive.join("sets")).unwrap();
        let store = SetStore::new(archive.to_str().unwrap());
        assert!(store.create(&sample()).is_err());
        assert!(store.save("talk", &sample()).is_err());
        assert!(store.load("talk").is_err());
        assert!(store.update("talk", |_| Ok(())).is_err());
        assert!(store.list().is_empty());
        assert_eq!(fs::read_dir(&elsewhere).unwrap().count(), 0, "nothing was written through the link");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_linked_set_file_is_refused_for_read_and_write() {
        let root = tmp("link-file");
        let store = SetStore::new(root.to_str().unwrap());
        let outside = outside_file(&root, &render(&sample()));
        fs::create_dir_all(root.join("sets")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("sets/talk.md")).unwrap();
        assert!(store.load("talk").is_err());
        assert!(store.save("talk", &SetFile::new("Other", SetKind::Hand, None, "d")).is_err());
        assert!(store.update("talk", |s| { s.title = "Changed".into(); Ok(()) }).is_err());
        assert!(store.list().is_empty(), "a linked file is not listed");
        assert_eq!(fs::read_to_string(&outside).unwrap(), render(&sample()), "the linked file is untouched");
        // The link itself is still a link (not replaced by a rename either).
        assert!(fs::symlink_metadata(root.join("sets/talk.md")).unwrap().file_type().is_symlink());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_planted_temp_link_is_never_written_through() {
        let root = tmp("link-tmp");
        let store = SetStore::new(root.to_str().unwrap());
        let id = store.create(&SetFile::new("Talk", SetKind::Hand, None, "d")).unwrap();
        let outside = outside_file(&root, "precious");
        // The temp name a fixed-name writer would use.
        std::os::unix::fs::symlink(&outside, root.join(format!("sets/.{id}.md.tmp"))).unwrap();
        store.save(&id, &sample()).unwrap();
        store.update(&id, |s| { s.title = "Changed".into(); Ok(()) }).unwrap();
        assert_eq!(fs::read_to_string(&outside).unwrap(), "precious");
        assert_eq!(store.load(&id).unwrap().title, "Changed");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn concurrent_updates_never_lose_an_edit_or_show_a_partial_file() {
        let root = tmp("concurrent");
        let store = std::sync::Arc::new(SetStore::new(root.to_str().unwrap()));
        let id = store.create(&SetFile::new("Talk", SetKind::Hand, None, "d")).unwrap();
        let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let reader = {
            let (store, id, done) = (store.clone(), id.clone(), done.clone());
            std::thread::spawn(move || {
                let mut reads = 0;
                while !done.load(std::sync::atomic::Ordering::SeqCst) {
                    store.load(&id).expect("every read sees a whole set file");
                    reads += 1;
                }
                reads
            })
        };
        let writers: Vec<_> = (0..12)
            .map(|i| {
                let (store, id) = (store.clone(), id.clone());
                std::thread::spawn(move || {
                    store
                        .update(&id, |s| {
                            // Long enough that unserialised load-edit-save would overlap.
                            std::thread::sleep(std::time::Duration::from_millis(15));
                            add_item(s, item(&format!("writing:w{i}.md:1"), &"long text ".repeat(2000)), "d");
                            Ok(())
                        })
                        .unwrap();
                })
            })
            .collect();
        for w in writers {
            w.join().unwrap();
        }
        done.store(true, std::sync::atomic::Ordering::SeqCst);
        assert!(reader.join().unwrap() > 0);
        assert_eq!(store.load(&id).unwrap().items.len(), 12, "no edit was lost");
        let leftovers: Vec<_> = fs::read_dir(root.join("sets")).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(leftovers.len(), 1, "no temp file is left: {leftovers:?}");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn concurrent_creations_claim_their_own_names() {
        let root = tmp("create-race");
        let store = std::sync::Arc::new(SetStore::new(root.to_str().unwrap()));
        let ids: Vec<String> = (0..8)
            .map(|_| {
                let store = store.clone();
                std::thread::spawn(move || store.create(&SetFile::new("Talk", SetKind::Hand, None, "d")).unwrap())
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect();
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), 8, "{ids:?}");
        assert_eq!(store.list().len(), 8);
        fs::remove_dir_all(root).unwrap();
    }

    const PERSONAL: &str = "---\ntitle: \"Talk\"\ncreated: 2026-10-20\nupdated: 2026-10-20\nkind: hand\n\
        audience: \"Oxford, week 3\"\ntags:\n  - metaphor\n  - talk\n# my own comment\n---\n\n\
        Opening thoughts: this set is for the second half.\n\nSecond paragraph, *mine*.\n\n\
        ## 1\n\n- passage: \"writing:a.md:3\"\n- corpus: writing\n- source_path: \"a.md\"\n- colour: blue\n\n> Quoted text\n\n\
        A paragraph of my own under the quote.\n\n> and a later quotation of mine\n";

    #[test]
    fn the_users_own_frontmatter_and_prose_survive_a_rewrite() {
        let mut s = parse(PERSONAL).unwrap();
        assert_eq!(s.items.len(), 1);
        assert_eq!(s.items[0].quote, "Quoted text", "a later quotation is prose, not the stored quote");
        // Any rewrite: an edit, a repair.
        apply_edit(&mut s, &SetEdit::Note { index: 0, note: "n".into() }, "2026-10-21").unwrap();
        apply_repair(&mut s.items[0], &Found { passage_id: "writing:a.md:9".into(), rel_path: "a.md".into(), quote: String::new() });
        let out = render(&s);
        for kept in [
            "audience: \"Oxford, week 3\"\ntags:\n  - metaphor\n  - talk\n# my own comment\n---\n",
            "Opening thoughts: this set is for the second half.\n\nSecond paragraph, *mine*.\n",
            "- colour: blue",
            "A paragraph of my own under the quote.\n\n> and a later quotation of mine\n",
        ] {
            assert!(out.contains(kept), "lost {kept:?} from:\n{out}");
        }
        assert!(out.contains("writing:a.md:9") && out.contains("- note: \"n\""));
        // Stable from then on.
        assert_eq!(render(&parse(&out).unwrap()), out);
    }

    #[test]
    fn a_saved_search_refresh_keeps_pinned_items_and_follows_the_query() {
        let mut s = SetFile::new("Scaffolding", SetKind::SavedSearch, Some("scaffolding".into()), "d");
        let mut pinned = item("writing:keep.md:1", "kept");
        pinned.pinned = true;
        pinned.note = "mine".into();
        let mut old = item("writing:old.md:1", "old match");
        old.note = "note on old".into();
        let gone = item("writing:gone.md:1", "no longer matches");
        s.items = vec![old, pinned, gone];
        s.dismissed = vec!["writing:no.md:1".into()];
        let matches = vec![
            item("writing:new.md:1", "new match"),
            item("writing:keep.md:1", "kept"),
            item("writing:old.md:1", "old match"),
            item("writing:no.md:1", "dismissed"),
        ];
        assert!(refresh_saved(&mut s, matches.clone(), 20, "d2"));
        let ids: Vec<&str> = s.items.iter().map(|i| i.passage.as_str()).collect();
        assert_eq!(ids, ["writing:keep.md:1", "writing:new.md:1", "writing:old.md:1"]);
        assert_eq!(s.items[0].note, "mine");
        assert_eq!(s.items[2].note, "note on old", "a note on a still-matching item stays");
        assert_eq!(s.updated, "d2");
        // Nothing new: unchanged, and a hand set is never touched.
        assert!(!refresh_saved(&mut s, matches.clone(), 20, "d3"));
        let mut h = sample();
        assert!(!refresh_saved(&mut h, matches, 20, "d3"));
    }
}
