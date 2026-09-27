use anyhow::Result;
use scout_archive::markdown::{self, ContainerMeta, RecordMeta};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::models::{Highlight, Work};

/// A Work as it goes to disk: the timestamps may differ from the batch's
/// (imported_at is carried over from the existing file; updated_at only moves
/// when the rendered content changes).
struct ArchiveWork<'a> {
    work: &'a Work,
    imported_at: &'a str,
    updated_at: &'a str,
}
struct ArchiveHighlight<'a>(&'a Highlight);

impl ContainerMeta for ArchiveWork<'_> {
    fn slug(&self) -> &str {
        &self.work.slug
    }

    fn id(&self) -> &str {
        &self.work.id
    }

    fn title(&self) -> &str {
        &self.work.title
    }

    fn author(&self) -> Option<&str> {
        self.work.author.as_deref()
    }

    fn kind(&self) -> &str {
        &self.work.kind
    }

    fn source_system(&self) -> &str {
        &self.work.source_system
    }

    fn source_id(&self) -> Option<&str> {
        self.work.source_id.as_deref()
    }

    fn url(&self) -> Option<&str> {
        self.work.url.as_deref()
    }

    fn imported_at(&self) -> &str {
        self.imported_at
    }

    fn updated_at(&self) -> &str {
        self.updated_at
    }

    fn source_data_json(&self) -> String {
        serde_json::to_string(&self.work.source_data).unwrap_or_else(|_| "{}".into())
    }
}

impl RecordMeta for ArchiveHighlight<'_> {
    fn id(&self) -> &str {
        &self.0.id
    }

    fn text(&self) -> &str {
        &self.0.text
    }

    fn note(&self) -> Option<&str> {
        self.0.note.as_deref()
    }

    fn created_at(&self) -> Option<&str> {
        self.0.created_at.as_deref()
    }

    fn tags(&self) -> &[String] {
        &self.0.tags
    }

    fn annotation_color(&self) -> Option<&str> {
        self.0.annotation_color.as_deref()
    }

    fn annotation_type(&self) -> Option<&str> {
        self.0.annotation_type.as_deref()
    }

    fn format(&self) -> &str {
        &self.0.format
    }
}

/// What a write pass did.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct WriteSummary {
    pub written: usize,
    pub unchanged: usize,
}

