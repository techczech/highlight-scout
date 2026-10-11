//! Writing tweet works into the archive: one file per tweet, found by its
//! key (`source_system: x` plus `source_id`) whatever the file is called.
//!
//! A tweet's slug is built from its text, so the same tweet imported again
//! with fuller text has a different slug. Writing by slug therefore made a
//! second file. `write_archive` here looks the tweet up by key instead.
//!
//! One rule governs an existing file: it is changed only when the importer
//! has positively identified, in that file, the one block that is this
//! tweet. Then, in that block only:
//!
//! - the text is replaced when the file holds an error page, or when the
//!   tweet's own text in the file is a strict prefix of the incoming one
//!   (`replacement`); context lines the block already has are kept;
//! - a date on the highlight line is kept, a missing or empty one is filled;
//! - tags are unioned, the file's own first.
//!
//! The frontmatter is left as it is, except `updated_at` when that edit is
//! provably safe. A merge that changes nothing writes nothing. When the
//! block cannot be identified (the file is not laid out as the app writes
//! it, a block does not read back, none or several blocks could be the
//! tweet, the file changed on disk mid-import), nothing is written, no
//! second file is made, the caller is told to leave the index alone for
//! that tweet, and the key is reported.
//!
//! Only a tweet whose key no file carries is rendered fresh, under a
//! filename nothing else occupies.

use anyhow::Result;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::archive_meta::{self, WriteSummary};
use crate::import::merge_blocks::{normalise, Block};
use crate::import::tweet_common::{is_error_page, split_context};
use crate::import::tweet_frontmatter::{self, Frontmatter};
use crate::models::{Highlight, Work};

/// A tweet the importer left alone, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    pub key: String,
    pub reason: &'static str,
}

/// What a write pass did with the batch's tweets.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TweetReport {
    pub created: usize,
    pub updated: usize,
    pub unchanged: usize,
    /// Keys (`x:<tweet id>`) the archive holds more than one file for.
    pub duplicate_keys: Vec<String>,
    /// Tweets whose file was left exactly as it was, with the index.
    pub skipped: Vec<Skipped>,
    /// Keys whose index row could not be pointed at the tweet's file (the
    /// persist step fills this in); the index was left as it was for them.
    pub index_slug_conflicts: Vec<String>,
}

impl TweetReport {
    /// A sentence for the import log; None when there is nothing to say.
    pub fn log_message(&self) -> Option<String> {
        let mut parts = Vec::new();
        if !self.duplicate_keys.is_empty() {
            parts.push(format!(
                "{} tweets have more than one file in the archive (the fuller one was updated, no new file made): {}",
                self.duplicate_keys.len(),
                self.duplicate_keys.join(", ")
            ));
        }
        if !self.skipped.is_empty() {
            let list: Vec<String> = self
                .skipped
                .iter()
                .map(|s| format!("{} ({})", s.key, s.reason))
                .collect();
            parts.push(format!(
                "{} tweets were left as they are in the archive and the index: {}",
                list.len(),
                list.join(", ")
            ));
        }
        if !self.index_slug_conflicts.is_empty() {
            parts.push(format!(
                "{} tweets were updated in the archive but not in the index, where another entry holds their file name: {}",
                self.index_slug_conflicts.len(),
                self.index_slug_conflicts.join(", ")
            ));
        }
        (!parts.is_empty()).then(|| parts.join(" · "))
    }
}

/// The result of `write_archive`.
#[derive(Debug, Default)]
pub struct Written {
    /// Files written or left alone, tweets and other works together.
    pub summary: WriteSummary,
    pub tweets: TweetReport,
    /// Each tweet highlight as it now stands in the archive, by highlight
    /// id, for the index.
    pub highlights: HashMap<String, Highlight>,
    /// Ids of tweet works whose file was left alone: the index must not be
    /// given the batch's data for these, work or highlight.
    pub skipped_works: HashSet<String>,
}

pub fn is_tweet(work: &Work) -> bool {
    work.source_system == "x" && work.source_id.as_deref().is_some_and(|i| !i.is_empty())
}

/// `x:<tweet id>`, as `Frontmatter::key` gives it for the tweet's file.
pub fn key(work: &Work) -> String {
    format!(
        "{}:{}",
        work.source_system,
        work.source_id.as_deref().unwrap_or_default()
    )
}

