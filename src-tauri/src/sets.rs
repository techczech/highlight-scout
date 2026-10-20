//! Sets (HS-2A): a hand-picked, ordered collection of passages for a talk,
//! kept as one Markdown file per set in `<archive_path>/sets/<id>.md`. That
//! file is the only state a set has; it is git-tracked with the archive.
//!
//! This module is the seam for everything about a set that is not the engine:
//! - the file format: [`parse`] and [`render`] are exact inverses on any file
//!   this module wrote (byte-stable: order, notes, quotes survive a reopen),
//! - the store: [`SetStore`] reads and writes only inside its `sets/` dir
//!   (ids are validated, never joined to a path unchecked; writes are
//!   temp-file + rename),
//! - editing: [`apply_edit`] (move, note, remove, pin, rename),
//! - resolving items against the engine through the [`Lookup`] trait: by
//!   passage id first, then by `verify-quote` on the stored quote (repairing
//!   the id), else "changed", keeping the stored quote ([`resolve`]),
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
use std::fs;
use std::path::{Path, PathBuf};

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

/// The set as its file text. Item numbers are written for the reader and
/// ignored on parse; the order of the items is the file's order.
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
    if !s.dismissed.is_empty() {
        o.push_str("dismissed:\n");
        for d in &s.dismissed {
            o.push_str(&format!("  - {}\n", js(d)));
        }
    }
    o.push_str("---\n");
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
    }
    o
}

/// Read a set file. Unknown frontmatter keys are ignored; a malformed item
/// is an error naming it (a half-read set would be rewritten over the file).
pub fn parse(text: &str) -> Result<SetFile, String> {
    let mut lines = text.lines();
    if lines.next().map(str::trim_end) != Some("---") {
        return Err("a set file starts with ---".into());
    }
    let mut set = SetFile::new("", SetKind::Hand, None, "");
    let mut in_dismissed = false;
    let mut closed = false;
    for l in lines.by_ref() {
        if l.trim_end() == "---" {
            closed = true;
            break;
        }
        if let Some(rest) = l.strip_prefix("  - ") {
            if in_dismissed {
                set.dismissed.push(unjs(rest)?);
                continue;
            }
        }
        in_dismissed = false;
        let Some((k, v)) = l.split_once(':') else { continue };
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
            "dismissed" => in_dismissed = true,
            _ => {}
        }
    }
    if !closed {
        return Err("the set's frontmatter is not closed".into());
    }
    if set.title.is_empty() {
        return Err("the set has no title".into());
    }
    let mut cur: Option<(usize, SetItem, Vec<String>)> = None;
    let mut n = 0usize;
    let finish = |c: Option<(usize, SetItem, Vec<String>)>, set: &mut SetFile| -> Result<(), String> {
        if let Some((no, mut it, q)) = c {
            if it.passage.is_empty() {
                return Err(format!("item {no} has no passage"));
            }
            it.quote = q.join("\n");
            set.items.push(it);
        }
        Ok(())
    };
    for l in lines {
        if l.starts_with("## ") {
            finish(cur.take(), &mut set)?;
            n += 1;
            cur = Some((
                n,
                SetItem {
                    passage: String::new(),
                    corpus: String::new(),
                    source_path: String::new(),
                    quote: String::new(),
                    pinned: false,
                    note: String::new(),
                },
                vec![],
            ));
            continue;
        }
        let Some((no, it, q)) = cur.as_mut() else { continue };
        if let Some(rest) = l.strip_prefix("- ") {
            let Some((k, v)) = rest.split_once(':') else { continue };
            let v = v.trim();
            match k {
                "passage" => it.passage = unjs(v).map_err(|e| format!("item {no}: {e}"))?,
                "corpus" => it.corpus = v.to_string(),
                "source_path" => it.source_path = unjs(v).map_err(|e| format!("item {no}: {e}"))?,
                "pinned" => it.pinned = v == "true",
                "note" => it.note = unjs(v).map_err(|e| format!("item {no}: {e}"))?,
                _ => {}
            }
        } else if l == ">" {
            q.push(String::new());
        } else if let Some(rest) = l.strip_prefix("> ") {
            q.push(rest.to_string());
        }
    }
    finish(cur.take(), &mut set)?;
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

pub struct SetStore {
    dir: PathBuf,
}