/// Write `bytes` to `path` only if the file is missing or differs. Returns
/// true when it wrote. An identical file is left alone (mtime untouched), so
/// a re-sync of unchanged sources does not churn the archive.
///
/// The write is atomic: bytes go to a temp file in the same directory, which
/// is then renamed over `path`. A crash or full disk mid-write leaves the old
/// file intact rather than a truncated one.
pub fn write_if_changed(path: &Path, bytes: &[u8]) -> std::io::Result<bool> {
    if let Ok(existing) = fs::read(path) {
        if existing == bytes {
            return Ok(false);
        }
    }
    write_atomic(path, bytes)?;
    Ok(true)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp = dir.join(format!(".{}.tmp-{}-{}", name, std::process::id(), nanos));
    let result = (|| {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// `scout_archive::markdown::write_fulltext`, but an identical body is not
/// rewritten.
pub fn write_fulltext(archive_path: &str, slug: &str, text: &str) -> Result<bool> {
    let dir = Path::new(archive_path).join("readings").join("fulltext");
    fs::create_dir_all(&dir)?;
    Ok(write_if_changed(
        &dir.join(format!("{}.md", slug)),
        text.as_bytes(),
    )?)
}

/// `imported_at` / `updated_at` from a work file's leading frontmatter block.
fn frontmatter_stamps(content: &str) -> (Option<&str>, Option<&str>) {
    let mut imported = None;
    let mut updated = None;
    let mut lines = content.lines();
    if lines.next() != Some("---") {
        return (None, None);
    }
    for line in lines {
        if line == "---" {
            break;
        }
        if let Some(v) = line.strip_prefix("imported_at: ") {
            imported = Some(v.trim()).filter(|v| !v.is_empty());
        } else if let Some(v) = line.strip_prefix("updated_at: ") {
            updated = Some(v.trim()).filter(|v| !v.is_empty());
        }
    }
    (imported, updated)
}

/// How many highlight blocks a rendered work file holds.
///
/// scout-archive renders each record as its body (`> ` quote lines, a latex
/// fence or an image link), an optional metadata line and an optional raw
/// note, then a separator: a blank line, `---`, a blank line. Notes are raw
/// Markdown and may hold their own `---` rules, so a `---` line counts only
/// when it has blank lines on both sides and is followed by the end of the
/// file or by a line that can open a record. A note rule followed by more
/// note prose is not counted.
pub fn count_file_records(content: &str) -> usize {
    let body = match content.strip_prefix("---\n") {
        Some(rest) => match rest.find("\n---\n") {
            Some(end) => &rest[end + 5..],
            None => rest,
        },
        None => content,
    };
    let lines: Vec<&str> = body.lines().collect();
    let blank = |i: usize| lines.get(i).is_none_or(|l| l.trim().is_empty());
    let opens_record = |l: &str| {
        l.starts_with("> ")
            || l == ">"
            || l.starts_with("```latex")
            || l.starts_with("![](../assets/")
            || [
                "highlighted_at: ",
                "tags: ",
                "color: ",
                "type: ",
                "format: ",
            ]
            .iter()
            .any(|p| l.starts_with(p))
    };
    (0..lines.len())
        .filter(|&i| {
            lines[i] == "---"
                && i > 0
                && blank(i - 1)
                && blank(i + 1)
                && lines[i + 1..]
                    .iter()
                    .find(|l| !l.trim().is_empty())
                    .is_none_or(|l| opens_record(l))
        })
        .count()
}

/// True for the temp names `write_if_changed` uses (`.<name>.tmp-<pid>-<ns>`).
/// A hard crash between write and rename can leave one behind.
pub fn is_temp_write_name(name: &str) -> bool {
    name.starts_with('.') && name.contains(".tmp-")
}

/// Delete temp-write leftovers older than `max_age` under `readings/` (the
/// only tree `write_if_changed` writes into). Younger ones may belong to a
/// write in progress and are left. Returns how many were removed.
pub fn sweep_stale_temp_files(archive_path: &str, max_age: std::time::Duration) -> usize {
    fn walk(dir: &Path, max_age: std::time::Duration, removed: &mut usize) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                walk(&path, max_age, removed);
                continue;
            }
            if !is_temp_write_name(&entry.file_name().to_string_lossy()) {
                continue;
            }
            let old = meta
                .modified()
                .ok()
                .and_then(|m| m.elapsed().ok())
                .is_some_and(|age| age > max_age);
            if old && fs::remove_file(&path).is_ok() {
                *removed += 1;
            }
        }
    }
    let mut removed = 0;
    walk(
        &Path::new(archive_path).join("readings"),
        max_age,
        &mut removed,
    );
    removed
}

/// One Work per id, first position kept, last occurrence's fields win. The
/// export can list a book on more than one page; rendering it twice would
/// double its highlights in an incremental merge.
pub fn dedupe_works(works: Vec<Work>) -> Vec<Work> {
    let mut pos: HashMap<String, usize> = HashMap::new();
    let mut out: Vec<Work> = Vec::with_capacity(works.len());
    for w in works {
        match pos.get(&w.id) {
            Some(&i) => out[i] = w,
            None => {
                pos.insert(w.id.clone(), out.len());
                out.push(w);
            }
        }
    }
    out
}

