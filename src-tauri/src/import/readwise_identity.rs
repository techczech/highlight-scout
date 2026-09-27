//! One Readwise book, one work file.
//!
//! Older versions named Readwise work files with the book id in the
//! `rw_book_N` form (`…-rw-book-7034290.md`, `source_id: "rw_book_7034290"`);
//! the export API path names them with the bare number (`…-7034290.md`,
//! `source_id: "7034290"`). Both are the same book (work id `rw_book_N`).
//! `resolve_slugs` makes every write land on the file that already exists,
//! whichever form it has; `merge_duplicate_readwise_works` folds pairs left
//! by older versions into one file. The merge runs only when asked.

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::Serialize;

use crate::archive_meta::{join_file_records, split_file_records, write_if_changed};
use crate::models::Work;

/// The bare Readwise book number from `rw_book_N`, `N` or `"N"`.
pub fn book_number(s: &str) -> Option<String> {
    let s = s.trim().trim_matches('"');
    let n = s.strip_prefix("rw_book_").unwrap_or(s);
    (!n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())).then(|| n.to_string())
}

/// A Readwise work file found on disk.
#[derive(Debug, Clone)]
struct WorkFile {
    slug: String,
    imported_at: Option<String>,
}

fn works_dir(archive_path: &str) -> PathBuf {
    Path::new(archive_path).join("readings").join("works")
}

