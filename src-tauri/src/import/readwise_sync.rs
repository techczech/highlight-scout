//! One Readwise highlight sync, from export to archive files and index,
//! behind a seam that takes the export source, the index connection and the
//! archive path. `commands::import::run_import` wraps it with the window,
//! config cursor and Reader full text.
//!
//! Invariant: a highlight present in a work file and not deleted by Readwise
//! is never removed by a sync.

use std::future::Future;
use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;

use crate::commands::import::{persist_core, PersistBatch};
use crate::import::readwise::{merge_incremental, sort_reading_order, ExportBatch, ReadwiseClient};
use crate::models::{Highlight, ImportStatus, Work};

/// Where export batches come from: the Readwise API, or a fake in tests.
pub trait ExportSource {
    /// `updated_after` None asks for a full export.
    fn export(
        &self,
        updated_after: Option<&str>,
    ) -> impl Future<Output = anyhow::Result<ExportBatch>> + Send;
}

impl ExportSource for ReadwiseClient {
    async fn export(&self, updated_after: Option<&str>) -> anyhow::Result<ExportBatch> {
        self.import_export(updated_after).await
    }
}

pub struct SyncOutcome {
    pub status: ImportStatus,
    /// The works written this run (all of them after a full export).
    pub works: Vec<Work>,
    /// True when an incremental found the index short of a work file and a
    /// full export ran instead.
    pub fell_back_to_full: bool,
}

/// Run one sync. Incremental when `updated_after` is given: each work file is
/// rendered from the indexed highlights merged with the changed ones. If any
/// existing work file holds more highlights than the index knows for it, the
/// index cannot be trusted as the merge base, so the run switches to a full
/// export (every file rendered from Readwise's complete list).
pub async fn sync_readwise<S: ExportSource>(
    src: &S,
    db: &Mutex<Connection>,
    archive_path: &str,
    updated_after: Option<&str>,
    progress: &(dyn Fn(&str, usize, usize) + Sync),
) -> Result<SyncOutcome, String> {
    let mut batch = src.export(updated_after).await.map_err(|e| e.to_string())?;
    batch.works = crate::archive_meta::dedupe_works(std::mem::take(&mut batch.works));

    if batch.works.is_empty() && batch.deleted_ids.is_empty() {
        return Ok(SyncOutcome {
            status: ImportStatus {
                works_imported: 0,
                highlights_imported: 0,
                message: "Already up to date".into(),
            },
            works: vec![],
            fell_back_to_full: false,
        });
    }

    let mut incremental = updated_after.is_some();
    if incremental && !batch.works.is_empty() {
        let short = {
            let conn = db.lock().map_err(|e| e.to_string())?;
            works_with_short_index(&conn, archive_path, &batch.works).map_err(|e| e.to_string())?
        };
        if !short.is_empty() {
            progress(
                &format!(
                    "Index is missing highlights for {} works; running a full export so none are dropped…",
                    short.len()
                ),
                0,
                0,
            );
            let deleted = std::mem::take(&mut batch.deleted_ids);
            batch = src.export(None).await.map_err(|e| e.to_string())?;
            batch.works = crate::archive_meta::dedupe_works(std::mem::take(&mut batch.works));
            batch.deleted_ids.extend(deleted);
            incremental = false;
        }
    }

    let archive_records: Vec<Highlight> = if incremental {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let mut all = Vec::new();
        for work in &batch.works {
            let incoming: Vec<&Highlight> = batch
                .highlights
                .iter()
                .map(|(h, _, _)| h)
                .filter(|h| h.container_id == work.id)
                .collect();
            let known = index_highlights(&conn, &work.id).map_err(|e| e.to_string())?;
            all.extend(merge_incremental(known, &incoming, &batch.deleted_ids));
        }
        all
    } else {
        let mut all: Vec<Highlight> = batch
            .highlights
            .iter()
            .map(|(h, _, _)| h.clone())
            .filter(|h| !batch.deleted_ids.contains(&h.id))
            .collect();
        sort_reading_order(&mut all);
        all
    };

    let mut status = persist_core(
        db,
        archive_path,
        PersistBatch {
            source: "readwise",
            works: &batch.works,
            highlights: &batch.highlights,
            archive_records: Some(&archive_records),
            deleted_ids: &batch.deleted_ids,
            raw_json: Some(&batch.raw_json),
        },
        progress,
    )?;
    if batch.works.is_empty() {
        status.message = format!(
            "Already up to date · {} deleted highlights removed from the index",
            batch.deleted_ids.len()
        );
    }
    Ok(SyncOutcome {
        status,
        works: batch.works,
        fell_back_to_full: updated_after.is_some() && !incremental,
    })
}

