//! Sets (HS-2A): a hand-picked, ordered collection of passages for a talk,
//! kept as one Markdown file per set in `<archive_path>/sets/<id>.md`. That
//! file is the only state a set has; it is git-tracked with the archive.
//!
//! This module is the seam for everything about a set that is not the engine:
//! - the file format: [`parse`] and [`render`] are exact inverses on any file
//!   this module wrote (byte-stable: order, notes, quotes survive a reopen);
//!   frontmatter keys and prose the user adds are kept verbatim,
//! - the store: [`SetStore`] reads and writes only inside the real
//!   `<archive>/sets` folder, through one handle on it (ids are validated,
//!   links are refused, writes are a private temp file + rename relative to
//!   that handle, each set's changes are serialised by [`SetStore::update`],
//!   and a file another app saved meanwhile is never overwritten),
//! - item identity: every item has a permanent item id ([`SetItem::id`],
//!   [`ensure_item_ids`]); a passage id holds a source line, so it is never
//!   used to tell items apart,
//! - editing: [`apply_edit`] (move, note, remove, pin, rename; each names
//!   its item by item id and acts on exactly that item or is refused; a
//!   removed or unpinned saved-search match is excluded from later
//!   refreshes),
//! - resolving items against the engine through the [`Lookup`] trait: by
//!   passage id first, then by `verify-quote` on the stored quote (repairing
//!   the id, one passage per item: [`repair_all`]), else "changed",
//!   keeping the stored quote; a passage counts as the item only when its
//!   text equals the stored quote (whitespace aside),
//! - saved-search sets: [`refresh_saved`] (pinned items stay, the others
//!   follow the query; an item with a note or prose is never dropped; run
//!   [`repair_all`] first so moved passages keep their items),
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
use std::ffi::CString;
use std::fs;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
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
    /// The item's own id: short, random, made when the item is created and
    /// never changed. Edits name an item by it; a passage id is not an
    /// identity (it holds the source line, which moves). Empty only for an
    /// item read from a file written before ids existed, until the next
    /// write gives it one ([`ensure_item_ids`]).
    #[serde(default)]
    pub id: String,
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
    /// What the user wrote in the item between its metadata and the quote
    /// (prose, own `- key:` lines), kept verbatim and written back there.
    #[serde(default)]
    pub lead: String,
    /// Anything the user wrote in the item after the quote, kept verbatim
    /// and written back after the quote.
    #[serde(default)]
    pub extra: String,
}

impl SetItem {
    /// Whether the user wrote anything on the item (a note, prose).
    pub fn annotated(&self) -> bool {
        !self.note.is_empty() || !self.lead.is_empty() || !self.extra.is_empty()
    }
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

// ---------- item ids ----------

/// A new item id: 12 random hex digits, not used by any item of `s`.
pub fn new_item_id(s: &SetFile) -> String {
    loop {
        let id = uuid::Uuid::new_v4().simple().to_string()[..12].to_string();
        if !s.items.iter().any(|i| i.id == id) {
            return id;
        }
    }
}

/// Every item gets an id of its own: an item without one (a file written
/// before ids existed, an item typed in by hand) gets a new one, in file
/// order; a later item repeating an earlier item's id (a block copied in an
/// editor) gets a new one too, so an id never names two items. Nothing else
/// changes. Returns whether any id was given.
pub fn ensure_item_ids(s: &mut SetFile) -> bool {
    let mut seen = std::collections::HashSet::new();
    let mut changed = false;
    for i in 0..s.items.len() {
        if s.items[i].id.is_empty() || !seen.insert(s.items[i].id.clone()) {
            s.items[i].id = new_item_id(s);
            seen.insert(s.items[i].id.clone());
            changed = true;
        }
    }
    changed
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
        render_item(&mut o, i, it);
    }
    o
}