/// `source_system`, `source_id` and `imported_at` from a file's frontmatter,
/// reading only its first few KB.
fn read_identity(path: &Path) -> Option<(String, Option<String>)> {
    let mut buf = vec![0u8; 8192];
    let n = fs::File::open(path).ok()?.read(&mut buf).ok()?;
    let head = String::from_utf8_lossy(&buf[..n]);
    let mut lines = head.lines();
    if lines.next() != Some("---") {
        return None;
    }
    let (mut system, mut source_id, mut imported) = (None, None, None);
    for line in lines {
        if line == "---" {
            break;
        }
        if let Some(v) = line.strip_prefix("source_system: ") {
            system = Some(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("source_id: ") {
            source_id = Some(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("imported_at: ") {
            imported = Some(v.trim().to_string()).filter(|v| !v.is_empty());
        }
    }
    if system.as_deref() != Some("readwise") {
        return None;
    }
    Some((book_number(&source_id?)?, imported))
}

/// Every Readwise work file on disk, grouped by book number.
fn scan(archive_path: &str) -> HashMap<String, Vec<WorkFile>> {
    let mut by_book: HashMap<String, Vec<WorkFile>> = HashMap::new();
    let Ok(entries) = fs::read_dir(works_dir(archive_path)) else {
        return by_book;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(slug) = name.strip_suffix(".md") else {
            continue;
        };
        if crate::archive_meta::is_temp_write_name(&name) {
            continue;
        }
        if let Some((num, imported_at)) = read_identity(&entry.path()) {
            by_book.entry(num).or_default().push(WorkFile {
                slug: slug.to_string(),
                imported_at,
            });
        }
    }
    for files in by_book.values_mut() {
        sort_oldest_first(files);
    }
    by_book
}

/// Oldest first: earliest imported_at (a file without one sorts last), then
/// the `rw-book` form (the older naming), then name.
fn sort_oldest_first(files: &mut [WorkFile]) {
    files.sort_by(|a, b| {
        let key = |f: &WorkFile| {
            (
                f.imported_at.is_none(),
                f.imported_at.clone().unwrap_or_default(),
                !f.slug.contains("-rw-book-"),
                f.slug.clone(),
            )
        };
        key(a).cmp(&key(b))
    });
}

/// Point each Readwise work at the file that already exists for its book,
/// in either id form, so a sync never creates a second name:
/// 1. the index's slug for the work id, when that file exists;
/// 2. else the work's own slug, when that file exists;
/// 3. else the oldest file on disk for the same book number (a scan of the
///    work files, done at most once per call and only when needed).
///
/// A slug the index already gives to a different work id is never taken
/// (the index's slugs are unique). A work with no existing file keeps its
/// own slug.
pub fn resolve_slugs(conn: &Connection, archive_path: &str, works: &mut [Work]) {
    let dir = works_dir(archive_path);
    let exists = |slug: &str| dir.join(format!("{slug}.md")).is_file();
    let mut scanned: Option<HashMap<String, Vec<WorkFile>>> = None;
    for w in works.iter_mut() {
        if w.source_system != "readwise" {
            continue;
        }
        let indexed: Option<String> = conn
            .query_row("SELECT slug FROM works WHERE id = ?1", [&w.id], |r| {
                r.get(0)
            })
            .ok();
        if let Some(slug) = indexed.filter(|s| exists(s)) {
            w.slug = slug;
            continue;
        }
        if exists(&w.slug) {
            continue;
        }
        let Some(num) = book_number(&w.id).or_else(|| w.source_id.as_deref().and_then(book_number))
        else {
            continue;
        };
        let map = scanned.get_or_insert_with(|| scan(archive_path));
        let Some(oldest) = map.get(&num).and_then(|f| f.first()) else {
            continue;
        };
        let taken_by_other: bool = conn
            .query_row(
                "SELECT 1 FROM works WHERE slug = ?1 AND id != ?2",
                [&oldest.slug, &w.id],
                |_| Ok(true),
            )
            .unwrap_or(false);
        if !taken_by_other {
            w.slug = oldest.slug.clone();
        }
    }
}

/// One book's files folded (or, in a dry run, to be folded) into one.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct MergedBook {
    pub book: String,
    /// The file kept (the oldest), by slug.
    pub keep: String,
    pub keep_blocks: usize,
    /// The newer duplicate files, by slug, with their block counts.
    pub remove: Vec<(String, usize)>,
    /// Blocks in the merged file: the union, identical blocks once and each
    /// folded pair as one.
    pub merged_blocks: usize,
    /// Newer-file blocks folded into a kept block as the same highlight
    /// (same quote text after whitespace normalisation, same or missing
    /// `highlighted_at`): tags unioned, notes combined, missing metadata
    /// filled. A dry run and a real run fold exactly the same blocks.
    pub folded_blocks: usize,
    /// The first few folds (up to 5), as the merged block's first 160 chars.
    pub folded_samples: Vec<String>,
    /// Newer-file blocks equal to a kept block after whitespace
    /// normalisation but not byte-identical. Kept (nothing is dropped), so
    /// they show as near-duplicates in the merged file: inspect first.
    pub duplicate_blocks: usize,
    /// Newer-file blocks whose normalised text is contained in a kept block
    /// (a fragment of a note or record). Also kept; inspect first.
    pub note_fragment_matches: usize,
    /// The first few such blocks (up to 5, first 120 chars), for inspection.
    pub flagged: Vec<String>,
    /// Set when the pair was left alone (a file the splitter cannot
    /// reproduce byte for byte); nothing is written for it.
    pub skipped: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct MergeReport {
    pub dry_run: bool,
    pub books: Vec<MergedBook>,
}

/// Fold each group of work files for the same Readwise book into its oldest
/// file: the union of their blocks (the oldest file's blocks in order, then
/// each newer file's blocks not already present byte for byte), the oldest
/// file's name and frontmatter (so its imported_at) kept, the newer files
/// removed, and the index's `works.slug` pointed at the kept file. With
/// `dry_run` nothing is written; the report lists the groups and counts.
pub fn merge_duplicate_readwise_works(
    archive_path: &str,
    conn: &Connection,
    dry_run: bool,
) -> anyhow::Result<MergeReport> {
    let dir = works_dir(archive_path);
    let mut groups: Vec<(String, Vec<WorkFile>)> = scan(archive_path)
        .into_iter()
        .filter(|(_, f)| f.len() > 1)
        .collect();
    groups.sort_by(|a, b| a.0.cmp(&b.0));

    let mut report = MergeReport {
        dry_run,
        books: vec![],
    };
    for (book, files) in groups {
        let path = |slug: &str| dir.join(format!("{slug}.md"));
        let keep = &files[0];
        let parsed: Vec<Option<(String, Vec<String>)>> = files
            .iter()
            .map(|f| {
                fs::read_to_string(path(&f.slug))
                    .ok()
                    .and_then(|t| split_file_records(&t))
            })
            .collect();
        let counts: Vec<usize> = parsed
            .iter()
            .map(|p| p.as_ref().map_or(0, |(_, b)| b.len()))
            .collect();
        let mut entry = MergedBook {
            book: book.clone(),
            keep: keep.slug.clone(),
            keep_blocks: counts[0],
            remove: files[1..]
                .iter()
                .zip(&counts[1..])
                .map(|(f, n)| (f.slug.clone(), *n))
                .collect(),
            merged_blocks: 0,
            folded_blocks: 0,
            folded_samples: vec![],
            duplicate_blocks: 0,
            note_fragment_matches: 0,
            flagged: vec![],
            skipped: None,
        };
        if parsed.iter().any(Option::is_none) {
            entry.skipped = Some("a file's layout could not be reproduced; left as is".into());
            report.books.push(entry);
            continue;
        }
        let parsed: Vec<(String, Vec<String>)> = parsed.into_iter().flatten().collect();
        let (head, mut blocks) = parsed[0].clone();
        for (_, newer) in &parsed[1..] {
            for b in newer {
                if blocks.contains(b) {
                    continue;
                }
                let fold = blocks
                    .iter()
                    .enumerate()
                    .find_map(|(i, k)| merge_blocks::fold(k, b).map(|m| (i, m)));
                if let Some((i, merged)) = fold {
                    blocks[i] = merged;
                    entry.folded_blocks += 1;
                    if entry.folded_samples.len() < 5 {
                        entry
                            .folded_samples
                            .push(blocks[i].chars().take(160).collect());
                    }
                    continue;
                }
                let nb = normalise(b);
                let kind = if blocks.iter().any(|k| normalise(k) == nb) {
                    Some(&mut entry.duplicate_blocks)
                } else if !nb.is_empty() && blocks.iter().any(|k| normalise(k).contains(&nb)) {
                    Some(&mut entry.note_fragment_matches)
                } else {
                    None
                };
                if let Some(n) = kind {
                    *n += 1;
                    if entry.flagged.len() < 5 {
                        entry.flagged.push(b.chars().take(120).collect());
                    }
                }
                blocks.push(b.clone());
            }
        }
        entry.merged_blocks = blocks.len();
        // Union invariant, checked before anything is written: every quote
        // text in any file of the group is in the merged file.
        let kept_quotes: std::collections::HashSet<String> = blocks
            .iter()
            .filter_map(|b| merge_blocks::quote_text(b))
            .collect();
        let lost = parsed
            .iter()
            .flat_map(|(_, bs)| bs.iter().filter_map(|b| merge_blocks::quote_text(b)))
            .filter(|q| !kept_quotes.contains(q))
            .count();
        if lost > 0 {
            entry.skipped = Some(format!("{lost} quotes would be lost; left as is"));
            report.books.push(entry);
            continue;
        }
        if !dry_run {
            write_if_changed(
                &path(&keep.slug),
                join_file_records(&head, &blocks).as_bytes(),
            )?;
            for f in &files[1..] {
                let _ = conn.execute(
                    "UPDATE works SET slug = ?1 WHERE slug = ?2
                     AND NOT EXISTS (SELECT 1 FROM works WHERE slug = ?1)",
                    [&keep.slug, &f.slug],
                );
                move_fulltext(archive_path, &f.slug, &keep.slug);
                fs::remove_file(path(&f.slug))?;
            }
        }
        report.books.push(entry);
    }
    Ok(report)
}

use crate::import::merge_blocks::{self, normalise};

/// The merge under the one-writer claim: refused while an import or sync
/// holds it, and holding it (and the index connection) for the whole merge.
pub fn merge_duplicates_claimed(
    busy: &crate::busy::BusyLock,
    archive_path: &str,
    db: &std::sync::Mutex<Connection>,
    dry_run: bool,
) -> Result<MergeReport, String> {
    let _claim = busy.try_claim(crate::busy::Op::MergeDuplicates)?;
    let conn = db.lock().map_err(|e| e.to_string())?;
    merge_duplicate_readwise_works(archive_path, &conn, dry_run).map_err(|e| e.to_string())
}

/// `--merge-duplicate-readwise-works [--dry-run] [--archive PATH] [--index PATH]`.
/// Refuses (exit 2) while the app holds its lock file; otherwise takes the
/// lock itself for the run. Prints the report as JSON (exit 0), or the error
/// (exit 1). Defaults to the configured archive and index.
pub fn run_cli(args: &[String], lock_path: &Path, is_alive: impl Fn(u32) -> bool) -> i32 {
    let _lock = match crate::app_lock::AppLock::acquire(lock_path, is_alive) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("merge refused: {e}");
            return 2;
        }
    };
    let value = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let dry_run = args.iter().any(|a| a == "--dry-run");
    let archive = value("--archive").unwrap_or_else(|| crate::config::load().archive_path);
    let index = value("--index")
        .map(PathBuf::from)
        .unwrap_or_else(crate::config::index_path);
    let run = || -> anyhow::Result<MergeReport> {
        let conn = scout_index::sqlite::open(&index)?;
        scout_index::sqlite::init_schema(&conn)?;
        merge_duplicate_readwise_works(&archive, &conn, dry_run)
    };
    match run() {
        Ok(report) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&report).unwrap_or_default()
            );
            0
        }
        Err(e) => {
            eprintln!("merge failed: {e}");
            1
        }
    }
}