/// Ids of works whose existing archive file holds more highlight blocks than
/// the index has rows for (including an index that holds none).
fn works_with_short_index(
    conn: &Connection,
    archive_path: &str,
    works: &[Work],
) -> rusqlite::Result<Vec<String>> {
    let dir = Path::new(archive_path).join("readings").join("works");
    let mut short = Vec::new();
    for w in works {
        let Ok(content) = std::fs::read_to_string(dir.join(format!("{}.md", w.slug))) else {
            continue;
        };
        let in_file = crate::archive_meta::count_file_records(&content);
        if in_file == 0 {
            continue;
        }
        let in_index: i64 = conn.query_row(
            "SELECT COUNT(*) FROM highlights WHERE work_id = ?1",
            [&w.id],
            |r| r.get(0),
        )?;
        if (in_index as usize) < in_file {
            short.push(w.id.clone());
        }
    }
    Ok(short)
}

/// Every highlight the index holds for one work (the merge base for an
/// incremental sync).
fn index_highlights(conn: &Connection, work_id: &str) -> rusqlite::Result<Vec<Highlight>> {
    let mut stmt = conn.prepare(
        "SELECT id, work_id, text, note, highlighted_at, updated_at, tags, location,
                location_type, annotation_color, annotation_type, format, source_data
         FROM highlights WHERE work_id = ?1",
    )?;
    let rows = stmt
        .query_map([work_id], |r| {
            let tags: String = r.get(6)?;
            let sd: String = r.get(12)?;
            Ok(Highlight {
                id: r.get(0)?,
                container_id: r.get(1)?,
                text: r.get(2)?,
                note: r.get(3)?,
                created_at: r.get(4)?,
                updated_at: r.get(5)?,
                tags: serde_json::from_str(&tags).unwrap_or_default(),
                location: r.get(7)?,
                location_type: r.get(8)?,
                annotation_color: r.get(9)?,
                annotation_type: r.get(10)?,
                format: r.get(11)?,
                source_data: serde_json::from_str(&sd).unwrap_or(serde_json::Value::Null),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::fs;
    use std::path::PathBuf;

    /// Replays prepared batches in order and records each call's cursor.
    struct FakeSource {
        responses: Mutex<VecDeque<ExportBatch>>,
        calls: Mutex<Vec<Option<String>>>,
    }

    impl FakeSource {
        fn new(responses: Vec<ExportBatch>) -> Self {
            FakeSource {
                responses: Mutex::new(responses.into()),
                calls: Mutex::new(vec![]),
            }
        }
        fn calls(&self) -> Vec<Option<String>> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl ExportSource for FakeSource {
        async fn export(&self, updated_after: Option<&str>) -> anyhow::Result<ExportBatch> {
            self.calls
                .lock()
                .unwrap()
                .push(updated_after.map(String::from));
            self.responses
                .lock()
                .unwrap()
                .pop_front()
                .ok_or_else(|| anyhow::anyhow!("fake: no more responses"))
        }
    }

    const AFTER: Option<&str> = Some("2026-09-01T00:00:00+00:00");

    fn book() -> Work {
        Work {
            id: "rw_book_1".into(),
            slug: "author-a-book-1".into(),
            title: "A Book".into(),
            author: Some("Author A".into()),
            kind: "book".into(),
            source_system: "readwise".into(),
            source_id: Some("1".into()),
            url: None,
            imported_at: "2026-08-01T00:00:00+00:00".into(),
            updated_at: "2026-08-01T00:00:00+00:00".into(),
            source_data: serde_json::json!({"user_book_id": 1}),
        }
    }

    fn hl(id: &str, loc: &str, text: &str) -> Highlight {
        Highlight {
            id: format!("rw_highlight_{id}"),
            container_id: "rw_book_1".into(),
            text: text.into(),
            note: None,
            created_at: Some("2026-08-01T10:00:00Z".into()),
            updated_at: None,
            tags: vec![],
            location: Some(loc.into()),
            location_type: Some("location".into()),
            annotation_color: None,
            annotation_type: None,
            format: "plain".into(),
            source_data: serde_json::Value::Null,
        }
    }

    fn batch(works: Vec<Work>, hs: Vec<Highlight>, deleted: &[&str]) -> ExportBatch {
        ExportBatch {
            works,
            highlights: hs
                .into_iter()
                .map(|h| (h, "A Book".to_string(), Some("Author A".to_string())))
                .collect(),
            deleted_ids: deleted
                .iter()
                .map(|d| format!("rw_highlight_{d}"))
                .collect(),
            raw_json: "{}".into(),
        }
    }

    struct Env {
        dir: PathBuf,
        db: Mutex<Connection>,
    }

    impl Env {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("hs-rwsync-{}-{}", name, std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            Env {
                dir,
                db: Mutex::new(fresh_index()),
            }
        }
        fn archive(&self) -> &str {
            self.dir.to_str().unwrap()
        }
        fn file(&self) -> String {
            fs::read_to_string(self.dir.join("readings/works/author-a-book-1.md")).unwrap()
        }
        async fn sync(&self, src: &FakeSource, after: Option<&str>) -> SyncOutcome {
            sync_readwise(src, &self.db, self.archive(), after, &|_, _, _| {})
                .await
                .unwrap()
        }
        fn index_count(&self, table: &str, col: &str, id: &str) -> i64 {
            self.db
                .lock()
                .unwrap()
                .query_row(
                    &format!("SELECT COUNT(*) FROM {table} WHERE {col} = ?1"),
                    [id],
                    |r| r.get(0),
                )
                .unwrap()
        }
    }

    impl Drop for Env {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    fn fresh_index() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        scout_index::sqlite::init_schema(&c).unwrap();
        c
    }

    /// The quoted texts in file order.
    fn quotes(file: &str) -> Vec<String> {
        file.lines()
            .filter_map(|l| l.strip_prefix("> "))
            .map(String::from)
            .collect()
    }

    async fn seed_full(env: &Env) {
        let src = FakeSource::new(vec![batch(
            vec![book()],
            vec![
                hl("1", "10", "alpha"),
                hl("2", "20", "beta"),
                hl("3", "30", "gamma"),
            ],
            &[],
        )]);
        env.sync(&src, None).await;
        assert_eq!(quotes(&env.file()), ["alpha", "beta", "gamma"]);
    }

    // (c) The 2026-08-01 loss: an incremental rendered only the changed
    // highlights. If the merge wiring is dropped (archive rendered from the
    // batch alone), this goes red.
    #[tokio::test]
    async fn an_incremental_keeps_the_earlier_highlights_in_the_file() {
        let env = Env::new("keeps");
        seed_full(&env).await;
        let src = FakeSource::new(vec![batch(vec![book()], vec![hl("4", "25", "delta")], &[])]);
        let out = env.sync(&src, AFTER).await;
        assert!(!out.fell_back_to_full);
        assert_eq!(src.calls(), [AFTER.map(String::from)]);
        assert_eq!(quotes(&env.file()), ["alpha", "beta", "delta", "gamma"]);
    }

    // (a) A deletion must reach the index, or the next incremental's merge
    // base still holds the deleted highlight and writes it back.
    #[tokio::test]
    async fn a_deleted_highlight_stays_deleted_after_a_later_change() {
        let env = Env::new("deleted");
        seed_full(&env).await;

        let delete_gamma = FakeSource::new(vec![batch(vec![book()], vec![], &["3"])]);
        env.sync(&delete_gamma, AFTER).await;
        assert_eq!(quotes(&env.file()), ["alpha", "beta"]);
        assert_eq!(env.index_count("highlights", "id", "rw_highlight_3"), 0);
        assert_eq!(
            env.index_count("search_index", "highlight_id", "rw_highlight_3"),
            0
        );

        let change_beta = FakeSource::new(vec![batch(
            vec![book()],
            vec![hl("2", "20", "beta revised")],
            &[],
        )]);
        let out = env.sync(&change_beta, AFTER).await;
        assert!(!out.fell_back_to_full);
        assert_eq!(quotes(&env.file()), ["alpha", "beta revised"]);
    }

    // (b) An incremental over an existing file with an empty index (index
    // lost, reset or pulled stale) must not shrink the file.
    #[tokio::test]
    async fn an_incremental_over_an_empty_index_loses_nothing() {
        let mut env = Env::new("emptyindex");
        seed_full(&env).await;
        env.db = Mutex::new(fresh_index());

        let src = FakeSource::new(vec![
            batch(vec![book()], vec![hl("2", "20", "beta revised")], &[]),
            // The full export the fallback asks for.
            batch(
                vec![book()],
                vec![
                    hl("1", "10", "alpha"),
                    hl("2", "20", "beta revised"),
                    hl("3", "30", "gamma"),
                ],
                &[],
            ),
        ]);
        let out = env.sync(&src, AFTER).await;
        assert!(out.fell_back_to_full);
        assert_eq!(src.calls(), [AFTER.map(String::from), None]);
        assert_eq!(quotes(&env.file()), ["alpha", "beta revised", "gamma"]);
        // The full export also healed the index.
        assert_eq!(env.index_count("highlights", "work_id", "rw_book_1"), 3);
    }

    #[tokio::test]
    async fn an_incremental_over_a_short_index_loses_nothing() {
        let env = Env::new("shortindex");
        seed_full(&env).await;
        env.db
            .lock()
            .unwrap()
            .execute(
                "DELETE FROM highlights WHERE id IN ('rw_highlight_1','rw_highlight_3')",
                [],
            )
            .unwrap();

        let src = FakeSource::new(vec![
            batch(vec![book()], vec![hl("4", "40", "delta")], &[]),
            batch(
                vec![book()],
                vec![
                    hl("1", "10", "alpha"),
                    hl("2", "20", "beta"),
                    hl("3", "30", "gamma"),
                    hl("4", "40", "delta"),
                ],
                &[],
            ),
        ]);
        let out = env.sync(&src, AFTER).await;
        assert!(out.fell_back_to_full);
        assert_eq!(quotes(&env.file()), ["alpha", "beta", "gamma", "delta"]);
    }

    // Full and incremental paths write the same order, so an incremental
    // that changes nothing leaves the file byte-identical.
    #[tokio::test]
    async fn full_and_incremental_write_the_same_order() {
        let env = Env::new("order");
        let mut unlocated = hl("5", "", "unlocated");
        unlocated.location = None;
        let src = FakeSource::new(vec![batch(
            vec![book()],
            vec![
                unlocated,
                hl("1", "10", "ten"),
                hl("2", "9.5", "nine and a half"),
                hl("3", "2", "two"),
            ],
            &[],
        )]);
        env.sync(&src, None).await;
        let full = env.file();
        assert_eq!(
            quotes(&full),
            ["two", "nine and a half", "ten", "unlocated"]
        );

        let same_again =
            FakeSource::new(vec![batch(vec![book()], vec![hl("1", "10", "ten")], &[])]);
        env.sync(&same_again, AFTER).await;
        assert_eq!(env.file(), full);
    }

    // A book listed twice in one export must not double its highlights.
    #[tokio::test]
    async fn a_work_listed_twice_in_one_batch_is_rendered_once() {
        let env = Env::new("dupe");
        seed_full(&env).await;
        let src = FakeSource::new(vec![batch(
            vec![book(), book()],
            vec![hl("4", "40", "delta"), hl("5", "50", "epsilon")],
            &[],
        )]);
        env.sync(&src, AFTER).await;
        assert_eq!(
            quotes(&env.file()),
            ["alpha", "beta", "gamma", "delta", "epsilon"]
        );
    }

    // A raw note holding its own `---` rule must not make the file look
    // longer than the index, or that book forces a full export every sync.
    #[tokio::test]
    async fn a_note_with_a_rule_does_not_force_a_full_export_every_sync() {
        let env = Env::new("noterule");
        let mut ruled = hl("2", "20", "beta");
        ruled.note = Some("first thought\n\n---\n\nsecond thought".into());
        let full = FakeSource::new(vec![batch(
            vec![book()],
            vec![hl("1", "10", "alpha"), ruled],
            &[],
        )]);
        env.sync(&full, None).await;

        let src = FakeSource::new(vec![batch(vec![book()], vec![hl("3", "30", "gamma")], &[])]);
        let out = env.sync(&src, AFTER).await;
        assert!(!out.fell_back_to_full);
        assert_eq!(src.calls(), [AFTER.map(String::from)]);
        assert_eq!(quotes(&env.file()), ["alpha", "beta", "gamma"]);
    }

    // Export JSON fed through build_batch, the same path a live sync takes.
    // Each book: (user_book_id, is_deleted, [(id, is_deleted, location, text)]).
    type JsonBook<'a> = (u64, bool, &'a [(u64, bool, &'a str, &'a str)]);

    fn export_batch(books: &[JsonBook<'_>]) -> ExportBatch {
        let results: Vec<serde_json::Value> = books
            .iter()
            .map(|(book_id, deleted, hs)| {
                serde_json::json!({
                    "user_book_id": book_id,
                    "is_deleted": deleted,
                    "title": format!("Book {book_id}"),
                    "author": "Author A",
                    "category": "books",
                    "highlights": hs.iter().map(|(id, del, loc, text)| serde_json::json!({
                        "id": id, "is_deleted": del, "location": loc, "text": text,
                        "highlighted_at": "2026-08-01T10:00:00Z",
                    })).collect::<Vec<_>>(),
                })
            })
            .collect();
        let body = serde_json::json!({"nextPageCursor": null, "results": results}).to_string();
        crate::import::readwise::batch_from_export_json(&body, "2026-09-27T00:00:00+00:00").unwrap()
    }

    fn file_of(env: &Env, book_id: u64) -> String {
        let slug = scout_archive::markdown::make_slug(
            Some("Author A"),
            &format!("Book {book_id}"),
            &book_id.to_string(),
        );
        fs::read_to_string(env.dir.join(format!("readings/works/{slug}.md"))).unwrap()
    }

    fn index_rows(env: &Env, work_id: &str) -> Vec<(String, String)> {
        let conn = env.db.lock().unwrap();
        let mut st = conn
            .prepare(
                "SELECT h.id, h.text FROM highlights h WHERE h.work_id = ?1
                 UNION ALL
                 SELECT 'fts:' || s.highlight_id, s.text FROM search_index s WHERE s.work_id = ?1
                 ORDER BY 1",
            )
            .unwrap();
        st.query_map([work_id], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    // Live wins over deleted: an id deleted in one book and live in another
    // (a moved highlight) is rendered and indexed; an id deleted everywhere
    // is neither.
    #[tokio::test]
    async fn a_full_export_renders_an_id_live_elsewhere_and_drops_one_deleted_everywhere() {
        let env = Env::new("livewins");
        let seed = FakeSource::new(vec![export_batch(&[(
            1,
            false,
            &[
                (1, false, "10", "alpha"),
                (2, false, "20", "beta"),
                (3, false, "30", "gamma"),
            ],
        )])]);
        env.sync(&seed, None).await;
        assert_eq!(quotes(&file_of(&env, 1)), ["alpha", "beta", "gamma"]);

        let src = FakeSource::new(vec![export_batch(&[
            (
                1,
                false,
                &[
                    (1, false, "10", "alpha"),
                    (2, true, "20", "beta"),
                    (3, true, "30", "gamma"),
                ],
            ),
            (7, false, &[(3, false, "5", "gamma")]),
        ])]);
        env.sync(&src, None).await;
        assert_eq!(quotes(&file_of(&env, 1)), ["alpha"]);
        assert_eq!(quotes(&file_of(&env, 7)), ["gamma"]);
        assert_eq!(env.index_count("highlights", "id", "rw_highlight_2"), 0);
        assert_eq!(
            env.index_count("search_index", "highlight_id", "rw_highlight_2"),
            0
        );
        assert_eq!(env.index_count("highlights", "id", "rw_highlight_3"), 1);
        assert_eq!(
            env.index_count("search_index", "highlight_id", "rw_highlight_3"),
            1
        );
    }

    // A book deleted in Readwise keeps its file and stays searchable: the
    // sync leaves its file bytes and its index rows exactly as they were.
    #[tokio::test]
    async fn a_deleted_book_keeps_its_file_and_index_rows() {
        let env = Env::new("deletedbook");
        let seed = FakeSource::new(vec![export_batch(&[
            (
                1,
                false,
                &[(1, false, "10", "alpha"), (2, false, "20", "beta")],
            ),
            (2, false, &[(9, false, "1", "other book")]),
        ])]);
        env.sync(&seed, None).await;
        let file_before = file_of(&env, 1);
        let rows_before = index_rows(&env, "rw_book_1");
        assert_eq!(rows_before.len(), 4);

        let src = FakeSource::new(vec![export_batch(&[
            (
                1,
                true,
                &[(1, false, "10", "alpha"), (2, true, "20", "beta")],
            ),
            (2, false, &[(10, false, "2", "new in other book")]),
        ])]);
        let out = env.sync(&src, AFTER).await;
        assert!(!out.fell_back_to_full);
        assert_eq!(file_of(&env, 1), file_before);
        assert_eq!(index_rows(&env, "rw_book_1"), rows_before);
        assert_eq!(
            quotes(&file_of(&env, 2)),
            ["other book", "new in other book"]
        );
    }
}
