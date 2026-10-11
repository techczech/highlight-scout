//! Writing tweet works into the archive: one file per tweet, found by its
//! key (`source_system: x` plus `source_id`) whatever the file is called.
//!
//! A tweet's slug is built from its text, so the same tweet imported again
//! with fuller text has a different slug. Writing by slug therefore made a
//! second file. `write_archive` here looks the tweet up by key instead.
//!
//! The importer updates an existing file only in the shape the app itself
//! writes. In any other shape it leaves the file and the index exactly as
//! they are for that tweet and reports the key with the reason. A missed
//! update is acceptable; changing the wrong content is not.
//!
//! The shape: the file's frontmatter gives the tweet's key
//! (`tweet_frontmatter`); the file splits into blocks that each read back
//! byte for byte; and exactly one block is related to the incoming tweet by
//! text (`related`). In that block only:
//!
//! - a cut-off text (it ends with `…` or `...`, optionally followed by one
//!   t.co link) is replaced by an incoming text that continues it with more
//!   than a link (`replacement`); every other text is kept; context lines the block has
//!   are kept and incoming context is not added;
//! - a date on the highlight line is kept, a missing or empty one is filled;
//! - tags are unioned, the file's own first.
//!
//! Of the frontmatter only `updated_at` changes, and only when it is a plain
//! single-line value. A merge that changes nothing writes nothing. The
//! replacement is prepared in a temp file first and published only if the
//! file still holds the bytes that were loaded.
//!
//! A tweet whose key no file holds is rendered fresh under a name nothing
//! else has, published without replacing anything. An incoming text that is
//! an error page saved in place of the tweet is not written at all, and
//! neither is a work from X that carries no tweet id.
//!
//! Threat model. Highlight Scout is a single-user desktop app. What the
//! importer reads is the user's own Readwise account, X API pulls, and files
//! the app wrote, which the user may since have edited by hand. An import
//! file crafted to deceive the importer is outside the model; even so, no
//! import may write outside the works directory. Where the importer cannot
//! tell what a file or a record is, it does nothing rather than guess.

use anyhow::Result;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

use crate::archive_meta::{self, PreparedReplacement, WriteSummary};
use crate::import::merge_blocks::{normalise, Block};
use crate::import::tweet_common::{comparable, is_error_page, split_context};
use crate::import::tweet_frontmatter::{self, Frontmatter};
use crate::models::{Highlight, Work};

/// A tweet the importer left alone, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    pub key: String,
    pub reason: String,
}

/// What a write pass did with the batch's tweets.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TweetReport {
    pub created: usize,
    pub updated: usize,
    pub unchanged: usize,
    /// Keys (`x:<tweet id>`) the archive holds more than one file for.
    pub duplicate_keys: Vec<String>,
    /// Tweets for which nothing was written and the index left as it was.
    pub skipped: Vec<Skipped>,
    /// Files whose frontmatter could not be read for an identity; they were
    /// not considered to hold any tweet.
    pub unreadable: Vec<PathBuf>,
    /// Keys whose index row could not be pointed at the tweet's file (the
    /// persist step fills this in); the index was left as it was for them.
    pub index_slug_conflicts: Vec<String>,
    /// One message per tweet work or tweet highlight whose id the index
    /// already holds for a different key or work (the persist step fills
    /// this in); those index rows were left as they were.
    pub index_ownership_conflicts: Vec<String>,
}

