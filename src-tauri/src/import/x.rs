// X (Twitter) importer (ADR-0013): reads the birdclaw-produced compact
// `saved.jsonl` (likes + bookmarks). One saved tweet = one Work (kind
// "tweet") + one Highlight, keyed by the native tweet ID (`x-{tweet_id}`) so
// re-import after birdclaw enriches authors/threads upserts rather than
// duplicating. Quoted/parent context is embedded into the highlight body when
// present. Own tweets are excluded upstream (they are writing, not highlights).

use anyhow::{bail, Result};
use chrono::Utc;
use serde::Deserialize;

use crate::models::{Highlight, Work};

#[derive(Debug, Deserialize)]
struct SavedTweet {
    tweet_id: String,
    #[serde(default)]
    saved_as: Option<String>, // "likes" | "bookmarks"
    text: String,
    #[serde(default)]
    author_handle: Option<String>,
    #[serde(default)]
    author_name: Option<String>,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    reply_to_id: Option<String>,
    #[serde(default)]
    parent_text: Option<String>,
    #[serde(default)]
    parent_handle: Option<String>,
    #[serde(default)]
    quoted_tweet_id: Option<String>,
    #[serde(default)]
    quoted_text: Option<String>,
    #[serde(default)]
    quoted_handle: Option<String>,
    #[serde(default)]
    images: Vec<String>,
    #[serde(default)]
    article_urls: Vec<String>,
}