/// One item as its file text, `## <i+1>` and all. The `- item:` line comes
/// first, so giving an item of an older file its id adds that one line and
/// changes no other byte.
fn render_item(o: &mut String, i: usize, it: &SetItem) {
    o.push_str(&format!("\n## {}\n\n", i + 1));
    if !it.id.is_empty() {
        o.push_str(&format!("- item: {}\n", it.id));
    }
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
    if !it.lead.is_empty() {
        o.push_str(&it.lead);
        o.push_str("\n\n");
    }
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

/// `text` (a set file) with the k-th item's id set to `ids[k]` (`None`
/// leaves it): an item whose first metadata line is an id (a block copied
/// with its id) has that line replaced in place; an item with none gets the
/// line inserted where [`render`] puts it, first in the item after its
/// heading and blank lines. The new line takes the line ending of the line
/// it replaces or precedes, and nothing else changes, so giving ids keeps
/// every other byte as it was.
fn insert_item_ids(text: &str, ids: &[Option<String>]) -> String {
    let crlf_file = text.contains("\r\n");
    let mut o = String::with_capacity(text.len() + ids.len() * 22);
    let mut fences = 0;
    let mut k = 0usize;
    let mut pending: Option<&str> = None;
    for line in text.split_inclusive('\n') {
        let l = line.trim_end_matches('\n');
        if fences < 2 {
            if l.trim_end() == "---" {
                fences += 1;
            }
            o.push_str(line);
            continue;
        }
        if item_heading(l) {
            pending = ids.get(k).and_then(|x| x.as_deref());
            k += 1;
        } else if let Some(id) = pending.filter(|_| !l.trim().is_empty()) {
            pending = None;
            let body = l.strip_suffix('\r').unwrap_or(l);
            let ending = &line[body.len()..];
            if item_id_line(body).is_some() {
                o.push_str(&format!("- item: {id}{ending}"));
                continue;
            }
            let ending = if ending.is_empty() { if crlf_file { "\r\n" } else { "\n" } } else { ending };
            o.push_str(&format!("- item: {id}{ending}"));
        }
        o.push_str(line);
    }
    o
}

/// An item id: 12 lower-case hex digits ([`new_item_id`]).
fn valid_item_id(v: &str) -> bool {
    v.len() == 12 && v.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The id an `- item: <id>` line names, when the line is one (line ending excluded).
fn item_id_line(l: &str) -> Option<&str> {
    let (k, v) = l.strip_prefix("- ")?.split_once(':')?;
    let v = v.trim();
    (k == "item" && valid_item_id(v)).then_some(v)
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
        lead: Vec<String>,
        extra: Vec<String>,
        phase: Phase,
        /// The item's first metadata line has been read (only that line may be its id).
        started: bool,
    }
    let finish = |c: Option<Cur>, set: &mut SetFile| -> Result<(), String> {
        if let Some(Cur { no, mut it, quote, lead, extra, .. }) = c {
            if it.passage.is_empty() {
                return Err(format!("item {no} has no passage"));
            }
            it.lead = trim_blank_lines(&lead);
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
            cur = Some(Cur { no: n, it: SetItem::default(), quote: vec![], lead: vec![], extra: vec![], phase: Phase::Meta, started: false });
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
                // Once the user's own text starts, every line up to the quote is
                // theirs, blank lines included (the item's metadata comes first).
                if !c.lead.is_empty() {
                    c.lead.push(l.to_string());
                    continue;
                }
                if l.trim().is_empty() {
                    continue;
                }
                let no = c.no;
                // The id is the item's first metadata line, in the id format, or
                // nothing: any other `- item:` line is the user's prose, as it
                // was before ids existed.
                let first = !c.started;
                c.started = true;
                if first {
                    if let Some(id) = item_id_line(l) {
                        c.it.id = id.to_string();
                        continue;
                    }
                }
                let meta = l.strip_prefix("- ").and_then(|r| r.split_once(':'));
                match meta {
                    Some(("passage", v)) => c.it.passage = unjs(v).map_err(|e| format!("item {no}: {e}"))?,
                    Some(("corpus", v)) => c.it.corpus = v.trim().to_string(),
                    Some(("source_path", v)) => c.it.source_path = unjs(v).map_err(|e| format!("item {no}: {e}"))?,
                    Some(("pinned", v)) => c.it.pinned = v.trim() == "true",
                    Some(("note", v)) => c.it.note = unjs(v).map_err(|e| format!("item {no}: {e}"))?,
                    _ => c.lead.push(l.to_string()),
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

/// The store of one archive's sets. Every byte goes through one handle on
/// the `sets` folder, opened once per operation (`O_DIRECTORY | O_NOFOLLOW`
/// on the archive's own `sets` entry) and checked; files are created,
/// renamed and removed relative to that handle (`openat`, `renameat`,
/// `unlinkat`), so swapping `sets` for a link after the check changes
/// nothing about where the bytes go. Set files and temp files are never
/// links (`O_NOFOLLOW`, `O_EXCL`). Every change to one set runs under that
/// set's lock (load, edit and save together), and a set file that changed on
/// disk since it was read (another editor saved it) is never replaced: the
/// change is made again on the new text once, when that is safe
/// ([`SetStore::update_guarded`]), else refused.
pub struct SetStore {
    archive: PathBuf,
    /// Tests only: called at named points of an operation, to inject a race.
    #[cfg(test)]
    hook: Option<Hook>,
}

#[cfg(test)]
type Hook = Arc<dyn Fn(&str) + Send + Sync>;

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

fn c_name(name: &str) -> CString {
    // Names here are set ids (validated) and generated temp names: no NUL.
    CString::new(name).expect("a set file name has no NUL")
}

fn errno() -> std::io::Error {
    std::io::Error::last_os_error()
}

/// The open `sets` folder: every name is resolved relative to it.
struct Dir {
    fd: OwnedFd,
}

/// What `fstatat` says of a name in the folder, without following a link.
enum Entry {
    Missing,
    Link,
    File,
    Other,
}

impl Dir {
    fn raw(&self) -> libc::c_int {
        self.fd.as_raw_fd()
    }

    fn entry(&self, name: &str) -> std::io::Result<Entry> {
        let n = c_name(name);
        let mut st: libc::stat = unsafe { std::mem::zeroed() };
        // SAFETY: a valid fd, a NUL-terminated name and a stat buffer of the right type.
        let r = unsafe { libc::fstatat(self.raw(), n.as_ptr(), &mut st, libc::AT_SYMLINK_NOFOLLOW) };
        if r != 0 {
            let e = errno();
            return if e.kind() == std::io::ErrorKind::NotFound { Ok(Entry::Missing) } else { Err(e) };
        }
        Ok(match st.st_mode & libc::S_IFMT {
            libc::S_IFLNK => Entry::Link,
            libc::S_IFREG => Entry::File,
            _ => Entry::Other,
        })
    }

    fn open(&self, name: &str, flags: libc::c_int, mode: libc::c_uint) -> std::io::Result<fs::File> {
        let n = c_name(name);
        // SAFETY: a valid fd and a NUL-terminated name; the returned fd is owned below.
        let fd = unsafe { libc::openat(self.raw(), n.as_ptr(), flags | libc::O_NOFOLLOW | libc::O_CLOEXEC, mode) };
        if fd < 0 {
            return Err(errno());
        }
        // SAFETY: `fd` was just returned by openat and is owned by nobody else.
        Ok(fs::File::from(unsafe { OwnedFd::from_raw_fd(fd) }))
    }

    /// A regular file's bytes; never through a link, never a fifo or device.
    fn read(&self, name: &str) -> std::io::Result<Vec<u8>> {
        let mut f = self.open(name, libc::O_RDONLY | libc::O_NONBLOCK, 0)?;
        if !f.metadata()?.is_file() {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "not a file"));
        }
        let mut b = vec![];
        f.read_to_end(&mut b)?;
        Ok(b)
    }

    /// Create a file that must not exist yet (`O_EXCL`; never through a link).
    fn create(&self, name: &str) -> std::io::Result<fs::File> {
        self.open(name, libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL, 0o644)
    }

    fn rename(&self, from: &str, to: &str) -> std::io::Result<()> {
        let (a, b) = (c_name(from), c_name(to));
        // SAFETY: valid fds and NUL-terminated names.
        if unsafe { libc::renameat(self.raw(), a.as_ptr(), self.raw(), b.as_ptr()) } != 0 {
            return Err(errno());
        }
        Ok(())
    }

    fn remove(&self, name: &str) {
        let n = c_name(name);
        // SAFETY: a valid fd and a NUL-terminated name.
        unsafe { libc::unlinkat(self.raw(), n.as_ptr(), 0) };
    }

    /// The names in the folder (read through the handle).
    fn names(&self) -> Vec<String> {
        let mut out = vec![];
        // SAFETY: fdopendir takes ownership of the duplicate fd; closedir releases both.
        unsafe {
            let d = libc::dup(self.raw());
            if d < 0 {
                return out;
            }
            let dp = libc::fdopendir(d);
            if dp.is_null() {
                libc::close(d);
                return out;
            }
            libc::rewinddir(dp);
            loop {
                let ent = libc::readdir(dp);
                if ent.is_null() {
                    break;
                }
                if let Ok(s) = std::ffi::CStr::from_ptr((*ent).d_name.as_ptr()).to_str() {
                    out.push(s.to_string());
                }
            }
            libc::closedir(dp);
        }
        out
    }

    /// The folder's identity, for the per-set lock.
    fn key(&self, id: &str) -> PathBuf {
        let mut st: libc::stat = unsafe { std::mem::zeroed() };
        // SAFETY: a valid fd and a stat buffer of the right type.
        unsafe { libc::fstat(self.raw(), &mut st) };
        PathBuf::from(format!("{}:{}/{id}", st.st_dev, st.st_ino))
    }
}

const LINKED_FILE: &str = "the file is a link; sets are only read and written as real files";

impl SetStore {
    /// The store of an archive: `<archive_path>/sets`.
    pub fn new(archive_path: &str) -> SetStore {
        SetStore {
            archive: PathBuf::from(archive_path),
            #[cfg(test)]
            hook: None,
        }
    }

    #[cfg(test)]
    fn with_hook(mut self, h: impl Fn(&str) + Send + Sync + 'static) -> SetStore {
        self.hook = Some(Arc::new(h));
        self
    }

    #[allow(unused_variables)]
    fn at(&self, point: &str) {
        #[cfg(test)]
        if let Some(h) = &self.hook {
            h(point)
        }
    }

    /// Open `<archive>/sets` once: the archive's own entry, refused when it
    /// is a link or not a folder; created when `create` and missing.
    fn dir(&self, create: bool) -> Result<Dir, String> {
        let archive = fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_CLOEXEC)
            .open(&self.archive)
            .map_err(|e| format!("archive folder: {e}"))?;
        let archive = Dir { fd: OwnedFd::from(archive) };
        let name = c_name("sets");
        for _ in 0..2 {
            // SAFETY: a valid fd and a NUL-terminated name; the fd is owned below.
            let fd = unsafe {
                libc::openat(archive.raw(), name.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            };
            if fd >= 0 {
                // SAFETY: just returned by openat, owned by nobody else.
                let d = Dir { fd: unsafe { OwnedFd::from_raw_fd(fd) } };
                self.at("dir-opened");
                return Ok(d);
            }
            let e = errno();
            match archive.entry("sets") {
                Ok(Entry::Link) => return Err("the archive's sets folder is a link: sets are kept only in a real folder".into()),
                Ok(Entry::File | Entry::Other) if e.raw_os_error() == Some(libc::ENOTDIR) => {
                    return Err("the archive's sets is not a folder".into())
                }
                Ok(Entry::Missing) if create => {
                    // SAFETY: a valid fd and a NUL-terminated name.
                    if unsafe { libc::mkdirat(archive.raw(), name.as_ptr(), 0o755) } != 0 {
                        let e = errno();
                        if e.kind() != std::io::ErrorKind::AlreadyExists {
                            return Err(format!("sets folder: {e}"));
                        }
                    }
                }
                _ => return Err(format!("sets folder: {e}")),
            }
        }
        Err("sets folder: it could not be opened".into())
    }

    /// `<id>.md` for a valid id; refused when the file is a link or not a file.
    fn file(dir: &Dir, id: &str) -> Result<String, String> {
        if !valid_id(id) {
            return Err(format!("{id:?} is not a set id"));
        }
        let name = format!("{id}.md");
        match dir.entry(&name) {
            Ok(Entry::Link) => Err(format!("set {id}: {LINKED_FILE}")),
            Ok(Entry::Other) => Err(format!("set {id}: not a file")),
            Ok(Entry::File | Entry::Missing) => Ok(name),
            Err(e) => Err(format!("set {id}: {e}")),
        }
    }

    fn read_bytes(dir: &Dir, id: &str) -> Result<Vec<u8>, String> {
        let name = Self::file(dir, id)?;
        dir.read(&name).map_err(|e| {
            if e.raw_os_error() == Some(libc::ELOOP) {
                format!("set {id}: {LINKED_FILE}")
            } else {
                format!("set {id}: {e}")
            }
        })
    }

    fn parse_bytes(id: &str, b: &[u8]) -> Result<SetFile, String> {
        let text = std::str::from_utf8(b).map_err(|_| format!("set {id}: the file is not UTF-8 text"))?;
        parse(text).map_err(|e| format!("set {id}: {e}"))
    }

    /// The set's new text in a temp file of its own (random name, `O_EXCL`)
    /// in the folder, flushed. Returns the temp name.
    fn write_temp(dir: &Dir, id: &str, set: &SetFile) -> Result<String, String> {
        Self::write_temp_text(dir, id, &render(set))
    }

    fn write_temp_text(dir: &Dir, id: &str, text: &str) -> Result<String, String> {
        let tmp = format!(".{id}.{}.tmp", uuid::Uuid::new_v4().simple());
        let result = (|| -> std::io::Result<()> {
            let mut f = dir.create(&tmp)?;
            f.write_all(text.as_bytes())?;
            f.sync_all()?;
            Ok(())
        })();
        if let Err(e) = result {
            dir.remove(&tmp);
            return Err(format!("set {id}: {e}"));
        }
        Ok(tmp)
    }

    /// Rename the temp file over the set file (both in the folder).
    fn replace(dir: &Dir, id: &str, tmp: &str) -> Result<(), String> {
        let done = Self::file(dir, id).and_then(|name| dir.rename(tmp, &name).map_err(|e| format!("set {id}: {e}")));
        if done.is_err() {
            dir.remove(tmp);
        }
        done
    }

    pub fn load(&self, id: &str) -> Result<SetFile, String> {
        let dir = self.dir(false)?;
        Self::parse_bytes(id, &Self::read_bytes(&dir, id)?)
    }

    /// Replace the set's file with `set`, under the set's lock.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn save(&self, id: &str, set: &SetFile) -> Result<(), String> {
        let dir = self.dir(true)?;
        Self::file(&dir, id)?;
        let lock = set_lock(&dir.key(id));
        let _g = lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut set = set.clone();
        ensure_item_ids(&mut set);
        let tmp = Self::write_temp(&dir, id, &set)?;
        self.at("before-replace");
        Self::replace(&dir, id, &tmp)
    }

    /// Load, change and save one set as one step under its lock; the file is
    /// written only when `f` changed the set (or an item was given its id,
    /// [`ensure_item_ids`]). An error from `f` writes nothing.
    ///
    /// For a change worked out from the file itself (adding, repairing,
    /// refreshing): the file is replaced only while it still holds the bytes
    /// that were read; when another app saved it in between, `f` runs again
    /// on the new text (once); if it changed again, nothing is written and the
    /// change is refused with a message saying so.
    pub fn update<T>(&self, id: &str, f: impl FnMut(&mut SetFile) -> Result<T, String>) -> Result<T, String> {
        self.update_guarded(id, |_| String::new(), f)
    }

    /// [`update`](Self::update) for a change the user decided on what they
    /// saw (a note, a title, a move): after another app saved the file in
    /// between, the change is made again only when `touches` (the part of
    /// the set it acts on, see [`footprint`]) is the same in the new text as
    /// in the text first read. Otherwise the other app's save stands and the
    /// change is refused: an outside change is never silently replaced.
    pub fn update_guarded<T>(
        &self,
        id: &str,
        touches: impl Fn(&SetFile) -> String,
        mut f: impl FnMut(&mut SetFile) -> Result<T, String>,
    ) -> Result<T, String> {
        let refused = || {
            format!(
                "set {id}: the file was changed by another app while this change was being saved; nothing was overwritten. Reopen the set and try again."
            )
        };
        let dir = self.dir(false)?;
        Self::file(&dir, id)?;
        let lock = set_lock(&dir.key(id));
        let _g = lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut first: Option<String> = None;
        for _ in 0..2 {
            let read = Self::read_bytes(&dir, id)?;
            let mut set = Self::parse_bytes(id, &read)?;
            let before = set.clone();
            ensure_item_ids(&mut set);
            let with_ids = set.clone();
            let part = touches(&set);
            match &first {
                None => first = Some(part),
                // The other app changed what this change acts on: its save stands.
                Some(was) if *was != part => return Err(refused()),
                Some(_) => {}
            }
            let out = f(&mut set)?;
            if set == before {
                return Ok(out);
            }
            // Only ids were given: they go into the text as it was, every other byte kept.
            let text = if set == with_ids {
                let ids: Vec<Option<String>> =
                    before.items.iter().zip(&set.items).map(|(b, n)| (b.id != n.id).then(|| n.id.clone())).collect();
                std::str::from_utf8(&read)
                    .ok()
                    .map(|t| insert_item_ids(t, &ids))
                    .filter(|t| parse(t).is_ok_and(|p| p == set))
                    .unwrap_or_else(|| render(&set))
            } else {
                render(&set)
            };
            let tmp = Self::write_temp_text(&dir, id, &text)?;
            self.at("before-replace");
            // Still what was read? Otherwise another editor saved it: never overwrite that.
            if Self::read_bytes(&dir, id).is_ok_and(|now| now == read) {
                Self::replace(&dir, id, &tmp)?;
                return Ok(out);
            }
            dir.remove(&tmp);
        }
        Err(refused())
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
        let name = loop {
            let name = Self::file(&dir, &id)?;
            match dir.create(&name) {
                Ok(_) => break name,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    n += 1;
                    id = format!("{base}-{n}");
                }
                Err(e) => return Err(format!("set {id}: {e}")),
            }
        };
        let lock = set_lock(&dir.key(&id));
        let _g = lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut set = set.clone();
        ensure_item_ids(&mut set);
        let written = Self::write_temp(&dir, &id, &set).and_then(|tmp| {
            self.at("before-replace");
            Self::replace(&dir, &id, &tmp)
        });
        if let Err(e) = written {
            dir.remove(&name);
            return Err(e);
        }
        Ok(id)
    }

    /// Every set, most recently updated first. Links are skipped.
    pub fn list(&self) -> Vec<SetSummary> {
        let Ok(dir) = self.dir(false) else { return vec![] };
        let mut out: Vec<SetSummary> = dir
            .names()
            .into_iter()
            .filter_map(|name| {
                let id = name.strip_suffix(".md")?.to_string();
                if !valid_id(&id) || !matches!(dir.entry(&name), Ok(Entry::File)) {
                    return None;
                }
                let s = Self::parse_bytes(&id, &dir.read(&name).ok()?).ok()?;
                Some(SetSummary { id, title: s.title, kind: s.kind, items: s.items.len(), updated: s.updated })
            })
            .collect();
        out.sort_by(|a, b| b.updated.cmp(&a.updated).then(a.title.cmp(&b.title)));
        out
    }
}

// ---------- editing ----------

/// An edit names the item it acts on by its item id ([`SetItem::id`]), never
/// by position alone and never by passage id: the file may have changed
/// since the page showed it (a saved search refreshed in the background, a
/// passage moved and another took its line), and an edit must not land on
/// another item. An item that is not in the file any more is refused
/// ([`STALE`]).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case")]
pub enum SetEdit {
    /// Take the item `item`, shown at `from`, and put it at `to`, where the
    /// page showed `to_item`; refused unless both are still there.
    Move { item: String, from: usize, to: usize, to_item: String },
    Note { item: String, note: String },
    Remove { item: String },
    /// Saved-search sets: unpinning keeps the item out after later updates.
    Pin { item: String, pinned: bool },
    Rename { title: String },
}

/// The set file no longer matches what the page showed.
pub const STALE: &str = "the set changed since the page showed it (a saved search updated in the background); it has been reloaded, so make the change again";

/// Where the item with this id is: exactly one item, else `None`.
fn find_item(s: &SetFile, id: &str) -> Option<usize> {
    let mut at = s.items.iter().enumerate().filter(|(_, i)| !id.is_empty() && i.id == id).map(|(n, _)| n);
    match (at.next(), at.next()) {
        (Some(n), None) => Some(n),
        _ => None,
    }
}

/// The part of the set an edit acts on, as text: for an item, its position
/// and its whole block as the file holds it (note, prose, quote, pin); for a
/// rename, the title. Two reads of the file with the same footprint give
/// the edit the same thing to act on ([`SetStore::update_guarded`]).
pub fn footprint(s: &SetFile, e: &SetEdit) -> String {
    let item = |id: &str| match find_item(s, id) {
        Some(n) => {
            let mut o = String::new();
            render_item(&mut o, n, &s.items[n]);
            o
        }
        None => format!("no item {id:?}\n"),
    };
    match e {
        SetEdit::Move { item: a, to_item: b, .. } => item(a) + &item(b),
        SetEdit::Note { item: a, .. } | SetEdit::Remove { item: a } | SetEdit::Pin { item: a, .. } => item(a),
        SetEdit::Rename { .. } => format!("title: {}\n", js(&s.title)),
    }
}

pub fn apply_edit(s: &mut SetFile, e: &SetEdit, today: &str) -> Result<(), String> {
    let find = |s: &SetFile, id: &str| find_item(s, id).ok_or_else(|| STALE.to_string());
    match e {
        SetEdit::Move { item, from, to, to_item } => {
            if find_item(s, item) != Some(*from) || find_item(s, to_item) != Some(*to) {
                return Err(STALE.into());
            }
            let it = s.items.remove(*from);
            s.items.insert(*to, it);
        }
        SetEdit::Note { item, note } => {
            let i = find(s, item)?;
            s.items[i].note = note.trim().replace('\n', " ");
        }
        SetEdit::Remove { item } => {
            let i = find(s, item)?;
            let it = s.items.remove(i);
            // A saved search would bring a removed match straight back.
            if s.kind == SetKind::SavedSearch && !s.excluded.contains(&it.passage) {
                s.excluded.push(it.passage);
            }
        }
        SetEdit::Pin { item, pinned } => {
            let i = find(s, item)?;
            let was = s.items[i].pinned;
            let passage = s.items[i].passage.clone();
            s.items[i].pinned = *pinned;
            if s.kind == SetKind::SavedSearch {
                if *pinned {
                    s.excluded.retain(|x| *x != passage);
                } else if was && !s.excluded.contains(&passage) {
                    // "An unpinned item stays out after updates" (HS-2A, saved search).
                    s.excluded.push(passage);
                }
            }
        }
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
        id: uuid::Uuid::new_v4().simple().to_string()[..12].to_string(),
        passage: c.passage_id.clone(),
        corpus: c.corpus.clone(),
        source_path: c.rel_path.clone(),
        quote: c.quote.trim().to_string(),
        ..Default::default()
    }
}

/// Add an item at the end; `false` when the set already holds that passage.
pub fn add_item(s: &mut SetFile, mut it: SetItem, today: &str) -> bool {
    if s.items.iter().any(|x| x.passage == it.passage) {
        return false;
    }
    if it.id.is_empty() || s.items.iter().any(|x| x.id == it.id) {
        it.id = new_item_id(s);
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

/// Saved-search sets: pinned items stay where they are; the other items
/// become `matches` (those not already pinned, in match order, neither
/// dismissed nor removed nor unpinned by the user), after the pinned ones.
/// An item is the same item as a match only when it holds that passage with
/// the same text (an item whose source changed is not the passage now on
/// its old line). Items are told apart by item id, never by passage id: a
/// refresh never deletes what the user wrote, so every item with a note or
/// prose stays (after the matches) even when it no longer matches, was
/// unpinned, its passage is gone, or another item now holds its passage
/// id. Resolve and repair the items first ([`repair_all`]), so a moved
/// passage keeps its item. Returns whether anything changed.
pub fn refresh_saved(s: &mut SetFile, matches: Vec<SetItem>, cap: usize, today: &str) -> bool {
    if s.kind != SetKind::SavedSearch {
        return false;
    }
    let before = s.items.clone();
    let holds = |i: &SetItem, m: &SetItem| i.passage == m.passage && same_text(&i.quote, &m.quote);
    let mut items: Vec<SetItem> = s.items.iter().filter(|i| i.pinned).cloned().collect();
    let room = cap.max(items.len());
    for m in matches {
        if items.len() >= room {
            break;
        }
        if items.iter().any(|i| holds(i, &m)) || s.dismissed.contains(&m.passage) || s.excluded.contains(&m.passage) {
            continue;
        }
        // A still-matching item is the same item: it keeps everything it had.
        let it = match before.iter().find(|i| holds(i, &m) && !items.iter().any(|x| x.id == i.id)) {
            Some(i) => i.clone(),
            None => {
                let mut m = m;
                if m.id.is_empty() || before.iter().chain(items.iter()).any(|x| x.id == m.id) {
                    let mut all = s.clone();
                    all.items.extend(items.iter().cloned());
                    m.id = new_item_id(&all);
                }
                m
            }
        };
        items.push(it);
    }
    for it in &before {
        if it.annotated() && !items.iter().any(|i| i.id == it.id) {
            items.push(it.clone());
        }
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

/// The line a passage id names (`<corpus>:<path>:<line>`), if it names one.
fn line_of(passage: &str) -> Option<i64> {
    passage.rsplit_once(':').and_then(|(_, n)| n.trim().parse().ok())
}

/// How far a candidate passage is from where the item was: another file is
/// farther than any line of the same file; within the file, by lines.
fn distance(it: &SetItem, f: &Found) -> (bool, i64) {
    let lines = match (line_of(&it.passage), line_of(&f.passage_id)) {
        (Some(a), Some(b)) => (a - b).abs(),
        _ => i64::MAX,
    };
    (f.rel_path != it.source_path, lines)
}

/// The passages that hold an item's quote now, closest first (see [`distance`]).
fn candidates(it: &SetItem, l: &dyn Lookup) -> Vec<Found> {
    let mut found = l.find_quote(&it.corpus, &it.quote);
    // The same file first (a moved line, not another document with the same words), then the nearest line.
    found.sort_by_key(|f| distance(it, f));
    found.into_iter().filter_map(|f| l.cite(&f.passage_id)).filter(|c| same_text(&c.quote, &it.quote)).collect()
}

/// Where an item's passage is now: by id first, then by its quote (the
/// closest passage holding it).
#[cfg_attr(not(test), allow(dead_code))]
pub fn resolve(it: &SetItem, l: &dyn Lookup) -> (Status, Option<Found>) {
    if let Some(f) = l.cite(&it.passage) {
        if same_text(&f.quote, &it.quote) {
            return (Status::Ok, Some(f));
        }
    }
    match candidates(it, l).into_iter().next() {
        Some(c) => (Status::Repaired, Some(c)),
        None => (Status::Changed, None),
    }
}

/// Write a repair back into the item.
pub fn apply_repair(it: &mut SetItem, f: &Found) {
    it.passage = f.passage_id.clone();
    it.source_path = f.rel_path.clone();
}

/// Resolve every item and write each repair into the set: the item takes
/// its passage's new id (keeping its note and prose), and an exclusion on
/// the old id moves to the new one. One result per item, in order.
///
/// Repair is one-to-one: a passage an item still holds is never another
/// item's repair, and no two items are given the same passage. When two
/// items would be repaired to one passage, the one whose old line was
/// closer gets it (the earlier item on a tie) and the other is "source
/// changed" this time, its note and prose untouched.
pub fn repair_all(s: &mut SetFile, l: &dyn Lookup) -> Vec<(Status, Option<Found>)> {
    let mut out: Vec<(Status, Option<Found>)> = vec![(Status::Changed, None); s.items.len()];
    let mut held = std::collections::HashSet::new();
    for (i, it) in s.items.iter().enumerate() {
        if let Some(f) = l.cite(&it.passage) {
            // Two items holding one passage (saved so by an older version): the
            // first keeps it; the other is placed below like any moved item.
            if same_text(&f.quote, &it.quote) && held.insert(f.passage_id.clone()) {
                out[i] = (Status::Ok, Some(f));
            }
        }
    }
    // Each item still to place: its closest passage no item holds.
    let mut wants: Vec<(usize, Found)> = vec![];
    for (i, it) in s.items.iter().enumerate() {
        if out[i].0 == Status::Ok {
            continue;
        }
        if let Some(c) = candidates(it, l).into_iter().find(|c| !held.contains(&c.passage_id)) {
            wants.push((i, c));
        }
    }
    // Closest claim first: one passage, one item.
    wants.sort_by_key(|(i, c)| (distance(&s.items[*i], c), *i));
    for (i, c) in wants {
        if !held.insert(c.passage_id.clone()) {
            continue;
        }
        let old = s.items[i].passage.clone();
        apply_repair(&mut s.items[i], &c);
        for x in s.excluded.iter_mut().filter(|x| **x == old) {
            *x = c.passage_id.clone();
        }
        out[i] = (Status::Repaired, Some(c));
    }
    out
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
        // In these tests an item's id is taken from its passage id as first added; it never changes after.
        SetItem { id: tid(p), passage: p.into(), corpus, source_path: path, quote: q.into(), ..Default::default() }
    }

    /// The id [`item`] gives the item first added at passage `p`: 12 hex digits, like a real id.
    fn tid(p: &str) -> String {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        p.hash(&mut h);
        format!("{:012x}", h.finish() & 0xffff_ffff_ffff)
    }

    fn pid(s: &SetFile, i: usize) -> String {
        s.items[i].id.clone()
    }
    /// A move as the page sends it: the item shown at `from` to where `to` showed.
    fn mv(s: &SetFile, from: usize, to: usize) -> SetEdit {
        SetEdit::Move { item: pid(s, from), from, to, to_item: pid(s, to) }
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
        { let e = mv(&s2, 1, 0); apply_edit(&mut s2, &e, "2026-10-21") }.unwrap();
        { let e = SetEdit::Note { item: pid(&s2, 0), note: "Then this".into() }; apply_edit(&mut s2, &e, "2026-10-21") }.unwrap();
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
        a.id = "3f9a2c1b7d4e".into();
        a.note = "n".into();
        s.items = vec![a];
        assert_eq!(
            render(&s),
            "---\ntitle: \"Talk\"\ncreated: 2026-10-20\nupdated: 2026-10-20\nkind: hand\n---\n\n## 1\n\n- item: 3f9a2c1b7d4e\n- passage: \"writing:a.md:3\"\n- corpus: writing\n- source_path: \"a.md\"\n- note: \"n\"\n\n> Line one\n>\n> Line two\n"
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
        { let e = mv(&s, 2, 0); apply_edit(&mut s, &e, "d") }.unwrap();
        assert_eq!(s.items.iter().map(|i| i.corpus.as_str()).collect::<Vec<_>>(), ["tweets", "writing", "highlights"]);
        { let e = SetEdit::Remove { item: pid(&s, 1) }; apply_edit(&mut s, &e, "d") }.unwrap();
        assert_eq!(s.items.len(), 2);
        { let e = SetEdit::Pin { item: pid(&s, 0), pinned: true }; apply_edit(&mut s, &e, "d") }.unwrap();
        assert!(s.items[0].pinned);
        { let e = SetEdit::Note { item: pid(&s, 1), note: "  two\nlines ".into() }; apply_edit(&mut s, &e, "d") }.unwrap();
        assert_eq!(s.items[1].note, "two lines");
        apply_edit(&mut s, &SetEdit::Rename { title: " New ".into() }, "d").unwrap();
        assert_eq!(s.title, "New");
        assert!(apply_edit(&mut s, &SetEdit::Remove { item: tid("writing:nowhere.md:1") }, "d").is_err());
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
        apply_edit(&mut s, &SetEdit::Remove { item: tid("writing:a.md:1") }, "d").unwrap();
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
        { let e = SetEdit::Remove { item: pid(&h, 0) }; apply_edit(&mut h, &e, "d") }.unwrap();
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
        ensure_item_ids(&mut s);
        assert_eq!(s.items.len(), 1);
        assert_eq!(s.items[0].quote, "Quoted text", "a later quotation is prose, not the stored quote");
        // Any rewrite: an edit, a repair.
        { let e = SetEdit::Note { item: pid(&s, 0), note: "n".into() }; apply_edit(&mut s, &e, "2026-10-21") }.unwrap();
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

    /// Swap `<root>/sets` for a link to `elsewhere` (the real folder moves to `sets-real`).
    fn swap_for_link(root: &Path, elsewhere: &Path) {
        fs::rename(root.join("sets"), root.join("sets-real")).unwrap();
        std::os::unix::fs::symlink(elsewhere, root.join("sets")).unwrap();
    }

    #[test]
    fn a_sets_folder_swapped_for_a_link_after_the_check_is_never_written_through() {
        for point in ["dir-opened", "before-replace"] {
            let root = tmp("swap");
            let elsewhere = root.join("elsewhere");
            fs::create_dir_all(&elsewhere).unwrap();
            let id = SetStore::new(root.to_str().unwrap()).create(&sample()).unwrap();
            let fired = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let store = {
                let (root, elsewhere, fired, point) = (root.clone(), elsewhere.clone(), fired.clone(), point.to_string());
                SetStore::new(root.to_str().unwrap()).with_hook(move |at| {
                    if at == point && !fired.swap(true, std::sync::atomic::Ordering::SeqCst) {
                        swap_for_link(&root, &elsewhere);
                    }
                })
            };
            store.update(&id, |s| { s.title = "Changed".into(); Ok(()) }).unwrap();
            assert!(fired.load(std::sync::atomic::Ordering::SeqCst), "{point}: the swap happened");
            assert_eq!(fs::read_dir(&elsewhere).unwrap().count(), 0, "{point}: nothing was written through the link");
            // The change went into the folder that was checked, inside the archive.
            assert_eq!(parse(&fs::read_to_string(root.join("sets-real").join(format!("{id}.md"))).unwrap()).unwrap().title, "Changed");
            // From then on the link is refused.
            assert!(store.load(&id).is_err());
            fs::remove_dir_all(root).unwrap();
        }
        // The same for a new set and a save.
        for op in ["create", "save"] {
            let root = tmp("swap-new");
            let elsewhere = root.join("elsewhere");
            fs::create_dir_all(&elsewhere).unwrap();
            fs::create_dir_all(root.join("sets")).unwrap();
            let fired = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let store = {
                let (root, elsewhere, fired) = (root.clone(), elsewhere.clone(), fired.clone());
                SetStore::new(root.to_str().unwrap()).with_hook(move |at| {
                    if at == "dir-opened" && !fired.swap(true, std::sync::atomic::Ordering::SeqCst) {
                        swap_for_link(&root, &elsewhere);
                    }
                })
            };
            if op == "create" { store.create(&sample()).unwrap(); } else { store.save("talk", &sample()).unwrap(); }
            assert_eq!(fs::read_dir(&elsewhere).unwrap().count(), 0, "{op}: nothing was written through the link");
            assert_eq!(fs::read_dir(root.join("sets-real")).unwrap().count(), 1, "{op}");
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn an_outside_save_between_read_and_replace_is_never_overwritten() {
        let root = tmp("outside-edit");
        let id = SetStore::new(root.to_str().unwrap()).create(&sample()).unwrap();
        let file = root.join("sets").join(format!("{id}.md"));
        // Another editor saves once, after the app read the file and before it replaced it.
        let outside = render(&sample()).replace("title: \"Metaphor talk\"", "title: \"Renamed elsewhere\"") + "\nWritten in another editor.\n";
        let fired = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let store = {
            let (file, outside, fired) = (file.clone(), outside.clone(), fired.clone());
            SetStore::new(root.to_str().unwrap()).with_hook(move |at| {
                if at == "before-replace" && !fired.swap(true, std::sync::atomic::Ordering::SeqCst) {
                    fs::write(&file, &outside).unwrap();
                }
            })
        };
        store.update(&id, |s| { add_item(s, item("writing:new.md:1", "New"), "d"); Ok(()) }).unwrap();
        let after = fs::read_to_string(&file).unwrap();
        assert!(after.contains("Renamed elsewhere") && after.contains("Written in another editor."), "the outside save is kept:\n{after}");
        assert!(after.contains("writing:new.md:1"), "and the app's edit is applied on top of it:\n{after}");

        // An editor that keeps saving in that window: the edit is refused and its text stays.
        let n = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let store = {
            let (file, n) = (file.clone(), n.clone());
            SetStore::new(root.to_str().unwrap()).with_hook(move |at| {
                if at == "before-replace" {
                    let k = n.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    fs::write(&file, render(&sample()) + &format!("\nSave {k}.\n")).unwrap();
                }
            })
        };
        let err = store.update(&id, |s| { s.title = "Mine".into(); Ok(()) }).unwrap_err();
        assert!(err.contains("changed by another app") && err.contains("nothing was overwritten"), "{err}");
        assert!(fs::read_to_string(&file).unwrap().ends_with("\nSave 1.\n"), "the other app's last save stands");
        let left: Vec<_> = fs::read_dir(root.join("sets")).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(left.len(), 1, "no temp file is left: {left:?}");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn prose_before_an_items_quote_round_trips_byte_for_byte_through_a_repair() {
        let prose = "First paragraph of mine, before the quote.\n\nSecond paragraph, *also mine*.\n- colour: blue";
        let text = format!(
            "---\ntitle: \"Talk\"\ncreated: 2026-10-20\nupdated: 2026-10-20\nkind: hand\n---\n\n## 1\n\n\
             - passage: \"writing:a.md:3\"\n- corpus: writing\n- source_path: \"a.md\"\n\n{prose}\n\n> Quoted text\n\nAfter the quote.\n"
        );
        let mut s = parse(&text).unwrap();
        assert_eq!(s.items[0].lead, prose);
        assert_eq!(s.items[0].quote, "Quoted text");
        assert_eq!(render(&s), text, "an untouched file renders as it was");
        apply_repair(&mut s.items[0], &Found { passage_id: "writing:a.md:9".into(), rel_path: "a.md".into(), quote: String::new() });
        let out = render(&s);
        assert_eq!(out, text.replace("writing:a.md:3", "writing:a.md:9"), "only the id changed");
        assert_eq!(render(&parse(&out).unwrap()), out);
    }

    fn saved(items: Vec<SetItem>) -> SetFile {
        let mut s = SetFile::new("S", SetKind::SavedSearch, Some("q".into()), "d");
        s.items = items;
        s
    }

    #[test]
    fn an_unpinned_item_stays_out_after_updates() {
        let matches = || vec![item("writing:a.md:1", "a"), item("writing:b.md:1", "b")];
        let mut s = saved(vec![]);
        refresh_saved(&mut s, matches(), 20, "d");
        apply_edit(&mut s, &SetEdit::Pin { item: tid("writing:a.md:1"), pinned: true }, "d").unwrap();
        apply_edit(&mut s, &SetEdit::Pin { item: tid("writing:a.md:1"), pinned: false }, "d").unwrap();
        assert_eq!(s.excluded, ["writing:a.md:1"]);
        refresh_saved(&mut s, matches(), 20, "d2");
        assert_eq!(s.items.iter().map(|i| i.passage.as_str()).collect::<Vec<_>>(), ["writing:b.md:1"], "it still matches, and stays out");
        assert_eq!(parse(&render(&s)).unwrap().excluded, ["writing:a.md:1"], "the unpin is in the file");
        // Pinning a match that was never pinned, and toggling, records nothing else.
        apply_edit(&mut s, &SetEdit::Pin { item: tid("writing:b.md:1"), pinned: false }, "d").unwrap();
        assert_eq!(s.excluded, ["writing:a.md:1"], "a match that was never pinned is not unpinned");
        // Pinning it again takes it back in.
        add_item(&mut s, item("writing:a.md:1", "a"), "d3");
        apply_edit(&mut s, &SetEdit::Pin { item: tid("writing:a.md:1"), pinned: true }, "d").unwrap();
        assert!(s.excluded.is_empty());
    }

    #[test]
    fn a_refresh_never_deletes_a_note_or_prose() {
        let mut noted = item("writing:old.md:1", "old");
        noted.note = "my note".into();
        let mut prose = item("writing:gone.md:1", "gone");
        prose.extra = "My paragraph.".into();
        let mut unpinned = item("writing:out.md:1", "out");
        unpinned.lead = "Why I kept it.".into();
        let plain = item("writing:plain.md:1", "plain");
        let mut s = saved(vec![noted, prose, unpinned, plain]);
        s.excluded = vec!["writing:out.md:1".into()];
        // None of them matches any more.
        refresh_saved(&mut s, vec![item("writing:new.md:1", "new")], 20, "d2");
        let ids: Vec<&str> = s.items.iter().map(|i| i.passage.as_str()).collect();
        assert_eq!(ids, ["writing:new.md:1", "writing:old.md:1", "writing:gone.md:1", "writing:out.md:1"], "only the plain match left");
        assert_eq!(s.items[1].note, "my note");
        assert_eq!(s.items[2].extra, "My paragraph.");
        assert_eq!(s.items[3].lead, "Why I kept it.");
    }

    #[test]
    fn a_moved_passage_keeps_its_note_through_a_refresh() {
        // The passage moved from line 3 to line 9; the query now matches it at line 9.
        let mut it = item("writing:a.md:3", "The quick fox");
        it.note = "Open with this".into();
        it.extra = "And my paragraph.".into();
        let mut s = saved(vec![it]);
        let mut t = Table::default();
        t.by_id.insert("writing:a.md:9".into(), found("writing:a.md:9", "a.md", "The quick fox"));
        t.by_quote = vec![("The quick fox".into(), found("writing:a.md:9", "a.md", ""))];
        let res = repair_all(&mut s, &t);
        assert_eq!(res[0].0, Status::Repaired);
        refresh_saved(&mut s, vec![item("writing:a.md:9", "The quick fox")], 20, "d2");
        assert_eq!(s.items.len(), 1, "one item, not a fresh copy beside the old one");
        assert_eq!(s.items[0].passage, "writing:a.md:9");
        assert_eq!(s.items[0].note, "Open with this");
        assert_eq!(s.items[0].extra, "And my paragraph.");
        // An exclusion follows its passage's new id too.
        let mut s = saved(vec![item("writing:a.md:3", "The quick fox")]);
        s.excluded = vec!["writing:a.md:3".into()];
        repair_all(&mut s, &t);
        assert_eq!(s.excluded, ["writing:a.md:9"]);
    }

    #[test]
    fn an_edit_acts_on_the_item_it_names_or_is_refused() {
        // The page shows [A, B]; a background refresh made the file [C, A, B].
        let page = saved(vec![item("writing:a.md:1", "a"), item("writing:b.md:1", "b")]);
        let mut file = saved(vec![item("writing:c.md:1", "c"), item("writing:a.md:1", "a"), item("writing:b.md:1", "b")]);
        // Removing the A the page showed removes A, never C.
        apply_edit(&mut file, &SetEdit::Remove { item: pid(&page, 0) }, "d").unwrap();
        assert_eq!(file.items.iter().map(|i| i.passage.as_str()).collect::<Vec<_>>(), ["writing:c.md:1", "writing:b.md:1"]);
        assert_eq!(file.excluded, ["writing:a.md:1"]);
        // A move from positions the file no longer has there is refused, and changes nothing.
        let mut file = saved(vec![item("writing:c.md:1", "c"), item("writing:a.md:1", "a"), item("writing:b.md:1", "b")]);
        let before = file.clone();
        assert_eq!(apply_edit(&mut file, &mv(&page, 0, 1), "d").unwrap_err(), STALE);
        // Also when only the destination moved.
        let mut file2 = saved(vec![item("writing:a.md:1", "a"), item("writing:c.md:1", "c"), item("writing:b.md:1", "b")]);
        assert_eq!(apply_edit(&mut file2, &mv(&page, 0, 1), "d").unwrap_err(), STALE);
        assert_eq!(file, before);
        // A note or pin for an item that is gone is refused.
        assert!(apply_edit(&mut file, &SetEdit::Note { item: tid("writing:z.md:1"), note: "n".into() }, "d").is_err());
        assert!(apply_edit(&mut file, &SetEdit::Pin { item: tid("writing:z.md:1"), pinned: true }, "d").is_err());
        // A note lands on the named item wherever it now is.
        apply_edit(&mut file, &SetEdit::Note { item: tid("writing:a.md:1"), note: "on A".into() }, "d").unwrap();
        assert_eq!(file.items[1].note, "on A");
        assert!(file.items[0].note.is_empty());
    }

    #[test]
    fn an_older_file_gets_item_ids_in_file_order_and_no_other_byte_changes() {
        let old = format!("{PERSONAL}\n## 2\n\n- passage: \"writing:b.md:1\"\n- corpus: writing\n- source_path: \"b.md\"\n- note: \"second\"\n\n> Other text\n");
        assert!(parse(&old).unwrap().items.iter().all(|i| i.id.is_empty()));
        let root = tmp("old-ids");
        fs::create_dir_all(root.join("sets")).unwrap();
        let file = root.join("sets/talk.md");
        fs::write(&file, &old).unwrap();
        let store = SetStore::new(root.to_str().unwrap());
        // The next write (here: opening, which changes nothing else) gives every item an id.
        store.update("talk", |_| Ok(())).unwrap();
        let new = fs::read_to_string(&file).unwrap();
        let ids: Vec<String> = parse(&new).unwrap().items.into_iter().map(|i| i.id).collect();
        assert_eq!(ids.len(), 2);
        assert!(ids.iter().all(|i| i.len() == 12) && ids[0] != ids[1], "{ids:?}");
        // Exactly one `- item:` line per item, in file order, and nothing else changed.
        let item_lines: Vec<&str> = new.lines().filter(|l| l.starts_with("- item: ")).collect();
        assert_eq!(item_lines, [format!("- item: {}", ids[0]), format!("- item: {}", ids[1])]);
        let without: String = new.lines().filter(|l| !l.starts_with("- item: ")).map(|l| format!("{l}\n")).collect();
        assert_eq!(without, old);
        // From then on the ids stay: a no-op writes nothing, an edit keeps them.
        store.update("talk", |_| Ok(())).unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), new);
        store.update("talk", |s| apply_edit(s, &SetEdit::Note { item: ids[1].clone(), note: "changed".into() }, "d")).unwrap();
        let after = store.load("talk").unwrap();
        assert_eq!(after.items.iter().map(|i| i.id.clone()).collect::<Vec<_>>(), ids);
        assert_eq!(after.items[1].note, "changed");
        // A block copied in an editor (same id twice): the later copy gets an id of its own.
        let mut dup = after.clone();
        dup.items.push(dup.items[0].clone());
        assert!(ensure_item_ids(&mut dup));
        assert_eq!(dup.items[0].id, ids[0]);
        assert_ne!(dup.items[2].id, ids[0]);
        fs::remove_dir_all(root).unwrap();
    }

    /// A lookup where the file `a.md` now holds `lines` (line number, text).
    fn file_now(lines: &[(u32, &str)]) -> Table {
        let mut t = Table::default();
        for (n, q) in lines {
            let id = format!("writing:a.md:{n}");
            t.by_id.insert(id.clone(), found(&id, "a.md", q));
            t.by_quote.push((q.to_string(), found(&id, "a.md", "")));
        }
        t
    }

    #[test]
    fn two_identical_quotations_are_never_repaired_to_one_passage() {
        // Two unpinned matches, the same quotation at lines 3 and 5, each with its own note.
        let mut first = item("writing:a.md:3", "Same words");
        first.note = "on the first".into();
        let mut second = item("writing:a.md:5", "Same words");
        second.note = "on the second".into();
        second.extra = "And my paragraph.".into();
        let mut s = saved(vec![first.clone(), second.clone()]);
        // Text is inserted above both: they are now at lines 7 and 9.
        let t = file_now(&[(3, "New opening"), (5, "More new text"), (7, "Same words"), (9, "Same words")]);
        let res = repair_all(&mut s, &t);
        let repaired: Vec<&str> = s.items.iter().zip(&res).filter(|(_, r)| r.0 == Status::Repaired).map(|(i, _)| i.passage.as_str()).collect();
        let unique: std::collections::HashSet<_> = repaired.iter().collect();
        assert_eq!(unique.len(), repaired.len(), "two items were given one passage: {repaired:?}");
        // Both would take line 7; the second was closer (5 -> 7), so it does, and the first is "source changed".
        assert_eq!((res[1].0, s.items[1].passage.as_str()), (Status::Repaired, "writing:a.md:7"));
        assert_eq!(res[0].0, Status::Changed);
        assert_eq!(s.items[0].note, "on the first");
        // The refresh keeps both notes and the paragraph: nothing the user wrote is deleted.
        refresh_saved(&mut s, vec![item("writing:a.md:7", "Same words"), item("writing:a.md:9", "Same words")], 20, "d2");
        let by_id = |id: &str| s.items.iter().find(|i| i.id == id).cloned().expect(id);
        assert_eq!(by_id(&first.id).note, "on the first");
        assert_eq!(by_id(&second.id).note, "on the second");
        assert_eq!(by_id(&second.id).extra, "And my paragraph.");
        let ids: std::collections::HashSet<_> = s.items.iter().map(|i| i.id.clone()).collect();
        assert_eq!(ids.len(), s.items.len(), "every item keeps an id of its own");
        // A later repair never gives the first item a passage another item holds.
        let res = repair_all(&mut s, &t);
        let held: Vec<_> = s.items.iter().zip(&res).filter(|(_, r)| r.0 != Status::Changed).map(|(i, _)| i.passage.clone()).collect();
        assert_eq!(held.iter().collect::<std::collections::HashSet<_>>().len(), held.len(), "{held:?}");
    }

    #[test]
    fn two_items_already_sharing_a_passage_are_settled_one_to_one() {
        // An older version saved two annotated items on one passage; it still holds their quotation.
        let mut first = item("writing:a.md:3", "Same words");
        first.note = "on the first".into();
        let mut second = item("writing:a.md:3", "Same words");
        second.id = tid("second");
        second.note = "on the second".into();
        second.extra = "My paragraph.".into();
        let mut s = saved(vec![first, second]);
        let res = repair_all(&mut s, &file_now(&[(3, "Same words")]));
        // The first keeps the passage; the second is "source changed", its note and prose untouched.
        assert_eq!(res.iter().map(|r| r.0).collect::<Vec<_>>(), [Status::Ok, Status::Changed]);
        assert_eq!(s.items[1].passage, "writing:a.md:3");
        assert_eq!((s.items[1].note.as_str(), s.items[1].extra.as_str()), ("on the second", "My paragraph."));
    }

    #[test]
    fn an_older_items_prose_is_never_read_as_its_id_or_note() {
        // Before ids, `- item: shopping` started the user's own text, and the `- note:` after it was text too.
        let old = format!(
            "{}\n## 1\n\n- passage: \"writing:b.md:1\"\n- corpus: writing\n- source_path: \"b.md\"\n- note: \"The real note\"\n\
             - item: shopping\n- note: \"Example\"\n\n> Quoted\n",
            PERSONAL.split("\n## 1").next().unwrap()
        );
        let p = parse(&old).unwrap();
        let it = &p.items[0];
        assert_eq!(it.id, "");
        assert_eq!(it.note, "The real note");
        assert_eq!(it.lead, "- item: shopping\n- note: \"Example\"");
        // Only the first metadata line, in the id format, is the id.
        let first = old.replace("- passage: \"writing:b", "- item: 0123456789ab\n- passage: \"writing:b");
        assert_eq!(parse(&first).unwrap().items[0].id, "0123456789ab");
        let later = old.replace("- corpus: writing\n", "- corpus: writing\n- item: 0123456789ab\n");
        assert_eq!(parse(&later).unwrap().items[0].id, "");
        // Giving the item its id keeps the note and the text.
        let root = tmp("prose-ids");
        fs::create_dir_all(root.join("sets")).unwrap();
        fs::write(root.join("sets/talk.md"), &old).unwrap();
        let store = SetStore::new(root.to_str().unwrap());
        store.update("talk", |_| Ok(())).unwrap();
        let after = store.load("talk").unwrap();
        assert_eq!(after.items[0].id.len(), 12);
        assert_eq!(after.items[0].note, "The real note");
        assert_eq!(after.items[0].lead, it.lead);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_copied_items_new_id_replaces_its_id_line_and_keeps_every_other_byte() {
        // Hand-edited: CRLF endings, the user's own heading numbers, a block copied with its id.
        let block = |n: u32, p: &str| {
            format!("## {n}\r\n\r\n- item: 0123456789ab\r\n- passage: \"{p}\"\r\n- corpus: writing\r\n- source_path: \"a.md\"\r\n\r\n> Words\r\n\r\n")
        };
        let old = format!(
            "---\r\ntitle: \"Talk\"\r\ncreated: 2026-10-20\r\nupdated: 2026-10-20\r\nkind: hand\r\n---\r\n\r\n{}{}",
            block(5, "writing:a.md:1"),
            block(9, "writing:a.md:2")
        );
        let root = tmp("dup-ids");
        fs::create_dir_all(root.join("sets")).unwrap();
        let file = root.join("sets/talk.md");
        fs::write(&file, &old).unwrap();
        let store = SetStore::new(root.to_str().unwrap());
        store.update("talk", |_| Ok(())).unwrap();
        let new = fs::read_to_string(&file).unwrap();
        let ids: Vec<String> = parse(&new).unwrap().items.into_iter().map(|i| i.id).collect();
        assert_eq!(ids[0], "0123456789ab");
        assert!(ids[1] != ids[0] && valid_item_id(&ids[1]), "{ids:?}");
        // Exactly the copy's id line changed: the same bytes with the old id put back.
        let second = new.rfind(&format!("- item: {}\r\n", ids[1])).expect("the new id line, CRLF-ended");
        let mut back = new.clone();
        back.replace_range(second..second + 8 + 12, "- item: 0123456789ab");
        assert_eq!(back, old);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn an_edit_for_a_moved_item_never_lands_on_the_item_that_took_its_line() {
        // The page shows A at line 3.
        let mut a = item("writing:a.md:3", "The quick fox");
        a.id = "item-a".into();
        let mut s = saved(vec![a]);
        // A rebuild: A moved to line 9, and C (also a match) is now at line 3.
        let t = file_now(&[(3, "C took this line"), (9, "The quick fox")]);
        repair_all(&mut s, &t);
        let mut c = item("writing:a.md:3", "C took this line");
        c.id = "item-c".into();
        refresh_saved(&mut s, vec![c, item("writing:a.md:9", "The quick fox")], 20, "d2");
        assert_eq!(s.items.len(), 2);
        let at = |s: &SetFile, id: &str| s.items.iter().find(|i| i.id == id).cloned().unwrap();
        assert_eq!(at(&s, "item-a").passage, "writing:a.md:9");
        assert_eq!(at(&s, "item-c").passage, "writing:a.md:3");
        // Before the page reloads, it saves A's note: it lands on A, never on C.
        apply_edit(&mut s, &SetEdit::Note { item: "item-a".into(), note: "For A".into() }, "d").unwrap();
        assert_eq!(at(&s, "item-a").note, "For A");
        assert!(at(&s, "item-c").note.is_empty());
        // Pin and remove act on A too.
        apply_edit(&mut s, &SetEdit::Pin { item: "item-a".into(), pinned: true }, "d").unwrap();
        assert!(at(&s, "item-a").pinned && !at(&s, "item-c").pinned);
        apply_edit(&mut s, &SetEdit::Remove { item: "item-a".into() }, "d").unwrap();
        assert_eq!(s.items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(), ["item-c"]);
        // An item that is gone is refused, and nothing changes.
        let before = s.clone();
        assert_eq!(apply_edit(&mut s, &SetEdit::Note { item: "item-a".into(), note: "x".into() }, "d").unwrap_err(), STALE);
        assert_eq!(s, before);
    }

    #[test]
    fn an_outside_change_to_what_an_edit_touches_is_never_replaced() {
        let root = tmp("outside-note");
        let mut set = SetFile::new("Talk", SetKind::Hand, None, "d");
        let mut it = item("writing:a.md:3", "Quoted");
        it.note = "Original".into();
        set.items = vec![it.clone(), item("writing:b.md:1", "Other")];
        let id = SetStore::new(root.to_str().unwrap()).create(&set).unwrap();
        let file = root.join("sets").join(format!("{id}.md"));
        // The edit the page sends, guarded as `set_edit` guards it.
        let note = |store: &SetStore, text: &str| {
            let e = SetEdit::Note { item: it.id.clone(), note: text.into() };
            store.update_guarded(&id, |s| footprint(s, &e), |s| apply_edit(s, &e, "d2"))
        };
        // The app read note "Original"; another app saves "External revision" before the app replaces the file.
        let outside = |change: &'static str| {
            let (file, fired) = (file.clone(), Arc::new(std::sync::atomic::AtomicBool::new(false)));
            SetStore::new(root.to_str().unwrap()).with_hook(move |at| {
                if at == "before-replace" && !fired.swap(true, std::sync::atomic::Ordering::SeqCst) {
                    let text = fs::read_to_string(&file).unwrap();
                    let changed = match change {
                        "note" => text.replace("\"Original\"", "\"External revision\""),
                        _ => text.replace("title: \"Talk\"", "title: \"Renamed elsewhere\""),
                    };
                    fs::write(&file, changed).unwrap();
                }
            })
        };
        let err = note(&outside("note"), "App revision").unwrap_err();
        assert!(err.contains("changed by another app") && err.contains("nothing was overwritten"), "{err}");
        let after = fs::read_to_string(&file).unwrap();
        assert!(after.contains("\"External revision\"") && !after.contains("App revision"), "the other app's revision stands:\n{after}");
        // A change elsewhere in the file (the title) does not touch the note: the edit is made on top of it.
        note(&outside("title"), "App revision").unwrap();
        let after = store_load(&root, &id);
        assert_eq!(after.title, "Renamed elsewhere");
        assert_eq!(after.items[0].note, "App revision");
        // A rename is refused the same way when the other app renamed the set too.
        let e = SetEdit::Rename { title: "Mine".into() };
        let store = outside("title");
        fs::write(&file, fs::read_to_string(&file).unwrap().replace("Renamed elsewhere", "Talk")).unwrap();
        assert!(store.update_guarded(&id, |s| footprint(s, &e), |s| apply_edit(s, &e, "d3")).is_err());
        assert_eq!(store_load(&root, &id).title, "Renamed elsewhere");
        let left: Vec<_> = fs::read_dir(root.join("sets")).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(left.len(), 1, "no temp file is left: {left:?}");
        fs::remove_dir_all(root).unwrap();
    }

    fn store_load(root: &Path, id: &str) -> SetFile {
        SetStore::new(root.to_str().unwrap()).load(id).unwrap()
    }
}