/// `archive_meta::write_archive` for a batch that may hold tweets. Works
/// from other sources are rendered as before. Each tweet work is merged
/// into the file that carries its key; its `slug`, and its `title`,
/// `author` and `url` where the file gives them, are set to that file's, so
/// the index describes the file that exists.
pub fn write_archive(
    archive_path: &str,
    works: &mut [Work],
    highlights_by_work: &HashMap<String, Vec<&Highlight>>,
) -> Result<Written> {
    let mut out = Written::default();
    if !works.iter().any(is_tweet) {
        out.summary = archive_meta::write_archive(archive_path, works, highlights_by_work)?;
        return Ok(out);
    }

    let others: Vec<Work> = works.iter().filter(|w| !is_tweet(w)).cloned().collect();
    out.summary = archive_meta::write_archive(archive_path, &others, highlights_by_work)?;

    let dir = Path::new(archive_path).join("readings").join("works");
    let by_key = tweet_frontmatter::scan(&dir, "x");
    for work in works.iter_mut().filter(|w| is_tweet(w)) {
        let incoming = highlights_by_work
            .get(&work.id)
            .and_then(|hs| fold_batch(hs));
        let files = by_key.get(&key(work)).cloned().unwrap_or_default();
        let merged = match (files.is_empty(), incoming) {
            (true, incoming) => {
                create(archive_path, &dir, work, incoming.as_ref(), &mut out.tweets)?;
                incoming
            }
            // A tweet work with no highlight in the batch has nothing to
            // merge into its file.
            (false, None) => None,
            (false, Some(incoming)) => merge(&files, work, incoming, &mut out.tweets)?,
        };
        match merged {
            Some(h) => {
                out.highlights.insert(h.id.clone(), h);
            }
            None if files.is_empty() => {}
            None => {
                out.skipped_works.insert(work.id.clone());
            }
        }
    }
    out.summary.written += out.tweets.created + out.tweets.updated;
    out.summary.unchanged += out.tweets.unchanged;
    Ok(out)
}

/// Render a tweet no file carries the key of. A file already at the work's
/// slug belongs to something else and is never touched: the tweet goes
/// under the first free `<slug>-<n>` instead.
fn create(
    archive_path: &str,
    dir: &Path,
    work: &mut Work,
    incoming: Option<&Highlight>,
    report: &mut TweetReport,
) -> Result<()> {
    let taken = |slug: &str| dir.join(format!("{slug}.md")).symlink_metadata().is_ok();
    if taken(&work.slug) {
        let base = work.slug.clone();
        work.slug = (2u32..)
            .map(|n| format!("{base}-{n}"))
            .find(|s| !taken(s))
            .unwrap_or(base);
    }
    let mut by_work: HashMap<String, Vec<&Highlight>> = HashMap::new();
    by_work.insert(work.id.clone(), incoming.into_iter().collect());
    let s = archive_meta::write_archive(archive_path, std::slice::from_ref(work), &by_work)?;
    report.created += s.written;
    report.unchanged += s.unchanged;
    Ok(())
}

/// One highlight for a tweet from the batch's highlights for it (a tweet
/// both liked and bookmarked arrives twice): text by the same rule as a
/// file's, first date, tags unioned.
fn fold_batch(highlights: &[&Highlight]) -> Option<Highlight> {
    let mut it = highlights.iter();
    let mut out: Highlight = (*it.next()?).clone();
    for h in it {
        if let Some(text) = replacement(&out.text, &h.text) {
            out.text = text;
            out.source_data = h.source_data.clone();
        }
        if out.created_at.is_none() {
            out.created_at = h.created_at.clone();
        }
        for t in &h.tags {
            if !out.tags.contains(t) {
                out.tags.push(t.clone());
            }
        }
    }
    Some(out)
}

fn trailing_tco() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\s*https?://t\.co/\w+$").unwrap())
}