pub fn import(path: &str) -> Result<(Vec<Work>, Vec<(Highlight, String, Option<String>)>)> {
    let content = std::fs::read_to_string(path)?;
    let now = Utc::now().to_rfc3339();

    let mut works = Vec::new();
    let mut highlights = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(t) = serde_json::from_str::<SavedTweet>(line) else {
            continue; // skip a malformed line rather than fail the whole import
        };
        if t.text.trim().is_empty() {
            continue;
        }

        let t = crate::import::tweet_common::TweetInput {
            tweet_id: t.tweet_id.clone(),
            text: t.text.clone(),
            author_handle: t.author_handle.clone(),
            author_name: t.author_name.clone(),
            created_at: t.created_at.clone(),
            url: t.url.clone(),
            images: t.images.clone(),
            article_urls: t.article_urls.clone(),
            saved_as: t.saved_as.clone(),
            reply_to_id: t.reply_to_id.clone(),
            parent_text: t.parent_text.clone(),
            parent_handle: t.parent_handle.clone(),
            quoted_tweet_id: t.quoted_tweet_id.clone(),
            quoted_text: t.quoted_text.clone(),
            quoted_handle: t.quoted_handle.clone(),
            body_markdown: None,
        };
        let (work, highlight, title) = crate::import::tweet_common::make_records(&t, &now);
        let author = work.author.clone();
        if seen.insert(work.id.clone()) {
            works.push(work);
        }
        highlights.push((highlight, title, author));
    }

    if highlights.is_empty() {
        bail!("No saved tweets found in {}", path);
    }
    Ok((works, highlights))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::tweet_common::make_records;

    fn write_tmp(name: &str, body: &str) -> String {
        let p = std::env::temp_dir().join(name);
        std::fs::write(&p, body).unwrap();
        p.to_str().unwrap().to_string()
    }

    #[test]
    fn imports_like_and_bookmark_with_context() {
        let l1 = r#"{"tweet_id":"111","saved_as":"likes","text":"hello world","author_handle":"alice","author_name":"Alice","created_at":"2024-01-02T00:00:00.000Z","url":"https://x.com/alice/status/111"}"#;
        let l2 = r#"{"tweet_id":"222","saved_as":"bookmarks","text":"my comment","author_handle":"bob","created_at":"2024-03-04T00:00:00.000Z","url":"https://x.com/bob/status/222","quoted_tweet_id":"999","quoted_text":"the original claim","quoted_handle":"carol","images":["https://pbs.twimg.com/media/AAA.jpg"],"article_urls":["https://example.com/post"]}"#;
        let path = write_tmp("hs-x-test.jsonl", &format!("{}\n{}\n", l1, l2));

        let (works, hls) = import(&path).unwrap();
        assert_eq!(works.len(), 2);
        assert_eq!(hls.len(), 2);

        // native tweet-id keying + tweet work type
        assert_eq!(works[0].id, "x-w-111");
        assert_eq!(works[0].kind, "tweet");
        assert_eq!(works[0].source_system, "x");
        assert_eq!(hls[0].0.id, "x-111");
        assert_eq!(
            hls[0].0.created_at.as_deref(),
            Some("2024-01-02T00:00:00.000Z")
        );
        assert_eq!(hls[0].0.tags, vec!["like".to_string()]);

        // quoted context embedded into the bookmark's body
        let bm_body = &hls[1].0.text;
        assert!(bm_body.contains("my comment"));
        assert!(bm_body.contains("Quoting @carol"));
        assert!(bm_body.contains("the original claim"));
        assert!(bm_body.contains("https://example.com/post")); // article link embedded
        assert!(bm_body.contains("https://pbs.twimg.com/media/AAA.jpg")); // image embedded
        assert_eq!(hls[1].0.tags, vec!["bookmark".to_string()]);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn author_less_tweet_imports_with_anonymous_url() {
        let line = r#"{"tweet_id":"333","saved_as":"likes","text":"orphan tweet","author_handle":null,"created_at":null,"url":"https://x.com/i/web/status/333"}"#;
        let path = write_tmp("hs-x-test2.jsonl", &format!("{}\n", line));
        let (works, hls) = import(&path).unwrap();
        assert_eq!(works[0].author, None);
        assert_eq!(hls[0].0.created_at, None);
        assert_eq!(
            works[0].url.as_deref(),
            Some("https://x.com/i/web/status/333")
        );
        let _ = std::fs::remove_file(&path);
    }

    /// A scratch archive, index and saved.jsonl for the headless import.
    struct Headless {
        dir: std::path::PathBuf,
    }

    impl Headless {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("hs-x-headless-{}-{}", name, std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Headless { dir }
        }
        /// `--import-x` with these saved.jsonl lines.
        fn run(&self, lines: &[String]) -> crate::import::tweet_upsert::TweetReport {
            let saved = self.dir.join("saved.jsonl");
            std::fs::write(&saved, lines.join("\n")).unwrap();
            let archive = self.dir.join("archive");
            crate::import_x_into(
                saved.to_str().unwrap(),
                archive.to_str().unwrap(),
                &self.dir.join("index.db"),
            )
            .unwrap()
            .1
        }
        fn files(&self) -> Vec<String> {
            let mut n: Vec<String> = std::fs::read_dir(self.dir.join("archive/readings/works"))
                .unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            n.sort();
            n
        }
        fn text(&self, name: &str) -> String {
            std::fs::read_to_string(self.dir.join("archive/readings/works").join(name)).unwrap()
        }
        /// (slug, highlight text, tags) the index holds for a tweet.
        fn indexed(&self, tweet_id: &str) -> (String, String, String) {
            let conn = rusqlite::Connection::open(self.dir.join("index.db")).unwrap();
            let slug = conn
                .query_row(
                    "SELECT slug FROM works WHERE id = ?1",
                    [format!("x-w-{tweet_id}")],
                    |r| r.get(0),
                )
                .unwrap();
            let (text, tags) = conn
                .query_row(
                    "SELECT text, tags FROM highlights WHERE id = ?1",
                    [format!("x-{tweet_id}")],
                    |r| {
                        Ok((
                            r.get(0)?,
                            r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                        ))
                    },
                )
                .unwrap();
            (slug, text, tags)
        }
    }

    impl Drop for Headless {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn saved(id: &str, saved_as: &str, text: &str, created_at: Option<&str>) -> String {
        serde_json::json!({
            "tweet_id": id, "saved_as": saved_as, "text": text,
            "author_handle": "alice", "author_name": "Alice", "created_at": created_at,
            "url": format!("https://x.com/alice/status/{id}"),
        })
        .to_string()
    }

    const CUT: &str = "A long post about the reading archive that X cut off in the first export because it ran past the… https://t.co/AbCdEf1234";
    const WHOLE: &str = "A long post about the reading archive that X cut off in the first export because it ran past the limit, and this is how it ends.";

    #[test]
    fn importing_the_same_file_twice_leaves_one_file_and_writes_nothing() {
        let env = Headless::new("twice");
        let lines = [
            saved(
                "1721081463234609266",
                "likes",
                WHOLE,
                Some("2023-11-05T09:00:00.000Z"),
            ),
            saved("1721081463234609999", "bookmarks", "a second tweet", None),
        ];
        let first = env.run(&lines);
        assert_eq!(first.created, 2);
        let names = env.files();
        assert_eq!(names.len(), 2);
        let before: Vec<(Vec<u8>, std::time::SystemTime)> = names
            .iter()
            .map(|n| {
                let p = env.dir.join("archive/readings/works").join(n);
                let old = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
                std::fs::File::options()
                    .write(true)
                    .open(&p)
                    .unwrap()
                    .set_modified(old)
                    .unwrap();
                (
                    std::fs::read(&p).unwrap(),
                    std::fs::metadata(&p).unwrap().modified().unwrap(),
                )
            })
            .collect();

        let again = env.run(&lines);
        assert_eq!((again.created, again.updated, again.unchanged), (0, 0, 2));
        assert_eq!(env.files(), names);
        for (n, (bytes, mtime)) in names.iter().zip(&before) {
            let p = env.dir.join("archive/readings/works").join(n);
            assert_eq!(&std::fs::read(&p).unwrap(), bytes);
            assert_eq!(&std::fs::metadata(&p).unwrap().modified().unwrap(), mtime);
        }
        // The tweet X gave no date for is dated from its id.
        let undated = names.iter().find(|n| n.contains("a-second-tweet")).unwrap();
        assert!(env
            .text(undated)
            .contains("highlighted_at: 2023-11-05 | tags: bookmark\n"));
    }

    #[test]
    fn a_fuller_second_import_updates_the_first_file_and_a_shorter_one_does_not() {
        let env = Headless::new("fuller");
        let id = "1721081463234609266";
        env.run(&[saved(id, "likes", CUT, Some("2023-11-05T09:00:00.000Z"))]);
        let names = env.files();
        assert_eq!(names.len(), 1);

        // 21 June: the same tweet with its whole text, so a different slug.
        let r = env.run(&[saved(id, "likes", WHOLE, Some("2023-11-05T09:00:00.000Z"))]);
        assert_eq!((r.created, r.updated), (0, 1));
        assert_eq!(env.files(), names, "same file, old name, no second file");
        let text = env.text(&names[0]);
        assert!(text.contains("and this is how it ends."), "{text}");
        assert!(!text.contains("t.co/AbCdEf1234"), "{text}");
        let (slug, indexed_text, _) = env.indexed(id);
        assert_eq!(
            format!("{slug}.md"),
            names[0],
            "the index points at the file that exists"
        );
        assert_eq!(indexed_text, WHOLE);

        // The cut-off text again: nothing changes, in the file or the index.
        let r = env.run(&[saved(id, "likes", CUT, Some("2023-11-05T09:00:00.000Z"))]);
        assert_eq!((r.created, r.updated, r.unchanged), (0, 0, 1));
        assert_eq!(env.text(&names[0]), text);
        assert_eq!(env.indexed(id).1, WHOLE);
    }

    #[test]
    fn a_like_then_a_bookmark_keeps_the_users_note_and_tag_and_has_both_tags() {
        let env = Headless::new("usertags");
        let id = "1721081463234609266";
        env.run(&[saved(id, "likes", WHOLE, Some("2023-11-05T09:00:00.000Z"))]);
        let names = env.files();
        let path = env.dir.join("archive/readings/works").join(&names[0]);
        let edited = env.text(&names[0]).replace(
            "highlighted_at: 2023-11-05 | tags: like\n",
            "highlighted_at: 2023-11-05 | tags: like, archives\n\nWorth rereading.\n",
        );
        std::fs::write(&path, &edited).unwrap();

        env.run(&[saved(
            id,
            "bookmarks",
            WHOLE,
            Some("2023-11-05T09:00:00.000Z"),
        )]);
        assert_eq!(env.files(), names);
        let text = env.text(&names[0]);
        assert!(
            text.contains(
                "highlighted_at: 2023-11-05 | tags: like, archives, bookmark\n\nWorth rereading.\n"
            ),
            "{text}"
        );
        let (_, _, tags) = env.indexed(id);
        for t in ["like", "archives", "bookmark"] {
            assert!(tags.contains(t), "{tags}");
        }
    }

    #[test]
    fn an_archive_with_two_files_for_a_tweet_gets_no_third() {
        let env = Headless::new("pair");
        let id = "1721081463234609266";
        env.run(&[saved(id, "likes", CUT, Some("2023-11-05T09:00:00.000Z"))]);
        let first = env.files().remove(0);
        // The second file, as the old importer left it under the fuller slug.
        let works = env.dir.join("archive/readings/works");
        let twin = "alice-a-long-post-second-import-1721081463234609266.md";
        std::fs::write(
            works.join(twin),
            env.text(&first)
                .replace(&format!("> {CUT}"), &format!("> {WHOLE}")),
        )
        .unwrap();
        let cut_bytes = std::fs::read(works.join(&first)).unwrap();

        let r = env.run(&[saved(
            id,
            "bookmarks",
            WHOLE,
            Some("2023-11-05T09:00:00.000Z"),
        )]);
        assert_eq!(env.files().len(), 2, "{:?}", env.files());
        assert_eq!(r.duplicate_keys, [format!("x:{id}")]);
        assert!(env.text(twin).contains("tags: like, bookmark\n"));
        assert_eq!(std::fs::read(works.join(&first)).unwrap(), cut_bytes);
        assert_eq!(format!("{}.md", env.indexed(id).0), twin);
    }

    #[test]
    fn a_crlf_file_leaves_the_archive_and_the_index_as_they_were() {
        let env = Headless::new("crlf");
        let id = "1721081463234609266";
        env.run(&[saved(id, "likes", WHOLE, Some("2023-11-05T09:00:00.000Z"))]);
        let name = env.files().remove(0);
        let path = env.dir.join("archive/readings/works").join(&name);
        // The user's note and tag, in a file saved with CRLF line endings.
        let crlf = env
            .text(&name)
            .replace("tags: like\n", "tags: like, archives\n\nWorth rereading.\n")
            .replace('\n', "\r\n");
        std::fs::write(&path, &crlf).unwrap();
        let indexed = env.indexed(id);

        for text in [CUT, WHOLE, &format!("{WHOLE} And a postscript.")] {
            let r = env.run(&[saved(
                id,
                "bookmarks",
                text,
                Some("2023-11-05T09:00:00.000Z"),
            )]);
            assert_eq!(r.skipped.len(), 1, "{r:?}");
            assert_eq!(r.skipped[0].key, format!("x:{id}"));
            assert_eq!(env.files(), [name.clone()], "no second file");
            assert_eq!(env.text(&name), crlf);
            assert_eq!(env.indexed(id), indexed, "the index keeps what it had");
        }
    }

    impl Headless {
        /// The persistence step every importer shares, on a batch built by
        /// hand (what a canonical JSON import can hold).
        fn persist(
            &self,
            works: &[crate::models::Work],
            highlights: &[(crate::models::Highlight, String, Option<String>)],
        ) -> crate::import::tweet_upsert::TweetReport {
            let conn = scout_index::sqlite::open(&self.dir.join("index.db")).unwrap();
            scout_index::sqlite::init_schema(&conn).unwrap();
            crate::commands::import::persist_core_reported(
                &std::sync::Mutex::new(conn),
                self.dir.join("archive").to_str().unwrap(),
                crate::commands::import::PersistBatch {
                    source: "json",
                    works,
                    highlights,
                    archive_records: None,
                    deleted_ids: &std::collections::HashSet::new(),
                    raw_json: None,
                },
                &|_, _, _| {},
            )
            .unwrap()
            .1
        }
        fn index_rows(&self) -> (i64, i64) {
            let conn = rusqlite::Connection::open(self.dir.join("index.db")).unwrap();
            let count = |table: &str| {
                conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                    .unwrap()
            };
            (count("works"), count("highlights"))
        }
    }

    const TWEET: &str = "1721081463234609266";
    const DATE: Option<&str> = Some("2023-11-05T09:00:00.000Z");

    #[test]
    fn a_file_whose_only_block_is_not_the_tweet_keeps_its_bytes_and_its_index_entry() {
        let env = Headless::new("soleblock");
        env.run(&[saved(TWEET, "likes", WHOLE, DATE)]);
        let name = env.files().remove(0);
        let path = env.dir.join("archive/readings/works").join(&name);
        // The user removed the tweet's block and kept their own highlight.
        let edited = env.text(&name).replace(
            &format!("> {WHOLE}\n\nhighlighted_at: 2023-11-05 | tags: like\n"),
            "> My budget assumptions\n\ntags: mine\n",
        );
        assert!(edited.contains("My budget assumptions"));
        std::fs::write(&path, &edited).unwrap();
        let indexed = env.indexed(TWEET);

        let r = env.run(&[saved(TWEET, "bookmarks", WHOLE, DATE)]);
        assert_eq!(r.skipped.len(), 1, "{r:?}");
        assert_eq!(r.skipped[0].key, format!("x:{TWEET}"));
        assert!(r.skipped[0]
            .reason
            .contains("none of the file's highlights"));
        assert_eq!(env.files(), std::slice::from_ref(&name));
        assert_eq!(env.text(&name), edited);
        assert_eq!(env.indexed(TWEET), indexed);
    }

    #[test]
    fn an_incoming_error_page_reaches_neither_the_archive_nor_the_index() {
        let env = Headless::new("errorpage");
        let page = "JavaScript is not available.\n\nWe’ve detected that JavaScript is disabled in this browser. Please enable JavaScript or switch to a supported browser to continue using twitter.com. You can see a list of supported browsers in our Help Center.\n\nHelp Center";
        // As a new tweet: no file, no index row, the key in the log line.
        let r = env.run(&[saved(TWEET, "likes", page, DATE)]);
        assert_eq!(r.created, 0);
        assert!(r
            .log_message()
            .unwrap()
            .contains(&format!("x:{TWEET} (the incoming text is an error page")));
        assert!(env.files().is_empty());
        assert_eq!(env.index_rows(), (0, 0));

        // Over a real tweet: file and index exactly as they were.
        env.run(&[saved(TWEET, "likes", WHOLE, DATE)]);
        let name = env.files().remove(0);
        let (text, indexed) = (env.text(&name), env.indexed(TWEET));
        let r = env.run(&[saved(TWEET, "bookmarks", page, DATE)]);
        assert_eq!(r.skipped.len(), 1);
        assert_eq!(env.text(&name), text);
        assert_eq!(env.indexed(TWEET), indexed);

        // Prose that opens like an error message is a tweet, kept whole
        // against a shorter export of itself.
        let other = "1721081463234609999";
        let prose =
            "Something went wrong with our deployment. Here is the complete incident analysis.";
        env.run(&[saved(other, "likes", prose, DATE)]);
        let r = env.run(&[saved(other, "likes", "Something went", DATE)]);
        assert_eq!((r.updated, r.unchanged, r.skipped.len()), (0, 1, 0));
        assert_eq!(env.indexed(other).1, prose);
    }

    #[test]
    fn a_file_with_duplicate_identity_lines_or_a_bom_is_never_changed() {
        let env = Headless::new("identity");
        env.run(&[saved(TWEET, "likes", WHOLE, DATE)]);
        let name = env.files().remove(0);
        let path = env.dir.join("archive/readings/works").join(&name);
        let clean = env.text(&name);
        let indexed = env.indexed(TWEET);

        // A BOM: still this tweet's file, so no second one; left alone.
        let bom = format!("\u{feff}{clean}");
        std::fs::write(&path, &bom).unwrap();
        let r = env.run(&[saved(TWEET, "bookmarks", WHOLE, DATE)]);
        assert_eq!(r.skipped.len(), 1, "{r:?}");
        assert_eq!(env.files(), std::slice::from_ref(&name));
        assert_eq!(env.text(&name), bom);
        assert_eq!(env.indexed(TWEET), indexed);

        // Two identities in one frontmatter: nobody's file. It is reported
        // by path and untouched; the tweet gets a file of its own, which is
        // the one the index then names.
        let twice = clean.replace(
            "imported_at: ",
            "source_system: zotero\nsource_id: \"ABC\"\nimported_at: ",
        );
        std::fs::write(&path, &twice).unwrap();
        let r = env.run(&[saved(TWEET, "bookmarks", WHOLE, DATE)]);
        assert_eq!(r.unreadable, std::slice::from_ref(&path));
        assert!(r.log_message().unwrap().contains(path.to_str().unwrap()));
        assert_eq!(env.text(&name), twice);
        assert_eq!(r.created, 1);
        let files = env.files();
        assert_eq!(files.len(), 2);
        let new = files.iter().find(|f| **f != name).unwrap();
        assert!(env.text(new).contains("tags: bookmark\n"));
        // (scout-index keeps a row's first slug; the row is moved here.)
        assert_eq!(format!("{}.md", env.indexed(TWEET).0), *new);
    }

    #[test]
    fn two_works_with_one_key_in_a_batch_make_one_file_and_one_index_entry() {
        let env = Headless::new("batchkey");
        let input = |text: &str, saved_as: &str| crate::import::tweet_common::TweetInput {
            tweet_id: TWEET.into(),
            text: text.into(),
            author_handle: Some("alice".into()),
            saved_as: Some(saved_as.into()),
            ..Default::default()
        };
        let now = "2026-06-21T00:00:00Z";
        let (first, like, title) = make_records(&input(CUT, "likes"), now);
        let (mut second, mut bookmark, _) = make_records(&input(WHOLE, "bookmarks"), now);
        second.id = "json-work-2".into();
        second.slug = format!("another-slug-{TWEET}");
        bookmark.id = "json-h-2".into();
        bookmark.container_id = second.id.clone();

        let r = env.persist(
            &[first.clone(), second.clone()],
            &[(like, title.clone(), None), (bookmark, title.clone(), None)],
        );
        assert_eq!((r.created, r.updated), (1, 1));
        assert_eq!(env.files(), [format!("{}.md", first.slug)]);
        let text = env.text(&env.files()[0]);
        assert!(text.contains("and this is how it ends."), "{text}");
        assert!(text.contains("tags: like, bookmark\n"), "{text}");
        // One index entry names the file; the second work is reported.
        assert_eq!(env.index_rows(), (1, 1));
        assert_eq!(env.indexed(TWEET).0, first.slug);
        assert_eq!(r.index_slug_conflicts, [format!("x:{TWEET}")]);
    }

    #[test]
    fn a_refused_work_that_came_with_no_highlight_reaches_the_log() {
        let env = Headless::new("nohighlight");
        env.run(&[saved(TWEET, "likes", WHOLE, DATE)]);
        let name = env.files().remove(0);
        let (text, indexed) = (env.text(&name), env.indexed(TWEET));

        let (mut work, _, _) = make_records(
            &crate::import::tweet_common::TweetInput {
                tweet_id: TWEET.into(),
                text: "a title the index must not take".into(),
                ..Default::default()
            },
            "2026-06-22T00:00:00Z",
        );
        work.title = "a title the index must not take".into();
        let r = env.persist(&[work], &[]);
        assert_eq!(r.skipped.len(), 1, "{r:?}");
        assert_eq!(r.skipped[0].key, format!("x:{TWEET}"));
        assert!(r.skipped[0].reason.contains("gave no highlight"));
        assert!(r
            .log_message()
            .unwrap()
            .contains(&format!("x:{TWEET} (the import gave no highlight")));
        assert_eq!(env.text(&name), text);
        assert_eq!(env.indexed(TWEET), indexed);
        let conn = rusqlite::Connection::open(env.dir.join("index.db")).unwrap();
        let title: String = conn
            .query_row(
                "SELECT title FROM works WHERE id = ?1",
                [format!("x-w-{TWEET}")],
                |r| r.get(0),
            )
            .unwrap();
        assert!(title.starts_with("A long post"), "{title}");
    }

    #[test]
    fn an_index_row_that_cannot_take_the_files_name_is_left_and_reported() {
        let env = Headless::new("slugconflict");
        let id = "1721081463234609266";
        env.run(&[saved(id, "likes", CUT, Some("2023-11-05T09:00:00.000Z"))]);
        let first = env.files().remove(0);
        // The fuller twin an older import left, whose name a stale index row
        // for another work holds.
        let works = env.dir.join("archive/readings/works");
        let twin = "alice-a-long-post-second-import-1721081463234609266";
        std::fs::write(
            works.join(format!("{twin}.md")),
            env.text(&first)
                .replace(&format!("> {CUT}"), &format!("> {WHOLE}")),
        )
        .unwrap();
        {
            let conn = rusqlite::Connection::open(env.dir.join("index.db")).unwrap();
            conn.execute(
                "INSERT INTO works (id, slug, title, work_type, source_system, imported_at, updated_at)
                 VALUES ('legacy-row', ?1, 'Stale', 'tweet', 'x', '2026-01-01', '2026-01-01')",
                [twin],
            )
            .unwrap();
        }
        let indexed = env.indexed(id);
        assert_eq!(format!("{}.md", indexed.0), first);

        let r = env.run(&[saved(
            id,
            "bookmarks",
            WHOLE,
            Some("2023-11-05T09:00:00.000Z"),
        )]);
        // The archive is updated in the fuller file…
        assert!(env
            .text(&format!("{twin}.md"))
            .contains("tags: like, bookmark\n"));
        // …the index is exactly as it was, and the key is reported.
        assert_eq!(r.index_slug_conflicts, [format!("x:{id}")]);
        assert!(r.log_message().unwrap().contains("not in the index"));
        assert_eq!(env.indexed(id), indexed);
        let conn = rusqlite::Connection::open(env.dir.join("index.db")).unwrap();
        let owner: String = conn
            .query_row("SELECT id FROM works WHERE slug = ?1", [twin], |r| r.get(0))
            .unwrap();
        assert_eq!(owner, "legacy-row");
    }

    #[test]
    fn the_headless_import_refuses_while_the_app_holds_its_lock() {
        let env = Headless::new("lock");
        let saved_path = env.dir.join("saved.jsonl");
        std::fs::write(
            &saved_path,
            saved("1721081463234609266", "likes", WHOLE, None),
        )
        .unwrap();
        let lock = env.dir.join("app/highlight-scout.lock");
        let run = || {
            crate::import_x_locked(
                saved_path.to_str().unwrap(),
                env.dir.join("archive").to_str().unwrap(),
                &env.dir.join("index.db"),
                &lock,
            )
        };

        let app = crate::app_lock::AppLock::acquire(&lock).unwrap();
        assert!(matches!(run(), Err(crate::HeadlessError::Refused(_))));
        assert!(!env.dir.join("archive").exists(), "nothing written");
        drop(app);

        assert!(run().is_ok());
        assert_eq!(env.files().len(), 1);
        assert!(
            crate::app_lock::AppLock::acquire(&lock).is_ok(),
            "the lock is released after the import"
        );
    }

    #[test]
    fn skips_blank_and_malformed_lines() {
        let body = "\n{not json}\n{\"tweet_id\":\"444\",\"text\":\"ok\"}\n";
        let path = write_tmp("hs-x-test3.jsonl", body);
        let (_w, hls) = import(&path).unwrap();
        assert_eq!(hls.len(), 1);
        assert_eq!(hls[0].0.id, "x-444");
        let _ = std::fs::remove_file(&path);
    }

    // In a file of its own for length; `include!` because a `#[path]` in
    // an inline module resolves under a directory that does not exist.
    mod index_ownership {
        include!("x_index_ownership_tests.rs");
    }
}
