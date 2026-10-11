//! Writing tweet works into the archive: one file per tweet, found by its
//! key (`source_system: x` plus `source_id`) whatever the file is called.
//!
//! A tweet's slug is built from its text, so the same tweet imported again
//! with fuller text has a different slug. Writing by slug therefore made a
//! second file. `write_archive` here looks the tweet up by key first and
//! updates the file it finds, in place:
//!
//! - the fuller text wins (`should_replace`);
//! - a date already on the highlight line is kept, a missing one is filled;
//! - tags are unioned, the file's own first;
//! - everything else in the file (a note, further highlights, other
//!   frontmatter) is left exactly as it is;
//! - a merge that changes nothing writes nothing.
//!
//! Only a tweet with no file yet is rendered fresh. A file this module
//! cannot split and re-join byte for byte is never rewritten, and no second
//! file is made for its tweet. When the archive already holds several files
//! for one key, the one with the fuller text is updated and the key is
//! reported.

use anyhow::Result;
use regex::Regex;
use scout_archive::markdown::yaml_escape;
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::archive_meta::{self, WriteSummary};
use crate::import::merge_blocks::{normalise, Block};
use crate::import::tweet_common::is_error_page;
use crate::models::{Highlight, Work};

/// What a write pass did with the batch's tweets.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TweetReport {
    pub created: usize,
    pub updated: usize,
    pub unchanged: usize,
    /// Keys (`x:<tweet id>`) the archive holds more than one file for.
    pub duplicate_keys: Vec<String>,
    /// Keys whose file could not be read back as the app writes it and was
    /// left alone.
    pub skipped_keys: Vec<String>,
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
        if !self.skipped_keys.is_empty() {
            parts.push(format!(
                "{} tweet files could not be read as written by the app and were left alone: {}",
                self.skipped_keys.len(),
                self.skipped_keys.join(", ")
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
}

pub fn is_tweet(work: &Work) -> bool {
    work.source_system == "x" && work.source_id.is_some()
}

/// `archive_meta::write_archive` for a batch that may hold tweets. Works
/// from other sources are rendered as before. Each tweet work is merged
/// into the file that already holds its key, and its `slug`, `title`,
/// `author` and `url` are set to that file's, so the index points at the
/// file that exists.
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
    let by_key = scan(&dir);
    for work in works.iter_mut().filter(|w| is_tweet(w)) {
        let Some(incoming) = highlights_by_work
            .get(&work.id)
            .and_then(|hs| fold_batch(hs))
        else {
            // A tweet work with no highlight in the batch has nothing to
            // merge; it is rendered only when it has no file yet.
            if existing_files(&dir, &by_key, work).is_empty() {
                let s = archive_meta::write_archive(
                    archive_path,
                    std::slice::from_ref(work),
                    highlights_by_work,
                )?;
                out.tweets.created += s.written;
            }
            continue;
        };
        let merged = upsert(archive_path, &dir, &by_key, work, incoming, &mut out.tweets)?;
        out.highlights.insert(merged.id.clone(), merged);
    }
    out.summary.written += out.tweets.created + out.tweets.updated;
    out.summary.unchanged += out.tweets.unchanged;
    Ok(out)
}

fn key(work: &Work) -> String {
    format!(
        "{}:{}",
        work.source_system,
        work.source_id.as_deref().unwrap_or_default()
    )
}

/// The tweet id from a file's frontmatter when it is a tweet work, reading
/// only the first few KB.
fn read_tweet_id(path: &Path) -> Option<String> {
    let mut buf = vec![0u8; 8192];
    let n = fs::File::open(path).ok()?.read(&mut buf).ok()?;
    let head = String::from_utf8_lossy(&buf[..n]);
    let mut lines = head.lines();
    if lines.next() != Some("---") {
        return None;
    }
    let (mut is_x, mut id) = (false, None);
    for line in lines {
        if line == "---" {
            break;
        }
        if let Some(v) = line.strip_prefix("source_system: ") {
            is_x = v.trim() == "x";
        } else if let Some(v) = line.strip_prefix("source_id: ") {
            id = Some(v.trim().trim_matches('"').to_string());
        }
    }
    id.filter(|i| is_x && !i.is_empty())
}

/// Every tweet file in the works directory, by tweet id, paths in name order.
fn scan(dir: &Path) -> HashMap<String, Vec<PathBuf>> {
    let mut by_id: HashMap<String, Vec<PathBuf>> = HashMap::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return by_id;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".md") || archive_meta::is_temp_write_name(&name) {
            continue;
        }
        let path = entry.path();
        if let Some(id) = read_tweet_id(&path) {
            by_id.entry(id).or_default().push(path);
        }
    }
    for paths in by_id.values_mut() {
        paths.sort();
    }
    by_id
}

