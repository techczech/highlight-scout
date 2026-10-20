// Third and fourth reviews of the tweet upsert: what the persistence step
// leaves alone in the index. A row whose id the batch reuses but which belongs to
// another key or work is not changed, and a tweet with no id reaches
// neither the archive nor the index. These use the scratch archive and
// index of the `x.rs` tests.

use super::*;
use crate::import::tweet_common::TweetInput;

const OTHER: &str = "1721081463234609999";
const NOW: &str = "2026-06-22T00:00:00Z";

fn input(id: &str, text: &str) -> TweetInput {
    TweetInput {
        tweet_id: id.into(),
        text: text.into(),
        author_handle: Some("alice".into()),
        saved_as: Some("bookmarks".into()),
        ..Default::default()
    }
}

/// Every column of every row of both index tables, in id order.
fn index_dump(env: &Headless) -> Vec<String> {
    let conn = rusqlite::Connection::open(env.dir.join("index.db")).unwrap();
    let mut out = Vec::new();
    for sql in [
        "SELECT id, slug, title, author, source_system, source_id, url, updated_at FROM works ORDER BY id",
        "SELECT id, work_id, text, note, highlighted_at, tags, NULL, NULL FROM highlights ORDER BY id",
    ] {
        let mut stmt = conn.prepare(sql).unwrap();
        let rows = stmt
            .query_map([], |r| {
                Ok((0..8)
                    .map(|i| r.get::<_, Option<String>>(i).map(|v| v.unwrap_or_default()))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(" | "))
            })
            .unwrap();
        out.extend(rows.map(|r| r.unwrap()));
    }
    out
}

#[test]
fn a_work_id_the_index_holds_for_another_key_leaves_the_index_untouched() {
    // A (x:TWEET) is indexed as x-w-TWEET. B (x:OTHER) has a file but no
    // index row, and arrives carrying A's work id.
    let env = Headless::new("workowner");
    env.run(&[saved(TWEET, "likes", WHOLE, DATE)]);
    let elsewhere = Headless::new("workowner-b");
    elsewhere.run(&[saved(OTHER, "bookmarks", "B, another tweet", DATE)]);
    let b_name = elsewhere.files().remove(0);
    std::fs::write(
        env.dir.join("archive/readings/works").join(&b_name),
        elsewhere.text(&b_name),
    )
    .unwrap();
    let before = index_dump(&env);
    let (a_slug, _, _) = env.indexed(TWEET);

    let (mut work, mut h, title) = make_records(&input(OTHER, "B, another tweet"), NOW);
    work.id = format!("x-w-{TWEET}");
    h.container_id = work.id.clone();
    let r = env.persist(&[work], &[(h, title, None)]);

    // The archive lookup found B's file by B's key and had nothing to add.
    assert_eq!((r.created, r.updated, r.unchanged), (0, 0, 1), "{r:?}");
    // A's row still names A's file under A's key; B's highlight is not in.
    assert_eq!(index_dump(&env), before);
    assert_eq!(env.indexed(TWEET).0, a_slug);
    assert_eq!(r.index_ownership_conflicts.len(), 1, "{r:?}");
    let logged = r.log_message().unwrap();
    assert!(
        logged.contains(&format!(
            "x:{OTHER}: the index entry x-w-{TWEET} belongs to x:{TWEET}"
        )),
        "{logged}"
    );
}