impl TweetReport {
    /// A sentence for the import log; None when there is nothing to say.
    pub fn log_message(&self) -> Option<String> {
        let mut parts = Vec::new();
        if !self.duplicate_keys.is_empty() {
            parts.push(format!(
                "{} tweets have more than one file in the archive (no new file made): {}",
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
                "{} tweets were not written, and the archive and the index left as they are for them: {}",
                list.len(),
                list.join(", ")
            ));
        }
        if !self.unreadable.is_empty() {
            let list: Vec<String> = self
                .unreadable
                .iter()
                .map(|p| p.display().to_string())
                .collect();
            parts.push(format!(
                "{} files have unreadable frontmatter and were not looked at: {}",
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
        if !self.index_ownership_conflicts.is_empty() {
            parts.push(format!(
                "{} index entries belong to something else and were left as they are: {}",
                self.index_ownership_conflicts.len(),
                self.index_ownership_conflicts.join(", ")
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
    /// Ids of tweet works for which nothing was written: the index must not
    /// be given the batch's data for these, work or highlight.
    pub skipped_works: HashSet<String>,
}

/// The source system tweets come from.
const TWEET_SYSTEM: &str = "x";

/// A work from a tweet source, whether or not it carries a tweet id.
fn from_tweet_source(work: &Work) -> bool {
    work.source_system == TWEET_SYSTEM
}

pub fn is_tweet(work: &Work) -> bool {
    from_tweet_source(work) && work.source_id.as_deref().is_some_and(|i| !i.is_empty())
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
/// into the file that holds its key; its `slug`, and its `title`, `author`
/// and `url` where the file gives them, are set to that file's, so the index
/// describes the file that exists.
pub fn write_archive(
    archive_path: &str,
    works: &mut [Work],
    highlights_by_work: &HashMap<String, Vec<&Highlight>>,
) -> Result<Written> {
    let mut out = Written::default();
    if !works.iter().any(from_tweet_source) {
        out.summary = archive_meta::write_archive(archive_path, works, highlights_by_work)?;
        return Ok(out);
    }

    // The generic writer renders by slug over whatever is there; nothing
    // from a tweet source goes to it, with a tweet id or without.
    let others: Vec<Work> = works
        .iter()
        .filter(|w| !from_tweet_source(w))
        .cloned()
        .collect();
    out.summary = archive_meta::write_archive(archive_path, &others, highlights_by_work)?;
    for work in works
        .iter()
        .filter(|w| from_tweet_source(w) && !is_tweet(w))
    {
        out.tweets.skipped.push(Skipped {
            key: format!("{} (work {})", key(work), work.id),
            reason: "tweet without an id".to_string(),
        });
        out.skipped_works.insert(work.id.clone());
    }

    let dir = Path::new(archive_path).join("readings").join("works");
    let scan = tweet_frontmatter::scan(&dir, TWEET_SYSTEM);
    let mut by_key = scan.by_key;
    out.tweets.unreadable = scan.unreadable;
    for work in works.iter_mut().filter(|w| is_tweet(w)) {
        let key = key(work);
        let incoming = highlights_by_work
            .get(&work.id)
            .and_then(|hs| fold_batch(hs));
        let files = by_key.get(&key).cloned().unwrap_or_default();
        let outcome = if incoming.as_ref().is_some_and(|h| is_error_page(&h.text)) {
            Err("the incoming text is an error page, not the tweet".to_string())
        } else if files.is_empty() {
            // A file made here holds the key for the rest of the batch.
            create(&dir, work, incoming.as_ref()).map(|path| {
                by_key.entry(key.clone()).or_default().push(path);
                out.tweets.created += 1;
                incoming
            })
        } else if let Some(incoming) = incoming {
            merge(&files, work, incoming, &mut out.tweets).map(Some)
        } else {
            Err(
                "the import gave no highlight for this tweet; its file was left as it is"
                    .to_string(),
            )
        };
        match outcome {
            Ok(Some(h)) => {
                out.highlights.insert(h.id.clone(), h);
            }
            Ok(None) => {}
            Err(reason) => {
                out.tweets.skipped.push(Skipped { key, reason });
                out.skipped_works.insert(work.id.clone());
            }
        }
    }
    out.summary.written += out.tweets.created + out.tweets.updated;
    out.summary.unchanged += out.tweets.unchanged;
    Ok(out)
}

/// How many names `create` tries (`<slug>`, `<slug>-2`, …) before giving up.
const NAME_ATTEMPTS: u32 = 1000;

/// `<slug>.md` when that is one plain path component, so a file of that name
/// can only be inside the works directory.
fn file_name(slug: &str) -> Option<String> {
    let unsafe_slug = slug.is_empty()
        || slug.starts_with('.')
        || slug.contains("..")
        || slug.contains(['/', '\\', '\0']);
    let name = format!("{slug}.md");
    let mut parts = Path::new(&name).components();
    let single = matches!(
        (parts.next(), parts.next()),
        (Some(Component::Normal(part)), None) if part == name.as_str()
    );
    (!unsafe_slug && single).then_some(name)
}

/// Render a tweet no file holds the key of, under the first of `<slug>`,
/// `<slug>-2`, … that nothing has at the moment of publication. Nothing that
/// exists is ever written over. Returns the new file; an error is the reason
/// nothing was created.
fn create(dir: &Path, work: &mut Work, incoming: Option<&Highlight>) -> Result<PathBuf, String> {
    let base = work.slug.clone();
    if file_name(&base).is_none() {
        return Err(format!("its file name {base:?} is not a plain file name"));
    }
    let highlights: Vec<&Highlight> = incoming.into_iter().collect();
    let content = archive_meta::render_new_work_file(work, &highlights);
    for n in 1..=NAME_ATTEMPTS {
        let slug = if n == 1 {
            base.clone()
        } else {
            format!("{base}-{n}")
        };
        let name = file_name(&slug).ok_or("its file name is not a plain file name")?;
        let path = dir.join(name);
        if path.parent() != Some(dir) {
            return Err("its file would be outside the works directory".to_string());
        }
        match archive_meta::create_exclusive(&path, content.as_bytes()) {
            Ok(true) => {
                work.slug = slug;
                return Ok(path);
            }
            Ok(false) => continue,
            Err(e) => return Err(format!("its file could not be created: {e}")),
        }
    }
    Err(format!(
        "{NAME_ATTEMPTS} file names starting {base:?} are taken"
    ))
}

/// One highlight for a tweet from the batch's highlights for it (a tweet
/// both liked and bookmarked arrives twice): the first whose text is not an
/// error page (else the first), its text continued by the same rule as a
/// file's, first date, tags unioned.
fn fold_batch(highlights: &[&Highlight]) -> Option<Highlight> {
    let base = highlights
        .iter()
        .position(|h| !is_error_page(&h.text))
        .unwrap_or(0);
    let mut out: Highlight = (*highlights.get(base)?).clone();
    for (i, h) in highlights.iter().enumerate() {
        if i == base {
            continue;
        }
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

/// The text before the explicit mark of a cut-off tweet: comparable text
/// that ends with `…` or `...`, optionally followed by one t.co link. None
/// for text without the mark; a trailing link alone marks nothing.
fn cut_off_stem(comparable_text: &str) -> Option<&str> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re =
        RE.get_or_init(|| Regex::new(r"^(.*?)\s*(?:…|\.\.\.)(?:\s*https?://t\.co/\w+)?$").unwrap());
    re.captures(comparable_text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str())
}

/// `whole` continues the cut-off `cut`: `cut` carries the mark, what stands
/// before it is a prefix of `whole` (less `whole`'s own mark, if it is cut
/// off too), and after that prefix `whole` has at least one character that
/// is neither whitespace nor part of a link. A different link after the
/// same words is not a continuation.
fn continues(cut: &str, whole: &str) -> bool {
    static URL: OnceLock<Regex> = OnceLock::new();
    let Some(stem) = cut_off_stem(cut) else {
        return false;
    };
    let whole = cut_off_stem(whole).unwrap_or(whole);
    if stem.is_empty() || !whole.starts_with(stem) {
        return false;
    }
    let links: Vec<std::ops::Range<usize>> = URL
        .get_or_init(|| Regex::new(r"https?://\S+").unwrap())
        .find_iter(whole)
        .map(|m| m.range())
        .collect();
    whole[stem.len()..]
        .char_indices()
        .any(|(i, c)| !c.is_whitespace() && !links.iter().any(|l| l.contains(&(stem.len() + i))))
}

/// Whether two bodies are the same tweet by text: their own texts, made
/// comparable, are equal, or one is a strict prefix of the other, or one is
/// cut off and the other continues it. A body with no text of its own (an
/// image or a link alone) is related only to an identical body.
fn related(a_body: &str, b_body: &str) -> bool {
    let a = comparable(split_context(a_body).0);
    let b = comparable(split_context(b_body).0);
    if a.is_empty() || b.is_empty() {
        return normalise(a_body) == normalise(b_body);
    }
    a == b || a.starts_with(&b) || b.starts_with(&a) || continues(&a, &b) || continues(&b, &a)
}

/// The body to put in place of `existing`, or None to keep it. It is
/// replaced only when its own text is cut off (`cut_off_stem`) and the
/// incoming own text continues it with text of its own (`continues`). Context the existing body carries after
/// the tweet's text (a parent or quoted tweet, links, images) is kept as it
/// is; incoming context is not added.
pub fn replacement(existing: &str, incoming: &str) -> Option<String> {
    let (existing_own, context) = split_context(existing);
    let incoming_own = split_context(incoming).0;
    if !continues(&comparable(existing_own), &comparable(incoming_own)) {
        return None;
    }
    Some(if context.is_empty() {
        incoming_own.to_string()
    } else {
        format!("{incoming_own}\n\n{context}")
    })
}

/// A tweet file in which the tweet's block has been identified.
struct Candidate {
    path: PathBuf,
    /// The file's bytes as loaded; a write is abandoned if the file no
    /// longer holds exactly these.
    original: Vec<u8>,
    head: String,
    blocks: Vec<String>,
    index: usize,
    block: Block,
    frontmatter: Frontmatter,
}

impl Candidate {
    /// Open `path` and find the block that is the tweet. An error is the
    /// reason the file must be left alone.
    fn load(path: &Path, key: &str, incoming_body: &str) -> Result<Candidate, &'static str> {
        let original = fs::read(path).map_err(|_| "the file could not be read")?;
        let frontmatter = tweet_frontmatter::read(&original)
            .readable()
            .filter(|fm| fm.key().as_deref() == Some(key))
            .ok_or("the file's frontmatter no longer gives this tweet's key")?;
        let (head, blocks) = std::str::from_utf8(&original)
            .ok()
            .and_then(archive_meta::split_file_records)
            .filter(|(head, _)| !head.is_empty())
            .ok_or("the file is not laid out as the app writes it")?;
        let parsed: Vec<Block> = blocks
            .iter()
            .map(|b| Block::parse(b))
            .collect::<Option<_>>()
            .ok_or("the file holds a highlight the app cannot read back")?;
        let mut matching = parsed
            .iter()
            .enumerate()
            .filter(|(_, b)| related(&b.text(), incoming_body))
            .map(|(i, _)| i);
        let index = match (matching.next(), matching.next()) {
            (Some(i), None) => i,
            (None, _) if parsed.is_empty() => return Err("the file holds no highlight"),
            (None, _) => return Err("none of the file's highlights is this tweet"),
            _ => return Err("more than one of the file's highlights could be this tweet"),
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
}

/// `YYYY-MM-DD` from an incoming date, when it opens with one.
fn day(created_at: Option<&str>) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap());
    let d = created_at?.split('T').next()?.trim();
    re.is_match(d).then(|| d.to_string())
}

/// Merge one tweet into the file that holds its key. Returns the highlight
/// as it now stands in the archive; an error is the reason nothing was
/// written.
fn merge(
    files: &[PathBuf],
    work: &mut Work,
    incoming: Highlight,
    report: &mut TweetReport,
) -> Result<Highlight, String> {
    if files.len() > 1 {
        report.duplicate_keys.push(key(work));
    }

    // Of several files, the one that continues another's cut-off text; the
    // first by name otherwise.
    let mut target: Option<Candidate> = None;
    let mut refusal = None;
    for path in files {
        match Candidate::load(path, &key(work), &incoming.text) {
            Err(reason) => refusal = refusal.or(Some(reason)),
            Ok(c) => {
                let fuller = target
                    .as_ref()
                    .is_none_or(|best| replacement(&best.block.text(), &c.block.text()).is_some());
                if fuller {
                    target = Some(c);
                }
            }
        }
    }
    let Some(mut c) = target else {
        return Err(refusal.unwrap_or("the file could not be read").to_string());
    };

    let before = c.block.clone();
    if let Some(text) = replacement(&c.block.text(), &incoming.text) {
        c.block.set_text(&text);
    }
    let incoming_day = day(incoming.created_at.as_deref());
    let dated = c
        .block
        .get("highlighted_at")
        .is_some_and(|d| !d.trim().is_empty());
    if let (false, Some(day)) = (dated, &incoming_day) {
        c.block.set("highlighted_at", day.clone());
    }
    let mut tags = c.block.tags();
    for t in &incoming.tags {
        if !t.trim().is_empty() && !tags.contains(t) {
            tags.push(t.clone());
        }
    }
    if tags != c.block.tags() {
        tags.retain(|t| !t.is_empty());
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
            return Err("the updated file would not read back as written".to_string());
        }
        // The replacement is complete on disk before the file is looked at
        // again; it goes in only over the bytes that were loaded.
        let published = PreparedReplacement::new(&c.path, content.as_bytes())
            .and_then(|prepared| prepared.publish_if_target_is(&c.original))
            .map_err(|e| format!("the updated file could not be written: {e}"))?;
        if !published {
            return Err("the file changed on disk during the import".to_string());
        }
        report.updated += 1;
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
    Ok(merged)
}

#[cfg(test)]
#[path = "tweet_upsert_tests.rs"]
mod tests;