/// A set id is the file's stem: lower-case letters, digits, `-`, `_`.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

impl SetStore {
    /// The store of an archive: `<archive_path>/sets`.
    pub fn new(archive_path: &str) -> SetStore {
        SetStore { dir: Path::new(archive_path).join("sets") }
    }

    fn path(&self, id: &str) -> Result<PathBuf, String> {
        if !valid_id(id) {
            return Err(format!("{id:?} is not a set id"));
        }
        Ok(self.dir.join(format!("{id}.md")))
    }

    pub fn load(&self, id: &str) -> Result<SetFile, String> {
        let p = self.path(id)?;
        let text = fs::read_to_string(&p).map_err(|e| format!("set {id}: {e}"))?;
        parse(&text).map_err(|e| format!("set {id}: {e}"))
    }

    /// Write the set (temp file in the same dir, then rename).
    pub fn save(&self, id: &str, set: &SetFile) -> Result<(), String> {
        let p = self.path(id)?;
        fs::create_dir_all(&self.dir).map_err(|e| format!("sets dir: {e}"))?;
        let tmp = self.dir.join(format!(".{id}.md.tmp"));
        fs::write(&tmp, render(set)).map_err(|e| format!("set {id}: {e}"))?;
        fs::rename(&tmp, &p).map_err(|e| {
            let _ = fs::remove_file(&tmp);
            format!("set {id}: {e}")
        })
    }

    /// A new set under an unused id taken from its title.
    pub fn create(&self, set: &SetFile) -> Result<String, String> {
        let mut base: String = slug::slugify(&set.title);
        base.truncate(60);
        let base = base.trim_matches('-').to_string();
        let base = if base.is_empty() { "set".to_string() } else { base };
        let mut id = base.clone();
        let mut n = 1;
        while self.path(&id)?.exists() {
            n += 1;
            id = format!("{base}-{n}");
        }
        self.save(&id, set)?;
        Ok(id)
    }

    /// Every set, most recently updated first.
    pub fn list(&self) -> Vec<SetSummary> {
        let Ok(rd) = fs::read_dir(&self.dir) else { return vec![] };
        let mut out: Vec<SetSummary> = rd
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let p = e.path();
                let id = p.file_stem()?.to_str()?.to_string();
                if p.extension()?.to_str()? != "md" || !valid_id(&id) {
                    return None;
                }
                let s = parse(&fs::read_to_string(&p).ok()?).ok()?;
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
            s.items.remove(at(*index)?);
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
        pinned: false,
        note: String::new(),
    }
}

/// Add an item at the end; `false` when the set already holds that passage.
pub fn add_item(s: &mut SetFile, it: SetItem, today: &str) -> bool {
    if s.items.iter().any(|x| x.passage == it.passage) {
        return false;
    }
    s.dismissed.retain(|d| *d != it.passage);
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
/// become `matches` (those not already pinned, in match order, not
/// dismissed), after the pinned ones. Returns whether anything changed.
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
        if items.iter().any(|i| i.passage == m.passage) || s.dismissed.contains(&m.passage) {
            continue;
        }
        let note = before.iter().find(|i| i.passage == m.passage).map(|i| i.note.clone()).unwrap_or_default();
        items.push(SetItem { note, ..m });
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

/// A passage still holds the quote when one text contains the other
/// (whitespace aside): the quote is the passage, or a part of it.
fn holds(passage: &str, quote: &str) -> bool {
    let (p, q) = (norm(passage), norm(quote));
    !q.is_empty() && (p.contains(&q) || q.contains(&p))
}

/// Where an item's passage is now: by id first, then by its quote.
pub fn resolve(it: &SetItem, l: &dyn Lookup) -> (Status, Option<Found>) {
    if let Some(f) = l.cite(&it.passage) {
        if holds(&f.quote, &it.quote) {
            return (Status::Ok, Some(f));
        }
    }
    let mut found = l.find_quote(&it.corpus, &it.quote);
    // The same file first: a moved line, not another document with the same words.
    found.sort_by_key(|f| f.rel_path != it.source_path);
    for f in found {
        if let Some(c) = l.cite(&f.passage_id) {
            return (Status::Repaired, Some(c));
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
        SetItem { passage: p.into(), corpus, source_path: path, quote: q.into(), pinned: false, note: String::new() }
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