/// The files already holding this tweet: every file with its key, or else
/// the file at the work's own slug (one whose frontmatter did not give the
/// key must still not be overwritten by a fresh render).
fn existing_files(dir: &Path, by_key: &HashMap<String, Vec<PathBuf>>, work: &Work) -> Vec<PathBuf> {
    let found = work
        .source_id
        .as_deref()
        .and_then(|id| by_key.get(id))
        .cloned()
        .unwrap_or_default();
    if !found.is_empty() {
        return found;
    }
    let own = dir.join(format!("{}.md", work.slug));
    if own.is_file() {
        vec![own]
    } else {
        vec![]
    }
}

/// One highlight for a tweet from the batch's highlights for it (a tweet
/// both liked and bookmarked arrives twice): fuller text, first date, tags
/// unioned.
fn fold_batch(highlights: &[&Highlight]) -> Option<Highlight> {
    let mut it = highlights.iter();
    let mut out: Highlight = (*it.next()?).clone();
    for h in it {
        if should_replace(&out.text, &h.text) {
            out.text = h.text.clone();
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

fn truncation_marker() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // An ellipsis closing a line, with or without the t.co link X appends
    // to a cut-off tweet.
    RE.get_or_init(|| Regex::new(r"(…|\.\.\.)(\s*https?://t\.co/\w+)?\s*$").unwrap())
}

fn tco_link() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"https?://t\.co/\w+").unwrap())
}

/// Where the tweet's own text is cut off, if it is: the byte offset of the
/// ellipsis that closes a line. Quoted context (`> ` lines) is not looked at.
fn truncation_point(text: &str) -> Option<usize> {
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        if !line.starts_with('>') {
            if let Some(m) = truncation_marker().find(line.trim_end_matches('\n')) {
                return Some(offset + m.start());
            }
        }
        offset += line.len();
    }
    None
}

/// How much tweet a text holds: its length in characters up to the cut-off
/// point, whitespace collapsed and t.co links (which stand for nothing a
/// reader can see) left out.
fn substance(text: &str) -> usize {
    let kept = &text[..truncation_point(text).unwrap_or(text.len())];
    normalise(&tco_link().replace_all(kept, "")).chars().count()
}

/// Whether `incoming` should take the place of `existing` as a tweet's text.
/// A page saved in place of the tweet never replaces anything and is itself
/// replaced by any real text; a cut-off text never replaces a whole one;
/// otherwise the text holding more of the tweet wins, and the existing one
/// stays on a tie.
pub fn should_replace(existing: &str, incoming: &str) -> bool {
    let (e, i) = (normalise(existing), normalise(incoming));
    if i.is_empty() || is_error_page(&i) {
        return false;
    }
    if e.is_empty() || is_error_page(&e) {
        return true;
    }
    if e == i {
        return false;
    }
    if truncation_point(incoming).is_some() && truncation_point(existing).is_none() {
        return false;
    }
    substance(incoming) > substance(existing)
}

/// A tweet file opened for merging.
struct Candidate {
    path: PathBuf,
    head: String,
    blocks: Vec<String>,
    /// Which block is the tweet, and that block parsed.
    index: usize,
    block: Block,
}

impl Candidate {
    fn load(path: &Path, incoming_text: &str) -> Option<Candidate> {
        let content = fs::read_to_string(path).ok()?;
        let (head, blocks) = archive_meta::split_file_records(&content)?;
        if head.is_empty() {
            return None;
        }
        let parsed: Vec<(usize, Block)> = blocks
            .iter()
            .enumerate()
            .filter_map(|(i, b)| Block::parse(b).map(|b| (i, b)))
            .collect();
        let (index, block) = tweet_block(parsed, incoming_text)?;
        Some(Candidate {
            path: path.to_path_buf(),
            head,
            blocks,
            index,
            block,
        })
    }