/// Give the kept slug the newer file's full text when it has none.
fn move_fulltext(archive_path: &str, from: &str, to: &str) {
    let d = Path::new(archive_path).join("readings").join("fulltext");
    let (src, dst) = (d.join(format!("{from}.md")), d.join(format!("{to}.md")));
    if src.is_file() && !dst.exists() {
        let _ = fs::rename(src, dst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Highlight;

    struct Dir(PathBuf);
    impl Dir {
        fn new(name: &str) -> Self {
            let d = std::env::temp_dir().join(format!("hs-rwid-{}-{}", name, std::process::id()));
            let _ = fs::remove_dir_all(&d);
            fs::create_dir_all(d.join("readings/works")).unwrap();
            Dir(d)
        }
        fn path(&self) -> &str {
            self.0.to_str().unwrap()
        }
        fn names(&self) -> Vec<String> {
            let mut v: Vec<String> = fs::read_dir(self.0.join("readings/works"))
                .unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            v.sort();
            v
        }
        fn read(&self, slug: &str) -> String {
            fs::read_to_string(self.0.join(format!("readings/works/{slug}.md"))).unwrap()
        }
    }
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    const OLD: &str = "rajiv-malhotra-being-different-rw-book-7034290";
    const NEW: &str = "rajiv-malhotra-being-different-7034290";

    fn work(slug: &str, source_id: &str, imported_at: &str) -> Work {
        Work {
            id: "rw_book_7034290".into(),
            slug: slug.into(),
            title: "Being Different".into(),
            author: Some("Rajiv Malhotra".into()),
            kind: "book".into(),
            source_system: "readwise".into(),
            source_id: Some(source_id.into()),
            url: None,
            imported_at: imported_at.into(),
            updated_at: imported_at.into(),
            source_data: serde_json::json!({}),
        }
    }

    fn hl(id: &str, text: &str) -> Highlight {
        Highlight {
            id: id.into(),
            container_id: "rw_book_7034290".into(),
            text: text.into(),
            note: None,
            created_at: Some("2026-01-01T00:00:00Z".into()),
            updated_at: None,
            tags: vec![],
            location: Some("1".into()),
            location_type: None,
            annotation_color: None,
            annotation_type: None,
            format: "plain".into(),
            source_data: serde_json::Value::Null,
        }
    }

    fn write_file(dir: &Dir, w: &Work, hs: &[&Highlight]) {
        let mut by: HashMap<String, Vec<&Highlight>> = HashMap::new();
        by.insert(w.id.clone(), hs.to_vec());
        crate::archive_meta::write_archive(dir.path(), std::slice::from_ref(w), &by).unwrap();
    }

    fn index() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        scout_index::sqlite::init_schema(&c).unwrap();
        c
    }

    #[test]
    fn book_numbers_normalise_both_forms() {
        assert_eq!(book_number("rw_book_7034290").as_deref(), Some("7034290"));
        assert_eq!(book_number("\"7034290\"").as_deref(), Some("7034290"));
        assert_eq!(
            book_number("\"rw_book_7034290\"").as_deref(),
            Some("7034290")
        );
        assert_eq!(book_number("zotero-ABC"), None);
    }

    #[test]
    fn an_incoming_numeric_id_lands_on_the_existing_rw_book_file() {
        let dir = Dir::new("num-to-rw");
        write_file(
            &dir,
            &work(OLD, "rw_book_7034290", "2025-01-01T00:00:00+00:00"),
            &[&hl("a", "alpha")],
        );
        let mut incoming = vec![work(NEW, "7034290", "2026-09-27T00:00:00+00:00")];
        resolve_slugs(&index(), dir.path(), &mut incoming);
        assert_eq!(incoming[0].slug, OLD);
    }

    #[test]
    fn an_incoming_rw_book_id_lands_on_the_existing_numeric_file() {
        let dir = Dir::new("rw-to-num");
        write_file(
            &dir,
            &work(NEW, "7034290", "2025-01-01T00:00:00+00:00"),
            &[&hl("a", "alpha")],
        );
        let mut incoming = vec![work(OLD, "rw_book_7034290", "2026-09-27T00:00:00+00:00")];
        resolve_slugs(&index(), dir.path(), &mut incoming);
        assert_eq!(incoming[0].slug, NEW);
    }

    #[test]
    fn the_index_slug_is_used_when_its_file_exists() {
        let dir = Dir::new("indexslug");
        write_file(
            &dir,
            &work(OLD, "rw_book_7034290", "2025-01-01T00:00:00+00:00"),
            &[&hl("a", "alpha")],
        );
        let conn = index();
        scout_index::sqlite::upsert_container(&conn, &work(OLD, "rw_book_7034290", "t")).unwrap();
        let mut incoming = vec![work(NEW, "7034290", "t")];
        resolve_slugs(&conn, dir.path(), &mut incoming);
        assert_eq!(incoming[0].slug, OLD);
    }

    fn duplicate_pair(dir: &Dir) {
        write_file(
            dir,
            &work(OLD, "rw_book_7034290", "2025-01-01T00:00:00+00:00"),
            &[&hl("a", "alpha"), &hl("b", "beta")],
        );
        write_file(
            dir,
            &work(NEW, "7034290", "2026-09-27T00:00:00+00:00"),
            &[&hl("b", "beta"), &hl("c", "gamma")],
        );
    }

    fn quotes(t: &str) -> Vec<&str> {
        t.lines().filter_map(|l| l.strip_prefix("> ")).collect()
    }

    #[test]
    fn a_dry_run_lists_the_pair_and_changes_nothing() {
        let dir = Dir::new("dry");
        duplicate_pair(&dir);
        let (old_before, new_before) = (dir.read(OLD), dir.read(NEW));
        let r = merge_duplicate_readwise_works(dir.path(), &index(), true).unwrap();
        assert_eq!(
            r.books,
            [MergedBook {
                book: "7034290".into(),
                keep: OLD.into(),
                keep_blocks: 2,
                remove: vec![(NEW.into(), 2)],
                merged_blocks: 3,
                folded_blocks: 0,
                folded_samples: vec![],
                duplicate_blocks: 0,
                note_fragment_matches: 0,
                flagged: vec![],
                skipped: None,
            }]
        );
        assert_eq!(dir.read(OLD), old_before);
        assert_eq!(dir.read(NEW), new_before);
    }

    #[test]
    fn the_merge_keeps_the_union_in_the_older_file_and_removes_the_newer() {
        let dir = Dir::new("merge");
        duplicate_pair(&dir);
        let conn = index();
        scout_index::sqlite::upsert_container(&conn, &work(NEW, "7034290", "t")).unwrap();
        let r = merge_duplicate_readwise_works(dir.path(), &conn, false).unwrap();
        assert_eq!(r.books[0].merged_blocks, 3);
        assert_eq!(dir.names(), [format!("{OLD}.md")]);
        let merged = dir.read(OLD);
        assert_eq!(quotes(&merged), ["alpha", "beta", "gamma"]);
        assert!(
            merged.contains("imported_at: 2025-01-01T00:00:00+00:00\n"),
            "{merged}"
        );
        let slug: String = conn
            .query_row(
                "SELECT slug FROM works WHERE id='rw_book_7034290'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(slug, OLD);
        // Nothing left to merge; a second run is a no-op.
        assert!(merge_duplicate_readwise_works(dir.path(), &conn, false)
            .unwrap()
            .books
            .is_empty());
    }

    #[test]
    fn the_dry_run_reports_near_duplicates_and_note_fragments() {
        let dir = Dir::new("neardup");
        let mut noted = hl("n", "noted");
        noted.note = Some("a long thought about being different".into());
        write_file(
            &dir,
            &work(OLD, "rw_book_7034290", "2025-01-01T00:00:00+00:00"),
            &[&hl("a", "alpha  beta"), &noted],
        );
        write_file(
            &dir,
            &work(NEW, "7034290", "2026-09-27T00:00:00+00:00"),
            &[&hl("a", "alpha beta"), &hl("c", "gamma")],
        );
        // A newer block that is only a fragment of a kept one (its metadata
        // line), as a hand edit or an older renderer could leave.
        let new_path = dir.0.join(format!("readings/works/{NEW}.md"));
        let (head, mut blocks) =
            crate::archive_meta::split_file_records(&fs::read_to_string(&new_path).unwrap())
                .unwrap();
        blocks.push("highlighted_at: 2026-01-01\n".into());
        // A non-quote block (never folded) that differs only in whitespace.
        blocks.push("```latex\nx y\n```\n\n".into());
        fs::write(&new_path, join_file_records(&head, &blocks)).unwrap();
        let old_path = dir.0.join(format!("readings/works/{OLD}.md"));
        let (head, mut blocks) =
            crate::archive_meta::split_file_records(&fs::read_to_string(&old_path).unwrap())
                .unwrap();
        blocks.push("```latex\nx  y\n```\n\n".into());
        fs::write(&old_path, join_file_records(&head, &blocks)).unwrap();

        let r = merge_duplicate_readwise_works(dir.path(), &index(), true).unwrap();
        let b = &r.books[0];
        // "alpha  beta" and "alpha beta" are one highlight: folded.
        assert_eq!(b.folded_blocks, 1, "{b:?}");
        assert_eq!(b.duplicate_blocks, 1, "{b:?}");
        assert_eq!(b.note_fragment_matches, 1, "{b:?}");
        assert_eq!(b.flagged.len(), 2);
        // 3 kept + gamma + fragment + latex near-duplicate.
        assert_eq!(b.merged_blocks, 6);
    }

    fn dated(id: &str, text: &str, date: &str, tags: &[&str], note: Option<&str>) -> Highlight {
        Highlight {
            created_at: Some(format!("{date}T10:00:00Z")),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            note: note.map(String::from),
            ..hl(id, text)
        }
    }

    fn pair(dir: &Dir, older: &[&Highlight], newer: &[&Highlight]) {
        write_file(
            dir,
            &work(OLD, "rw_book_7034290", "2025-01-01T00:00:00+00:00"),
            older,
        );
        write_file(
            dir,
            &work(NEW, "7034290", "2026-09-27T00:00:00+00:00"),
            newer,
        );
    }

    fn merged_blocks_of(dir: &Dir) -> Vec<String> {
        crate::archive_meta::split_file_records(&dir.read(OLD))
            .unwrap()
            .1
    }

    // The live-clone case: the same highlight, tags only in the newer file.
    #[test]
    fn the_naep_pair_folds_to_one_block_with_the_tags() {
        let dir = Dir::new("naep");
        let text = "NAEP reading scores fell for the lowest performers.";
        pair(
            &dir,
            &[&dated("1", text, "2025-03-13", &[], None)],
            &[&dated("1", text, "2025-03-13", &["education"], None)],
        );
        let r = merge_duplicate_readwise_works(dir.path(), &index(), false).unwrap();
        assert_eq!(r.books[0].folded_blocks, 1);
        assert_eq!(
            merged_blocks_of(&dir),
            [format!(
                "> {text}\n\nhighlighted_at: 2025-03-13 | tags: education\n"
            )]
        );
    }

    #[test]
    fn the_same_text_on_different_dates_stays_two_blocks() {
        let dir = Dir::new("dates");
        pair(
            &dir,
            &[&dated("1", "repeated line", "2025-03-13", &[], None)],
            &[&dated("2", "repeated line", "2025-06-01", &[], None)],
        );
        let r = merge_duplicate_readwise_works(dir.path(), &index(), false).unwrap();
        assert_eq!(r.books[0].folded_blocks, 0);
        assert_eq!(merged_blocks_of(&dir).len(), 2);
    }

    #[test]
    fn differing_notes_are_both_kept_in_the_folded_block() {
        let dir = Dir::new("notes");
        pair(
            &dir,
            &[&dated("1", "q", "2025-03-13", &[], Some("Older note."))],
            &[&dated("1", "q", "2025-03-13", &[], Some("Newer note."))],
        );
        merge_duplicate_readwise_works(dir.path(), &index(), false).unwrap();
        assert_eq!(
            merged_blocks_of(&dir),
            ["> q\n\nhighlighted_at: 2025-03-13\n\nOlder note.\n\nNewer note.\n"]
        );
    }

    #[test]
    fn the_dry_run_folds_exactly_what_the_real_run_folds() {
        let dir = Dir::new("dryreal");
        pair(
            &dir,
            &[
                &dated("1", "one", "2025-03-13", &[], None),
                &dated("2", "two", "2025-03-13", &["a"], None),
                &dated("3", "three", "2025-03-13", &[], None),
            ],
            &[
                &dated("1", "one", "2025-03-13", &["x"], None),
                &dated("2", "two", "2025-03-13", &["b"], Some("n")),
                &dated("3", "three", "2025-04-01", &[], None),
                &dated("4", "four", "2025-03-13", &[], None),
            ],
        );
        let dry = merge_duplicate_readwise_works(dir.path(), &index(), true).unwrap();
        let real = merge_duplicate_readwise_works(dir.path(), &index(), false).unwrap();
        assert_eq!(dry.books[0].folded_blocks, 2);
        assert_eq!(dry.books[0].folded_blocks, real.books[0].folded_blocks);
        assert_eq!(dry.books[0].folded_samples, real.books[0].folded_samples);
        assert_eq!(dry.books[0].merged_blocks, real.books[0].merged_blocks);
        assert_eq!(merged_blocks_of(&dir).len(), real.books[0].merged_blocks);
    }

    #[test]
    fn every_quote_from_every_file_in_a_group_is_in_the_kept_file() {
        let dir = Dir::new("union");
        let older = [
            dated("1", "alpha", "2025-03-13", &[], None),
            dated("2", "beta", "2025-03-13", &["t"], Some("note")),
        ];
        let newer = [
            dated("2", "beta", "2025-03-13", &["u"], None),
            dated("3", "gamma", "2025-03-14", &[], None),
            dated("1", "alpha", "2025-05-05", &[], None),
        ];
        pair(
            &dir,
            &older.iter().collect::<Vec<_>>(),
            &newer.iter().collect::<Vec<_>>(),
        );
        let quotes_of = |t: &str| -> std::collections::BTreeSet<String> {
            t.lines()
                .filter_map(|l| l.strip_prefix("> "))
                .map(String::from)
                .collect()
        };
        let mut all = quotes_of(&dir.read(OLD));
        all.extend(quotes_of(&dir.read(NEW)));
        merge_duplicate_readwise_works(dir.path(), &index(), false).unwrap();
        assert_eq!(quotes_of(&dir.read(OLD)), all);
        assert_eq!(dir.names(), [format!("{OLD}.md")]);
    }

    #[test]
    fn a_merge_is_refused_while_an_import_holds_the_claim() {
        let dir = Dir::new("claimed");
        duplicate_pair(&dir);
        let before = dir.names();
        let busy = crate::busy::BusyLock::default();
        let db = std::sync::Mutex::new(index());
        let import = busy.try_claim(crate::busy::Op::ReadwiseImport).unwrap();
        let err = merge_duplicates_claimed(&busy, dir.path(), &db, false).unwrap_err();
        assert!(err.contains("a Readwise import is running"), "{err}");
        assert_eq!(dir.names(), before);
        drop(import);
        assert!(merge_duplicates_claimed(&busy, dir.path(), &db, false).is_ok());
        assert_eq!(dir.names(), [format!("{OLD}.md")]);
        // And the claim is released afterwards: an import can start.
        assert!(busy.try_claim(crate::busy::Op::ReadwiseImport).is_ok());
    }

    #[test]
    fn the_cli_merge_is_refused_while_the_app_holds_its_lock() {
        let dir = Dir::new("clilock");
        duplicate_pair(&dir);
        let before = dir.names();
        let lock_path = dir.0.join("app/highlight-scout.lock");
        let index_path = dir.0.join("index.sqlite");
        let args: Vec<String> = [
            "hs",
            "--merge-duplicate-readwise-works",
            "--archive",
            dir.path(),
            "--index",
            index_path.to_str().unwrap(),
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let app = crate::app_lock::AppLock::acquire(&lock_path, |_| true).unwrap();
        assert_eq!(run_cli(&args, &lock_path, |_| true), 2);
        assert_eq!(dir.names(), before);
        drop(app);
        assert_eq!(run_cli(&args, &lock_path, |_| true), 0);
        assert_eq!(dir.names(), [format!("{OLD}.md")]);
        assert!(!lock_path.exists());
    }
}