/// Render and write one work file per Work. Re-importing is idempotent:
/// `imported_at` is preserved from an existing file, `updated_at` changes
/// only when something else in the rendered file changed, and a file whose
/// content would be byte-identical is not rewritten.
pub fn write_archive(
    archive_path: &str,
    works: &[Work],
    highlights_by_work: &HashMap<String, Vec<&Highlight>>,
) -> Result<WriteSummary> {
    let base = Path::new(archive_path);
    fs::create_dir_all(base.join("readings").join("works"))?;
    fs::create_dir_all(base.join("readings").join("fulltext"))?;
    fs::create_dir_all(base.join("readings").join("assets"))?;

    let works_dir = base.join("readings").join("works");
    let mut summary = WriteSummary::default();
    for work in works {
        let file_path = works_dir.join(format!("{}.md", work.slug));
        let empty = vec![];
        let source_records = highlights_by_work.get(&work.id).unwrap_or(&empty);
        let wrapped: Vec<ArchiveHighlight<'_>> =
            source_records.iter().map(|h| ArchiveHighlight(h)).collect();
        let refs: Vec<&ArchiveHighlight<'_>> = wrapped.iter().collect();

        let existing = fs::read_to_string(&file_path).ok();
        let (prev_imported, prev_updated) = existing
            .as_deref()
            .map(frontmatter_stamps)
            .unwrap_or((None, None));
        let imported_at = prev_imported.unwrap_or(&work.imported_at);

        // Same content under the old updated_at => nothing changed.
        if let (Some(existing), Some(prev_updated)) = (existing.as_deref(), prev_updated) {
            let same = markdown::render_container_file(
                &ArchiveWork {
                    work,
                    imported_at,
                    updated_at: prev_updated,
                },
                &refs,
            );
            if same == existing {
                summary.unchanged += 1;
                continue;
            }
        }

        let content = markdown::render_container_file(
            &ArchiveWork {
                work,
                imported_at,
                updated_at: &work.updated_at,
            },
            &refs,
        );
        if write_if_changed(&file_path, content.as_bytes())? {
            summary.written += 1;
        } else {
            summary.unchanged += 1;
        }
    }

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    fn work(now: &str) -> Work {
        Work {
            id: "zotero-ABC".into(),
            slug: "example-work-abc".into(),
            title: "Example Work".into(),
            author: Some("Example".into()),
            kind: "article".into(),
            source_system: "zotero".into(),
            source_id: Some("ABC".into()),
            url: None,
            imported_at: now.into(),
            updated_at: now.into(),
            source_data: serde_json::json!({"zotero_key": "ABC"}),
        }
    }

    fn highlight(note: Option<&str>) -> Highlight {
        Highlight {
            id: "zotero-h1".into(),
            container_id: "zotero-ABC".into(),
            text: "An annotated sentence.".into(),
            note: note.map(Into::into),
            created_at: Some("2026-08-01T10:00:00Z".into()),
            updated_at: None,
            tags: vec![],
            location: Some("3".into()),
            location_type: None,
            annotation_color: Some("yellow".into()),
            annotation_type: Some("highlight".into()),
            format: "plain".into(),
            source_data: serde_json::Value::Null,
        }
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("hs-archive-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    fn write(dir: &Path, w: &Work, h: &Highlight) -> WriteSummary {
        let mut by: HashMap<String, Vec<&Highlight>> = HashMap::new();
        by.entry(w.id.clone()).or_default().push(h);
        write_archive(dir.to_str().unwrap(), std::slice::from_ref(w), &by).unwrap()
    }

    fn file(dir: &Path) -> std::path::PathBuf {
        dir.join("readings/works/example-work-abc.md")
    }

    /// Push the file's mtime into the past so an unwanted rewrite is visible.
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

    #[test]
    fn reimporting_an_unchanged_item_leaves_bytes_and_mtime_alone() {
        let dir = scratch("unchanged");
        let first = write(&dir, &work("2026-09-01T00:00:00+00:00"), &highlight(None));
        assert_eq!(
            first,
            WriteSummary {
                written: 1,
                unchanged: 0
            }
        );
        let before_bytes = fs::read(file(&dir)).unwrap();
        let before_mtime = age(&file(&dir));

        let again = write(&dir, &work("2026-09-27T07:34:00+00:00"), &highlight(None));
        assert_eq!(
            again,
            WriteSummary {
                written: 0,
                unchanged: 1
            }
        );
        assert_eq!(fs::read(file(&dir)).unwrap(), before_bytes);
        assert_eq!(
            fs::metadata(file(&dir)).unwrap().modified().unwrap(),
            before_mtime
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_changed_annotation_moves_updated_at_but_keeps_imported_at() {
        let dir = scratch("changed");
        write(&dir, &work("2026-09-01T00:00:00+00:00"), &highlight(None));
        let s = write(
            &dir,
            &work("2026-09-27T07:34:00+00:00"),
            &highlight(Some("A new comment.")),
        );
        assert_eq!(s.written, 1);
        let text = fs::read_to_string(file(&dir)).unwrap();
        assert!(
            text.contains("imported_at: 2026-09-01T00:00:00+00:00\n"),
            "{text}"
        );
        assert!(
            text.contains("updated_at: 2026-09-27T07:34:00+00:00\n"),
            "{text}"
        );
        assert!(text.contains("A new comment."));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_without_stamps_takes_the_batch_timestamps() {
        let dir = scratch("nostamps");
        fs::create_dir_all(dir.join("readings/works")).unwrap();
        fs::write(file(&dir), "legacy body\n").unwrap();
        write(&dir, &work("2026-09-27T07:34:00+00:00"), &highlight(None));
        let text = fs::read_to_string(file(&dir)).unwrap();
        assert!(text.contains("imported_at: 2026-09-27T07:34:00+00:00\n"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_if_changed_replaces_the_file_by_rename_and_leaves_no_temp() {
        let dir = scratch("atomic");
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join("work.md");
        let other = dir.join("other-link.md");
        write_if_changed(&p, b"old").unwrap();
        // A second hard link to the old inode: an in-place write would change
        // it too; a temp-file-then-rename leaves it holding the old bytes.
        fs::hard_link(&p, &other).unwrap();
        assert!(write_if_changed(&p, b"new").unwrap());
        assert_eq!(fs::read(&p).unwrap(), b"new");
        assert_eq!(fs::read(&other).unwrap(), b"old");
        let names: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(names.iter().all(|n| !n.contains(".tmp-")), "{names:?}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn counts_the_records_in_a_rendered_file() {
        let dir = scratch("count");
        let w = work("2026-09-01T00:00:00+00:00");
        let (a, mut b) = (highlight(None), highlight(Some("a note")));
        b.id = "zotero-h2".into();
        let mut by: HashMap<String, Vec<&Highlight>> = HashMap::new();
        by.entry(w.id.clone()).or_default().extend([&a, &b]);
        write_archive(dir.to_str().unwrap(), std::slice::from_ref(&w), &by).unwrap();
        assert_eq!(
            count_file_records(&fs::read_to_string(file(&dir)).unwrap()),
            2
        );
        by.insert(w.id.clone(), vec![]);
        write_archive(dir.to_str().unwrap(), std::slice::from_ref(&w), &by).unwrap();
        assert_eq!(
            count_file_records(&fs::read_to_string(file(&dir)).unwrap()),
            0
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_note_with_its_own_rule_is_not_counted_as_a_record() {
        let dir = scratch("noterule");
        let w = work("2026-09-01T00:00:00+00:00");
        let mut a = highlight(Some("first thought\n\n---\n\nsecond thought"));
        a.id = "zotero-h1".into();
        let mut b = highlight(Some("tight\n---\nrule"));
        b.id = "zotero-h2".into();
        let c = {
            let mut c = highlight(Some("ends on a rule\n\n---"));
            c.id = "zotero-h3".into();
            c
        };
        let mut by: HashMap<String, Vec<&Highlight>> = HashMap::new();
        by.entry(w.id.clone()).or_default().extend([&a, &b, &c]);
        write_archive(dir.to_str().unwrap(), std::slice::from_ref(&w), &by).unwrap();
        assert_eq!(
            count_file_records(&fs::read_to_string(file(&dir)).unwrap()),
            3
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn stale_temp_writes_are_swept_and_fresh_ones_and_real_files_kept() {
        let dir = scratch("sweep");
        let works = dir.join("readings/works");
        let assets = dir.join("readings/assets");
        fs::create_dir_all(&works).unwrap();
        fs::create_dir_all(&assets).unwrap();
        let stale = works.join(".a.md.tmp-123-456");
        let stale_asset = assets.join(".x.png.tmp-1-2");
        let fresh = works.join(".b.md.tmp-123-789");
        let real = works.join("a.md");
        let dotfile = works.join(".keep");
        for p in [&stale, &stale_asset, &fresh, &real, &dotfile] {
            fs::write(p, b"x").unwrap();
        }
        let aged = |p: &Path| {
            fs::File::options()
                .write(true)
                .open(p)
                .unwrap()
                .set_modified(SystemTime::now() - Duration::from_secs(7200))
                .unwrap();
        };
        aged(&stale);
        aged(&stale_asset);
        aged(&real);
        aged(&dotfile);
        let removed = sweep_stale_temp_files(dir.to_str().unwrap(), Duration::from_secs(3600));
        assert_eq!(removed, 2);
        assert!(!stale.exists() && !stale_asset.exists());
        assert!(fresh.exists() && real.exists() && dotfile.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn dedupe_keeps_one_work_per_id() {
        let mut later = work("2026-09-27T00:00:00+00:00");
        later.title = "Later".into();
        let mut other = work("x");
        other.id = "zotero-XYZ".into();
        let out = dedupe_works(vec![work("a"), other, later]);
        let got: Vec<(&str, &str)> = out
            .iter()
            .map(|w| (w.id.as_str(), w.title.as_str()))
            .collect();
        assert_eq!(
            got,
            [("zotero-ABC", "Later"), ("zotero-XYZ", "Example Work")]
        );
    }

    #[test]
    fn write_if_changed_skips_identical_bytes() {
        let dir = scratch("wic");
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join("a.png");
        assert!(write_if_changed(&p, b"abc").unwrap());
        let before = age(&p);
        assert!(!write_if_changed(&p, b"abc").unwrap());
        assert_eq!(fs::metadata(&p).unwrap().modified().unwrap(), before);
        assert!(write_if_changed(&p, b"abd").unwrap());
        let _ = fs::remove_dir_all(&dir);
    }
}