    fn slug(&self) -> String {
        self.path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

/// The block that is the tweet, among a file's quote blocks (the user may
/// have added highlights of their own): the one with the incoming text, else
/// the one that opens the way the incoming text does, else the one carrying
/// a tag the tweet importers set, else the first.
fn tweet_block(parsed: Vec<(usize, Block)>, incoming_text: &str) -> Option<(usize, Block)> {
    let want = normalise(incoming_text);
    let opening = |s: &str| s.chars().take(24).collect::<String>();
    let pick = parsed
        .iter()
        .position(|(_, b)| normalise(&b.text()) == want)
        .or_else(|| {
            parsed.iter().position(|(_, b)| {
                let have = normalise(&b.text());
                !have.is_empty() && (opening(&have) == opening(&want))
            })
        })
        .or_else(|| {
            parsed.iter().position(|(_, b)| {
                b.tags()
                    .iter()
                    .any(|t| matches!(t.as_str(), "like" | "bookmark" | "x"))
            })
        })
        .unwrap_or(0);
    parsed.into_iter().nth(pick)
}

/// The value of a frontmatter line, as written (still YAML-escaped).
fn head_get<'a>(head: &'a str, field: &str) -> Option<&'a str> {
    let prefix = format!("{field}: ");
    head.lines()
        .find_map(|l| l.strip_prefix(prefix.as_str()))
        .map(str::trim)
        .filter(|v| !v.is_empty())
}

/// Set a frontmatter line, replacing it where it stands or adding it after
/// the `after` line (or before the closing `---` when that is missing too).
fn head_set(head: &str, field: &str, value: &str, after: &str) -> String {
    let prefix = format!("{field}: ");
    let line = format!("{prefix}{value}");
    let mut lines: Vec<String> = head.lines().map(String::from).collect();
    if let Some(l) = lines.iter_mut().find(|l| l.starts_with(&prefix)) {
        *l = line;
    } else {
        let after_prefix = format!("{after}: ");
        let at = lines
            .iter()
            .position(|l| l.starts_with(&after_prefix))
            .map(|i| i + 1)
            .unwrap_or(lines.len().saturating_sub(1));
        lines.insert(at, line);
    }
    lines.join("\n") + "\n"
}

/// Undo `yaml_escape`.
fn yaml_unescape(v: &str) -> String {
    match v.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
        Some(inner) if v.len() >= 2 => inner.replace("\\\"", "\""),
        _ => v.to_string(),
    }
}

/// A tweet URL that names no author (`x.com/i/web/status/…`, or the empty
/// handle a Reader document without one produces).
fn anonymous_url(url: &str) -> bool {
    url.contains("/i/web/status/") || url.contains(".com//status/")
}