/// A tweet's own text made comparable across sources: HTML entities
/// resolved, whitespace collapsed, and the ellipsis and t.co link that
/// close a cut-off tweet taken off the end.
fn comparable(own: &str) -> String {
    let unescaped = own
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&");
    let mut s = normalise(&unescaped);
    loop {
        let mut t = trailing_tco().replace(&s, "").trim_end().to_string();
        for ellipsis in ["…", "..."] {
            if let Some(cut) = t.strip_suffix(ellipsis) {
                t = cut.trim_end().to_string();
            }
        }
        if t == s {
            return s;
        }
        s = t;
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Relation {
    Same,
    /// The existing text is a strict prefix of the incoming one.
    IncomingExtends,
    /// The incoming text is a strict prefix of the existing one.
    ExistingExtends,
    Unrelated,
}

/// How two tweet bodies' own texts relate. Text with nothing comparable in
/// it is related only to another such text.
fn relation(existing_body: &str, incoming_body: &str) -> Relation {
    let e = comparable(split_context(existing_body).0);
    let i = comparable(split_context(incoming_body).0);
    if e == i {
        Relation::Same
    } else if e.is_empty() || i.is_empty() {
        Relation::Unrelated
    } else if i.starts_with(&e) {
        Relation::IncomingExtends
    } else if e.starts_with(&i) {
        Relation::ExistingExtends
    } else {
        Relation::Unrelated
    }
}

/// The body to put in place of `existing`, or None to keep it. Only two
/// cases replace: the existing body is an error page saved in place of the
/// tweet, or the tweet's own text in it is a strict prefix of the incoming
/// tweet's own text. In the second case context the existing body carries
/// after the tweet's text (a parent or quoted tweet, links, images) is kept
/// as it is and the incoming context dropped; an existing body without
/// context takes the whole incoming body. Empty text and error pages never
/// replace anything.
pub fn replacement(existing: &str, incoming: &str) -> Option<String> {
    if incoming.trim().is_empty() || is_error_page(incoming) {
        return None;
    }
    if is_error_page(existing) {
        return Some(incoming.to_string());
    }
    if relation(existing, incoming) != Relation::IncomingExtends {
        return None;
    }
    let context = split_context(existing).1;
    Some(if context.is_empty() {
        incoming.to_string()
    } else {
        format!("{}\n\n{}", split_context(incoming).0, context)
    })
}

/// A tweet file in which the tweet's block has been identified.
struct Candidate {
    path: PathBuf,
    /// The file as loaded; a write is abandoned if the file no longer
    /// holds exactly this.
    original: String,
    head: String,
    blocks: Vec<String>,
    index: usize,
    block: Block,
    frontmatter: Frontmatter,
}

/// What `Candidate::commit` did.
#[derive(Debug, PartialEq, Eq)]
enum Commit {
    Written,
    ChangedOnDisk,
}

impl Candidate {
    /// Open `path` and find the block that is the tweet. An error is the
    /// reason the file must be left alone.
    fn load(path: &Path, key: &str, incoming_body: &str) -> Result<Candidate, &'static str> {
        let original = fs::read_to_string(path).map_err(|_| "the file could not be read")?;
        let frontmatter =
            Frontmatter::of_file(&original).ok_or("the file has no readable frontmatter")?;
        if frontmatter.key().as_deref() != Some(key) {
            return Err("the file's frontmatter does not give this tweet's key");
        }
        let (head, blocks) = archive_meta::split_file_records(&original)
            .filter(|(head, _)| !head.is_empty())
            .ok_or("the file is not laid out as the app writes it")?;
        let parsed: Vec<Block> = blocks
            .iter()
            .map(|b| Block::parse(b))
            .collect::<Option<_>>()
            .ok_or("the file holds a highlight the app cannot read back")?;
        let index = match parsed.len() {
            0 => return Err("the file holds no highlight"),
            1 => 0,
            _ => {
                let mut matching = parsed
                    .iter()
                    .enumerate()
                    .filter(|(_, b)| relation(&b.text(), incoming_body) != Relation::Unrelated)
                    .map(|(i, _)| i);
                match (matching.next(), matching.next()) {
                    (Some(i), None) => i,
                    (None, _) => return Err("none of the file's highlights is this tweet"),
                    _ => return Err("more than one of the file's highlights could be this tweet"),
                }
            }
        };
        Ok(Candidate {
            path: path.to_path_buf(),
            block: parsed[index].clone(),
            original,
            head,
            blocks,
            index,
            frontmatter,
        })
    }

    fn slug(&self) -> String {
        self.path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    /// Replace the file with `content`, unless it no longer holds what was
    /// loaded (another writer got there first).
    fn commit(&self, content: &str) -> Result<Commit> {
        match fs::read(&self.path) {
            Ok(now) if now == self.original.as_bytes() => {}
            _ => return Ok(Commit::ChangedOnDisk),
        }
        archive_meta::write_if_changed(&self.path, content.as_bytes())?;
        Ok(Commit::Written)
    }
}

/// Merge one tweet into the file that carries its key. Returns the
/// highlight as it now stands in the archive, or None when no file could be
/// updated safely (nothing was written; the key is in `report.skipped`).
fn merge(
    files: &[PathBuf],
    work: &mut Work,
    incoming: Highlight,
    report: &mut TweetReport,
) -> Result<Option<Highlight>> {
    let key = key(work);
    if files.len() > 1 {
        report.duplicate_keys.push(key.clone());
    }

    // Of several files, the one whose tweet text is fuller; the first by
    // name otherwise.
    let mut target: Option<Candidate> = None;
    let mut refusal = None;
    for path in files {
        match Candidate::load(path, &key, &incoming.text) {
            Err(reason) => refusal = refusal.or(Some(reason)),
            Ok(c) => {
                let fuller = target.as_ref().is_none_or(|best| {
                    let (b, n) = (best.block.text(), c.block.text());
                    (is_error_page(&b) && !is_error_page(&n))
                        || relation(&b, &n) == Relation::IncomingExtends
                });
                if fuller {
                    target = Some(c);
                }
            }
        }
    }
    let Some(mut c) = target else {
        report.skipped.push(Skipped {
            key,
            reason: refusal.unwrap_or("the file could not be read"),
        });
        return Ok(None);
    };

    let before = c.block.clone();
    if let Some(text) = replacement(&c.block.text(), &incoming.text) {
        c.block.set_text(&text);
    }
    let incoming_day = incoming
        .created_at
        .as_deref()
        .map(|d| d.split('T').next().unwrap_or(d).trim().to_string())
        .filter(|d| !d.is_empty());
    let dated = c
        .block
        .get("highlighted_at")
        .is_some_and(|d| !d.trim().is_empty());
    if let (false, Some(day)) = (dated, &incoming_day) {
        c.block.set("highlighted_at", day.clone());
    }
    let mut tags = c.block.tags();
    for t in &incoming.tags {
        if !tags.contains(t) {
            tags.push(t.clone());
        }
    }
    if tags != c.block.tags() {
        c.block.set("tags", tags.join(", "));
    }

    if c.block == before {
        report.unchanged += 1;
    } else {
        let mut blocks = c.blocks.clone();
        blocks[c.index] = c.block.render();
        let head = tweet_frontmatter::set_updated_at(&c.head, &work.updated_at)
            .unwrap_or_else(|| c.head.clone());
        let content = archive_meta::join_file_records(&head, &blocks);
        // What is written must read back the way it was built, or a later
        // import could not find the tweet in it.
        let reads_back = archive_meta::split_file_records(&content)
            .is_some_and(|(h, b)| h == head && b == blocks)
            && Block::parse(&blocks[c.index]).as_ref() == Some(&c.block);
        if !reads_back {
            report.skipped.push(Skipped {
                key,
                reason: "the updated file would not read back as written",
            });
            return Ok(None);
        }
        match c.commit(&content)? {
            Commit::Written => report.updated += 1,
            Commit::ChangedOnDisk => {
                report.skipped.push(Skipped {
                    key,
                    reason: "the file changed on disk during the import",
                });
                return Ok(None);
            }
        }
    }

    work.slug = c.slug();
    if let Some(title) = c.frontmatter.get("title") {
        work.title = title;
    }
    if let Some(author) = c.frontmatter.get("author") {
        work.author = Some(author);
    }
    if let Some(url) = c.frontmatter.get("url") {
        work.url = Some(url);
    }

    let mut merged = incoming;
    merged.text = c.block.text();
    merged.tags = c.block.tags();
    merged.note = c.block.note.clone().filter(|n| !n.trim().is_empty());
    let file_day = c
        .block
        .get("highlighted_at")
        .map(|d| d.trim().to_string())
        .filter(|d| !d.is_empty());
    if file_day != incoming_day {
        merged.created_at = file_day;
    }
    Ok(Some(merged))
}

#[cfg(test)]
#[path = "tweet_upsert_tests.rs"]
mod tests;