#[test]
fn a_highlight_id_the_index_holds_for_another_work_is_not_re_pointed() {
    // A is indexed with highlight x-TWEET; its file is now CRLF, so A is
    // skipped. B is new and accepted, and its highlight carries A's id.
    let env = Headless::new("highlightowner");
    env.run(&[saved(TWEET, "likes", WHOLE, DATE)]);
    let a_name = env.files().remove(0);
    let a_path = env.dir.join("archive/readings/works").join(&a_name);
    let crlf = env.text(&a_name).replace('\n', "\r\n");
    std::fs::write(&a_path, &crlf).unwrap();
    let before = index_dump(&env);

    let (a_work, a_h, a_title) = make_records(&input(TWEET, WHOLE), NOW);
    let (b_work, mut b_h, b_title) = make_records(&input(OTHER, "B, another tweet"), NOW);
    b_h.id = format!("x-{TWEET}");
    let r = env.persist(
        &[a_work, b_work.clone()],
        &[(a_h, a_title, None), (b_h, b_title, None)],
    );

    assert_eq!((r.created, r.skipped.len()), (1, 1), "{r:?}");
    assert_eq!(r.skipped[0].key, format!("x:{TWEET}"));
    assert_eq!(env.text(&a_name), crlf);
    // B's work row is new; every row that was there is exactly as it was,
    // and A's highlight is still A's.
    let after = index_dump(&env);
    assert_eq!(env.index_rows(), (2, 1));
    for row in &before {
        assert!(after.contains(row), "changed: {row}\n{after:#?}");
    }
    assert_eq!(env.indexed(TWEET).1, WHOLE);
    let conn = rusqlite::Connection::open(env.dir.join("index.db")).unwrap();
    let owner: String = conn
        .query_row(
            "SELECT work_id FROM highlights WHERE id = ?1",
            [format!("x-{TWEET}")],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(owner, format!("x-w-{TWEET}"));
    assert_eq!(r.index_ownership_conflicts.len(), 1, "{r:?}");
    let logged = r.log_message().unwrap();
    assert!(
        logged.contains(&format!(
            "x:{OTHER}: the index highlight x-{TWEET} belongs to the work x-w-{TWEET}"
        )),
        "{logged}"
    );
}

#[test]
fn a_tweet_work_without_an_id_reaches_neither_the_archive_nor_the_index() {
    // A work from X with no id, named like an existing tweet's file, with
    // an unrelated highlight: the generic writer would render over the file.
    let env = Headless::new("noid");
    env.run(&[saved(TWEET, "likes", WHOLE, DATE)]);
    let name = env.files().remove(0);
    let text = env.text(&name);
    let before = index_dump(&env);

    for (n, missing) in [None, Some(String::new())].into_iter().enumerate() {
        let (mut work, mut h, title) = make_records(&input(TWEET, "Something else entirely"), NOW);
        work.id = format!("json-w-{n}");
        work.source_id = missing;
        work.slug = name.trim_end_matches(".md").to_string();
        h.id = format!("json-h-{n}");
        h.container_id = work.id.clone();
        let r = env.persist(&[work], &[(h, title, None)]);

        assert_eq!((r.created, r.updated, r.unchanged), (0, 0, 0), "{r:?}");
        assert_eq!(r.skipped.len(), 1, "{r:?}");
        assert_eq!(r.skipped[0].reason, "tweet without an id");
        assert!(r
            .log_message()
            .unwrap()
            .contains(&format!("x: (work json-w-{n}) (tweet without an id)")));
        assert_eq!(env.files(), std::slice::from_ref(&name));
        assert_eq!(env.text(&name), text);
        assert_eq!(index_dump(&env), before);
    }
}

#[test]
fn a_work_of_another_source_with_an_unsafe_slug_reaches_neither_the_archive_nor_the_index() {
    // r4 P1: a canonical JSON import holding csv works whose slugs point out
    // of the works directory, beside an ordinary one.
    let env = Headless::new("unsafeslug");
    let readings = env.dir.join("archive/readings");
    std::fs::create_dir_all(readings.join("works/sub")).unwrap();
    let victim = readings.join("victim.md");
    std::fs::write(&victim, "the user's own file").unwrap();
    let outside = std::env::temp_dir().join(format!("hs-abs-escape-index-{}", std::process::id()));
    let outside_file = outside.with_extension("md");
    let _ = std::fs::remove_file(&outside_file);

    let hostile = ["../victim", outside.to_str().unwrap(), "sub/escaped"];
    let csv = |n: usize, slug: &str| {
        let (mut work, mut h, title) = make_records(&input(OTHER, "A line from a book"), NOW);
        work.id = format!("csv-w-{n}");
        work.source_system = "csv".into();
        work.source_id = Some(format!("row-{n}"));
        work.kind = "book".into();
        work.slug = slug.into();
        h.id = format!("csv-h-{n}");
        h.container_id = work.id.clone();
        (work, (h, title, None))
    };
    let mut records: Vec<_> = hostile
        .iter()
        .enumerate()
        .map(|(n, slug)| csv(n, slug))
        .collect();
    records.insert(1, csv(100, "an-author-a-book-row-100"));
    let (works, highlights): (Vec<_>, Vec<_>) = records.into_iter().unzip();
    let r = env.persist(&works, &highlights);

    // Nothing was written for them, outside the works directory or in it.
    assert_eq!(env.files(), ["an-author-a-book-row-100.md", "sub"]);
    assert_eq!(
        std::fs::read_dir(readings.join("works/sub"))
            .unwrap()
            .count(),
        0
    );
    assert_eq!(
        std::fs::read_to_string(&victim).unwrap(),
        "the user's own file"
    );
    assert!(!outside_file.exists());
    let mut beside: Vec<String> = std::fs::read_dir(&readings)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    beside.sort();
    assert_eq!(beside, ["assets", "fulltext", "victim.md", "works"]);

    // The index holds the ordinary work and its highlight, and no other.
    let dump = index_dump(&env);
    assert_eq!(env.index_rows(), (1, 1), "{dump:#?}");
    assert!(
        dump[0].starts_with("csv-w-100 | an-author-a-book-row-100 | "),
        "{dump:#?}"
    );
    assert!(
        dump[1].starts_with("csv-h-100 | csv-w-100 | A line from a book"),
        "{dump:#?}"
    );

    // Each is logged by its source key with the reason.
    assert_eq!(r.skipped.len(), hostile.len(), "{r:?}");
    let logged = r.log_message().unwrap();
    for n in 0..hostile.len() {
        assert_eq!(r.skipped[n].key, format!("csv:row-{n} (work csv-w-{n})"));
        assert_eq!(r.skipped[n].reason, "unsafe file name");
        assert!(
            logged.contains(&format!("csv:row-{n} (work csv-w-{n}) (unsafe file name)")),
            "{logged}"
        );
    }
}