/// Merge one tweet into the archive and return its highlight as it now
/// stands there. Sets the work's slug, title, author and url to the file's.
fn upsert(
    archive_path: &str,
    dir: &Path,
    by_key: &HashMap<String, Vec<PathBuf>>,
    work: &mut Work,
    incoming: Highlight,
    report: &mut TweetReport,
) -> Result<Highlight> {
    let files = existing_files(dir, by_key, work);
    if files.is_empty() {
        let mut by_work: HashMap<String, Vec<&Highlight>> = HashMap::new();
        by_work.insert(work.id.clone(), vec![&incoming]);
        let s = archive_meta::write_archive(archive_path, std::slice::from_ref(work), &by_work)?;
        report.created += s.written;
        report.unchanged += s.unchanged;
        return Ok(incoming);
    }
    if files.len() > 1 {
        report.duplicate_keys.push(key(work));
    }

    // Of several files, the one whose tweet text is fuller; the first by
    // name on a tie.
    let mut target: Option<Candidate> = None;
    for c in files
        .iter()
        .filter_map(|p| Candidate::load(p, &incoming.text))
    {
        target = match target {
            Some(best) if !should_replace(&best.block.text(), &c.block.text()) => Some(best),
            _ => Some(c),
        };
    }
    let Some(mut c) = target else {
        // No file for this tweet can be rewritten safely. Leave them all and
        // make no new one; the index still points at an existing file.
        report.skipped_keys.push(key(work));
        if let Some(stem) = files[0].file_stem() {
            work.slug = stem.to_string_lossy().into_owned();
        }
        return Ok(incoming);
    };

    let before = c.block.clone();
    let old_text = c.block.text();
    let text_replaced = should_replace(&old_text, &incoming.text);
    if text_replaced {
        c.block.set_text(&incoming.text);
    }
    let incoming_day = incoming
        .created_at
        .as_deref()
        .map(|d| d.split('T').next().unwrap_or(d).trim().to_string())
        .filter(|d| !d.is_empty());
    if c.block.get("highlighted_at").is_none() {
        if let Some(day) = &incoming_day {
            c.block.set("highlighted_at", day.clone());
        }
    }
    // Tags another copy of the same tweet in this file already carries (an
    // older build wrote a like and a bookmark as two blocks) are not added.
    let old_key = normalise(&old_text);
    let elsewhere: Vec<String> = c
        .blocks
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != c.index)
        .filter_map(|(_, b)| Block::parse(b))
        .filter(|b| normalise(&b.text()) == old_key)
        .flat_map(|b| b.tags())
        .collect();
    let mut tags = c.block.tags();
    for t in &incoming.tags {
        if !tags.contains(t) && !elsewhere.contains(t) {
            tags.push(t.clone());
        }
    }
    if tags != c.block.tags() {
        c.block.set("tags", tags.join(", "));
    }

    let mut head = c.head.clone();
    if text_replaced {
        head = head_set(&head, "title", &yaml_escape(&work.title), "");
    } else if let Some(t) = head_get(&head, "title") {
        work.title = yaml_unescape(t);
    }
    match (head_get(&head, "author").map(yaml_unescape), &work.author) {
        (Some(have), _) => work.author = Some(have),
        (None, Some(a)) if !a.trim().is_empty() => {
            head = head_set(&head, "author", &yaml_escape(a), "title");
        }
        _ => {}
    }
    let have_url = head_get(&head, "url").map(String::from);
    match (&have_url, &work.url) {
        (Some(have), Some(new)) if have != new && anonymous_url(have) && !anonymous_url(new) => {
            head = head_set(&head, "url", new, "source_id");
        }
        (None, Some(new)) if !anonymous_url(new) => {
            head = head_set(&head, "url", new, "source_id");
        }
        (Some(have), _) => work.url = Some(have.clone()),
        _ => {}
    }

    let changed = c.block != before || head != c.head;
    if changed {
        head = head_set(&head, "updated_at", &work.updated_at, "imported_at");
        c.blocks[c.index] = c.block.render();
        let content = archive_meta::join_file_records(&head, &c.blocks);
        // The file must still split the way it was written, or a later
        // import could not read it.
        if archive_meta::split_file_records(&content).is_none() {
            report.skipped_keys.push(key(work));
            work.slug = c.slug();
            return Ok(incoming);
        }
        if archive_meta::write_if_changed(&c.path, content.as_bytes())? {
            report.updated += 1;
        } else {
            report.unchanged += 1;
        }
    } else {
        report.unchanged += 1;
    }
    work.slug = c.slug();

    let mut merged = incoming;
    merged.text = c.block.text();
    merged.tags = c.block.tags();
    merged.note = c.block.note.clone().filter(|n| !n.trim().is_empty());
    let file_day = c.block.get("highlighted_at").map(String::from);
    if file_day != incoming_day {
        merged.created_at = file_day;
    }
    Ok(merged)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::tweet_common::{make_records, TweetInput};
    use std::time::{Duration, SystemTime};

    const ID: &str = "1904944923159445914";
    const SHORT: &str = "Voice Chat + Video Chat! Just in Qwen Chat. We open-source the model behind all this, under the… https://t.co/LHQOQrl9Ha";
    const FULL: &str = "Voice Chat + Video Chat! Just in Qwen Chat. We open-source the model behind all this, under the Apache 2.0 licence, and here is the rest of a long post.";

    struct Env {
        dir: PathBuf,
    }

    impl Env {
        fn new(name: &str) -> Env {
            let dir = std::env::temp_dir().join(format!(
                "hs-tweet-upsert-{}-{}",
                name,
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(dir.join("readings/works")).unwrap();
            Env { dir }
        }
        fn works(&self) -> PathBuf {
            self.dir.join("readings/works")
        }
        fn names(&self) -> Vec<String> {
            let mut n: Vec<String> = fs::read_dir(self.works())
                .unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            n.sort();
            n
        }
        fn only_file(&self) -> PathBuf {
            let n = self.names();
            assert_eq!(n.len(), 1, "{n:?}");
            self.works().join(&n[0])
        }
        fn import(&self, t: &TweetInput, now: &str) -> (Work, Written) {
            let (work, h, _) = make_records(t, now);
            let mut works = vec![work];
            let mut by: HashMap<String, Vec<&Highlight>> = HashMap::new();
            by.insert(works[0].id.clone(), vec![&h]);
            let w = write_archive(self.dir.to_str().unwrap(), &mut works, &by).unwrap();
            (works.remove(0), w)
        }
    }

    impl Drop for Env {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    fn tweet(text: &str, saved_as: &str) -> TweetInput {
        TweetInput {
            tweet_id: ID.into(),
            text: text.into(),
            author_handle: Some("Alibaba_Qwen".into()),
            author_name: Some("Qwen".into()),
            created_at: Some("2025-03-26T10:00:00.000Z".into()),
            url: Some(format!("https://x.com/Alibaba_Qwen/status/{ID}")),
            saved_as: Some(saved_as.into()),
            ..Default::default()
        }
    }

    /// Push the file's mtime into the past so an unwanted rewrite shows.
    fn age(path: &Path) -> SystemTime {
        let old = SystemTime::now() - Duration::from_secs(3600);
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(old)
            .unwrap();
        fs::metadata(path).unwrap().modified().unwrap()
    }

    fn mtime(path: &Path) -> SystemTime {
        fs::metadata(path).unwrap().modified().unwrap()
    }

    #[test]
    fn the_same_tweet_twice_is_one_file_and_the_second_run_writes_nothing() {
        let env = Env::new("same");
        let (_, first) = env.import(&tweet(FULL, "likes"), "2026-06-20T00:00:00+00:00");
        assert_eq!(first.tweets.created, 1);
        let file = env.only_file();
        let bytes = fs::read(&file).unwrap();
        let before = age(&file);

        let (_, again) = env.import(&tweet(FULL, "likes"), "2026-06-21T00:00:00+00:00");
        assert_eq!(
            (
                again.tweets.created,
                again.tweets.updated,
                again.tweets.unchanged
            ),
            (0, 0, 1)
        );
        assert_eq!(fs::read(env.only_file()).unwrap(), bytes);
        assert_eq!(mtime(&file), before);
    }

    #[test]
    fn fuller_text_updates_the_same_file_under_its_old_name() {
        let env = Env::new("fuller");
        env.import(&tweet(SHORT, "likes"), "2026-06-20T00:00:00+00:00");
        let name = env.names();

        let (work, w) = env.import(&tweet(FULL, "likes"), "2026-06-21T00:00:00+00:00");
        assert_eq!((w.tweets.created, w.tweets.updated), (0, 1));
        assert_eq!(env.names(), name, "old filename kept, no second file");
        assert_eq!(
            format!("{}.md", work.slug),
            name[0],
            "the index gets the file's slug"
        );
        let text = fs::read_to_string(env.only_file()).unwrap();
        assert!(text.contains("Apache 2.0 licence"), "{text}");
        assert!(!text.contains("t.co/LHQOQrl9Ha"), "{text}");
        assert!(
            text.contains("imported_at: 2026-06-20T00:00:00+00:00\n"),
            "{text}"
        );
        assert!(
            text.contains("updated_at: 2026-06-21T00:00:00+00:00\n"),
            "{text}"
        );
        assert!(
            text.contains("highlighted_at: 2025-03-26 | tags: like\n"),
            "{text}"
        );
        assert_eq!(w.highlights[&format!("x-{ID}")].text, FULL);
    }

    #[test]
    fn shorter_text_and_error_pages_change_nothing() {
        let env = Env::new("shorter");
        env.import(&tweet(FULL, "likes"), "2026-06-20T00:00:00+00:00");
        let file = env.only_file();
        let bytes = fs::read(&file).unwrap();
        let before = age(&file);
        for worse in [
            SHORT,
            "Voice Chat + Video Chat!",
            "JavaScript is not available.\n\nWe’ve detected that JavaScript is disabled in this browser. Please enable JavaScript or switch to a supported browser to continue using twitter.com. You can see a list of supported browsers in our Help Center.\n\nHelp Center",
        ] {
            let (work, w) = env.import(&tweet(worse, "likes"), "2026-06-21T00:00:00+00:00");
            assert_eq!(w.tweets.unchanged, 1, "{worse}");
            assert_eq!(fs::read(env.only_file()).unwrap(), bytes, "{worse}");
            assert_eq!(mtime(&file), before, "{worse}");
            // The index is given what the file holds, not the worse text.
            assert_eq!(w.highlights[&format!("x-{ID}")].text, FULL);
            assert!(work.title.starts_with("Voice Chat + Video Chat! Just in Qwen"));
        }
    }

    #[test]
    fn an_error_page_in_the_archive_is_replaced_by_the_tweet() {
        let env = Env::new("errorpage");
        let mut junk = tweet(
            "JavaScript is not available.\n\nWe’ve detected that JavaScript is disabled in this browser.",
            "likes",
        );
        junk.author_handle = None;
        junk.url = Some(format!("https://x.com/i/web/status/{ID}"));
        env.import(&junk, "2026-06-20T00:00:00+00:00");
        let name = env.names();
        assert!(name[0].starts_with("unknown-javascript"), "{name:?}");

        env.import(&tweet(FULL, "likes"), "2026-06-21T00:00:00+00:00");
        assert_eq!(env.names(), name);
        let text = fs::read_to_string(env.only_file()).unwrap();
        assert!(text.contains("> Voice Chat + Video Chat!"), "{text}");
        assert!(!text.contains("JavaScript"), "{text}");
        // The author the first import lacked is filled in.
        assert!(text.contains("\nauthor: Alibaba_Qwen\n"), "{text}");
        assert!(
            text.contains(&format!("\nurl: https://x.com/Alibaba_Qwen/status/{ID}\n")),
            "{text}"
        );
    }

    #[test]
    fn a_missing_date_is_filled_and_an_existing_one_is_kept() {
        let env = Env::new("dates");
        let mut undated = tweet(FULL, "likes");
        undated.created_at = None;
        undated.tweet_id = "1234567890".into(); // before the snowflake era
        let (_, w) = env.import(&undated, "2026-06-20T00:00:00+00:00");
        assert_eq!(w.tweets.created, 1, "an undated tweet still imports");
        let text = fs::read_to_string(env.only_file()).unwrap();
        assert!(text.contains("\n\ntags: like\n"), "{text}");

        let mut dated = tweet(FULL, "likes");
        dated.tweet_id = "1234567890".into();
        dated.created_at = Some("2010-05-01T00:00:00Z".into());
        let (_, w) = env.import(&dated, "2026-06-21T00:00:00+00:00");
        assert_eq!(w.tweets.updated, 1);
        let text = fs::read_to_string(env.only_file()).unwrap();
        assert!(
            text.contains("highlighted_at: 2010-05-01 | tags: like\n"),
            "{text}"
        );

        dated.created_at = Some("2011-11-11T00:00:00Z".into());
        let (_, w) = env.import(&dated, "2026-06-22T00:00:00+00:00");
        assert_eq!(w.tweets.unchanged, 1);
        let text = fs::read_to_string(env.only_file()).unwrap();
        assert!(
            text.contains("highlighted_at: 2010-05-01 | tags: like\n"),
            "{text}"
        );
        assert_eq!(
            w.highlights["x-1234567890"].created_at.as_deref(),
            Some("2010-05-01"),
            "the index gets the date the file keeps"
        );
    }

    #[test]
    fn like_then_bookmark_gives_both_tags() {
        let env = Env::new("tags");
        env.import(&tweet(FULL, "likes"), "2026-06-20T00:00:00+00:00");
        let (_, w) = env.import(&tweet(FULL, "bookmarks"), "2026-06-21T00:00:00+00:00");
        assert_eq!(w.tweets.updated, 1);
        let text = fs::read_to_string(env.only_file()).unwrap();
        assert!(
            text.contains("highlighted_at: 2025-03-26 | tags: like, bookmark\n"),
            "{text}"
        );
        assert_eq!(text.matches("> Voice Chat").count(), 1);
        // And once both are there, either import changes nothing.
        let (_, w) = env.import(&tweet(FULL, "likes"), "2026-06-22T00:00:00+00:00");
        assert_eq!(w.tweets.unchanged, 1);
    }

    #[test]
    fn a_like_and_a_bookmark_in_one_batch_are_one_block_with_both_tags() {
        let env = Env::new("batch");
        let (work, like, _) = make_records(&tweet(FULL, "likes"), "2026-06-20T00:00:00+00:00");
        let (_, bookmark, _) = make_records(&tweet(FULL, "bookmarks"), "2026-06-20T00:00:00+00:00");
        let mut works = vec![work];
        let mut by: HashMap<String, Vec<&Highlight>> = HashMap::new();
        by.insert(works[0].id.clone(), vec![&like, &bookmark]);
        write_archive(env.dir.to_str().unwrap(), &mut works, &by).unwrap();
        let text = fs::read_to_string(env.only_file()).unwrap();
        assert_eq!(text.matches("> Voice Chat").count(), 1, "{text}");
        assert!(text.contains("tags: like, bookmark\n"), "{text}");
    }

    #[test]
    fn a_note_an_extra_tag_and_an_extra_highlight_survive_reimport() {
        let env = Env::new("user");
        env.import(&tweet(SHORT, "likes"), "2026-06-20T00:00:00+00:00");
        let file = env.only_file();
        let edited = fs::read_to_string(&file)
            .unwrap()
            .replace(
                "highlighted_at: 2025-03-26 | tags: like\n",
                "highlighted_at: 2025-03-26 | tags: like, voice-models\n\nMy note on this.\n\n---\n\n> a line I pulled out myself\n\ntags: mine\n",
            );
        fs::write(&file, &edited).unwrap();

        // Same text: nothing to do, the edits stay byte for byte.
        let before = age(&file);
        let (_, w) = env.import(&tweet(SHORT, "likes"), "2026-06-21T00:00:00+00:00");
        assert_eq!(w.tweets.unchanged, 1);
        assert_eq!(fs::read_to_string(&file).unwrap(), edited);
        assert_eq!(mtime(&file), before);
        let merged = &w.highlights[&format!("x-{ID}")];
        assert_eq!(merged.note.as_deref(), Some("My note on this."));
        assert_eq!(merged.tags, ["like", "voice-models"]);

        // Fuller text and a new tag: the tweet block changes, the rest stays.
        let (_, w) = env.import(&tweet(FULL, "bookmarks"), "2026-06-22T00:00:00+00:00");
        assert_eq!(w.tweets.updated, 1);
        let text = fs::read_to_string(env.only_file()).unwrap();
        assert!(text.contains("Apache 2.0 licence"), "{text}");
        assert!(
            text.contains(
                "highlighted_at: 2025-03-26 | tags: like, voice-models, bookmark\n\nMy note on this.\n\n---\n\n> a line I pulled out myself\n\ntags: mine\n\n---\n"
            ),
            "{text}"
        );
    }

    #[test]
    fn two_files_for_one_key_get_no_third_and_the_fuller_is_updated() {
        let env = Env::new("pair");
        // The pair an import on 20 June and another on 21 June left behind.
        let (short_work, short_h, _) =
            make_records(&tweet(SHORT, "likes"), "2026-06-20T00:00:00+00:00");
        let (mut full_work, full_h, _) =
            make_records(&tweet(FULL, "likes"), "2026-06-21T00:00:00+00:00");
        full_work.slug = format!("zz-second-import-{ID}");
        for (w, h) in [(&short_work, &short_h), (&full_work, &full_h)] {
            let mut by: HashMap<String, Vec<&Highlight>> = HashMap::new();
            by.insert(w.id.clone(), vec![h]);
            archive_meta::write_archive(env.dir.to_str().unwrap(), std::slice::from_ref(w), &by)
                .unwrap();
        }
        let names = env.names();
        assert_eq!(names.len(), 2);
        let short_bytes = fs::read(env.works().join(format!("{}.md", short_work.slug))).unwrap();

        let (work, w) = env.import(&tweet(FULL, "bookmarks"), "2026-06-22T00:00:00+00:00");
        assert_eq!(env.names(), names, "no third file");
        assert_eq!(w.tweets.duplicate_keys, [format!("x:{ID}")]);
        assert!(w.tweets.log_message().unwrap().contains(&format!("x:{ID}")));
        assert_eq!(
            work.slug, full_work.slug,
            "the fuller file is the one updated"
        );
        let full = fs::read_to_string(env.works().join(format!("{}.md", full_work.slug))).unwrap();
        assert!(full.contains("tags: like, bookmark\n"), "{full}");
        assert_eq!(
            fs::read(env.works().join(format!("{}.md", short_work.slug))).unwrap(),
            short_bytes,
            "the other file is left alone"
        );
    }

    #[test]
    fn a_file_that_cannot_be_read_back_is_left_alone_and_gets_no_twin() {
        let env = Env::new("unreadable");
        env.import(&tweet(SHORT, "likes"), "2026-06-20T00:00:00+00:00");
        let file = env.only_file();
        let mut odd = fs::read_to_string(&file).unwrap();
        odd.push_str("hand-written text after the last rule");
        fs::write(&file, &odd).unwrap();

        let (_, w) = env.import(&tweet(FULL, "likes"), "2026-06-21T00:00:00+00:00");
        assert_eq!(w.tweets.skipped_keys, [format!("x:{ID}")]);
        assert_eq!(fs::read_to_string(env.only_file()).unwrap(), odd);
    }

    #[test]
    fn works_from_other_sources_are_written_as_before() {
        let env = Env::new("mixed");
        let (tweet_work, tweet_h, _) =
            make_records(&tweet(FULL, "likes"), "2026-06-20T00:00:00+00:00");
        let mut book = tweet_work.clone();
        book.id = "kindle-1".into();
        book.slug = "a-book-1".into();
        book.source_system = "kindle".into();
        book.kind = "book".into();
        let mut book_h = tweet_h.clone();
        book_h.id = "kindle-h1".into();
        book_h.container_id = book.id.clone();
        let mut works = vec![book, tweet_work];
        let mut by: HashMap<String, Vec<&Highlight>> = HashMap::new();
        by.insert("kindle-1".into(), vec![&book_h]);
        by.insert(works[1].id.clone(), vec![&tweet_h]);
        let w = write_archive(env.dir.to_str().unwrap(), &mut works, &by).unwrap();
        assert_eq!(w.summary.written, 2);
        assert_eq!(env.names().len(), 2);
        assert!(env.names().contains(&"a-book-1.md".to_string()));
    }

    #[test]
    fn the_fuller_text_rule() {
        // Cut-off → whole, even when the whole is barely longer than the cut.
        assert!(should_replace("abc def… https://t.co/x1", "abc defg"));
        assert!(should_replace(SHORT, FULL));
        assert!(!should_replace(FULL, SHORT));
        // A cut-off text followed by link lines is still cut off.
        assert!(should_replace(
            &format!("{SHORT}\n\n🔗 https://chat.qwen.ai/a/very/long/link/that/adds/length/to/the/body/and/more/and/more"),
            FULL
        ));
        // Whole texts: longer wins, shorter and equal do not.
        assert!(should_replace(
            "my comment",
            "my comment\n\n— Quoting @carol:\n> the original"
        ));
        assert!(!should_replace("my comment and more", "my comment"));
        assert!(!should_replace("same  text", "same text"));
        // A trailing t.co link adds nothing.
        assert!(!should_replace("Great pic", "Great pic https://t.co/abc"));
        assert!(should_replace(
            "Great pic https://t.co/abc",
            "Great pic\n\n![image](https://pbs.twimg.com/media/A.jpg)"
        ));
        // Nothing and error pages never win.
        assert!(!should_replace("a", ""));
        assert!(!should_replace("a", "Something went wrong. Try reloading."));
        assert!(should_replace("Something went wrong. Try reloading.", "a"));
    }
}
