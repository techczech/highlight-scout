//! One test per finding of the second review of the tweet upsert, each
//! reproducing the finding's scenario. The first round's are in
//! `tweet_upsert_tests.rs`, whose scratch archive and helpers these use.

use super::*;

/// Import a tweet, respell its file's frontmatter through `edit`, and import
/// it again with a bookmark: the file must come through byte for byte.
fn reimport_after(name: &str, edit: impl Fn(&str) -> String) -> (Env, PathBuf, Written) {
    let env = Env::new(name);
    env.import(&tweet(FULL, "likes"), DAY1);
    let file = env.only_file();
    let edited = env.edit(edit);
    let before = age(&file);
    let (_, w) = env.import(&tweet(FULL, "bookmarks"), DAY2);
    assert_eq!(fs::read_to_string(&file).unwrap(), edited);
    assert_eq!(mtime(&file), before);
    (env, file, w)
}

#[test]
fn duplicate_identity_lines_make_the_file_nobodys() {
    // source_system: x / source_id / source_system: zotero / source_id: ABC
    let (env, file, w) = reimport_after("dupkeys", |t| {
        t.replace(
            "imported_at: ",
            "source_system: zotero\nsource_id: \"ABC\"\nimported_at: ",
        )
    });
    assert_eq!(w.tweets.unreadable, std::slice::from_ref(&file));
    assert!(w.tweets.log_message().unwrap().contains(&format!(
        "unreadable frontmatter and were not looked at: {}",
        file.display()
    )));
    // It is not this tweet's file: the tweet gets its own, under a new name.
    assert_eq!((w.tweets.created, w.tweets.updated), (1, 0));
    assert_eq!(env.names().len(), 2);
    assert!(Candidate::load(&file, &format!("x:{ID}"), FULL).is_err());
}

#[test]
fn a_key_nested_in_an_unclosed_flow_mapping_makes_the_file_nobodys() {
    // The file is a zotero work whose (invalid) frontmatter holds
    // `source_system: x` at column 0 inside `source_data: {`.
    let (env, file, w) = reimport_after("nested", |t| {
        let title = t.lines().nth(1).unwrap();
        t.replace(title, "title: @someone").replace(
            "source_system: x\n",
            "source_data: {\nsource_system: x\n}\nsource_system: zotero\n",
        )
    });
    assert_eq!(w.tweets.unreadable, std::slice::from_ref(&file));
    assert_eq!((w.tweets.created, w.tweets.updated), (1, 0));
    assert_eq!(env.names().len(), 2);
    assert!(Candidate::load(&file, &format!("x:{ID}"), FULL).is_err());
}

#[test]
fn a_file_saved_with_a_bom_is_found_and_no_second_file_is_made() {
    let (env, _, w) = reimport_after("bom", |t| format!("\u{feff}{t}"));
    assert_left_alone(&w, "not laid out");
    assert!(w.tweets.unreadable.is_empty());
    assert_eq!(env.names().len(), 1, "no duplicate");
}

#[test]
fn an_unrelated_sole_block_is_not_taken_for_the_tweet() {
    // The user removed the tweet's block and kept a highlight of their own.
    let env = Env::new("soleblock");
    env.import(&tweet(FULL, "likes"), DAY1);
    let file = env.only_file();
    let edited = env.edit(|t| {
        t.replace(
            &format!("> {FULL}\n\nhighlighted_at: 2025-03-26 | tags: like\n"),
            "> My budget assumptions\n\ntags: mine\n",
        )
    });
    let before = age(&file);
    let (_, w) = env.import(&tweet(FULL, "bookmarks"), DAY2);
    assert_left_alone(&w, "none of the file's highlights");
    assert_eq!(env.text(), edited, "no tag, no date added");
    assert_eq!(mtime(&file), before);
}

#[test]
fn a_trailing_link_without_an_ellipsis_is_not_a_cut_off_mark() {
    let env = Env::new("trailinglink");
    env.import(&tweet("Read this https://t.co/important", "likes"), DAY1);
    let file = env.only_file();
    let bytes = fs::read(&file).unwrap();
    let before = age(&file);
    let (_, w) = env.import(&tweet("Read this updated explanation", "likes"), DAY2);
    assert_left_alone(&w, "none of the file's highlights");
    assert_eq!(fs::read(&file).unwrap(), bytes);
    assert_eq!(mtime(&file), before);

    // The same text again is still the same tweet.
    let (_, w) = env.import(&tweet("Read this https://t.co/important", "likes"), DAY3);
    assert_eq!(w.tweets.unchanged, 1);
    assert_eq!(fs::read(&file).unwrap(), bytes);
}

#[test]
fn two_works_with_one_key_in_a_batch_share_one_file() {
    let env = Env::new("batchkey");
    let (first, like, _) = make_records(&tweet(SHORT, "likes"), DAY1);
    let (mut second, mut bookmark, _) = make_records(&tweet(FULL, "bookmarks"), DAY1);
    // A canonical JSON import: another work id, the same source_id.
    second.id = "json-work-2".into();
    bookmark.id = "json-h-2".into();
    bookmark.container_id = second.id.clone();
    second.slug = format!("the-slug-fuller-text-would-give-{ID}");
    let mut works = vec![first, second];
    let mut by: HashMap<String, Vec<&Highlight>> = HashMap::new();
    by.insert(works[0].id.clone(), vec![&like]);
    by.insert(works[1].id.clone(), vec![&bookmark]);
    let w = write_archive(env.dir.to_str().unwrap(), &mut works, &by).unwrap();

    assert_eq!((w.tweets.created, w.tweets.updated), (1, 1));
    assert_eq!(env.names(), [format!("{}.md", works[0].slug)]);
    assert_eq!(works[1].slug, works[0].slug, "both name the one file");
    let text = env.text();
    assert_eq!(text.matches("> Voice Chat").count(), 1, "{text}");
    assert!(text.contains("Apache 2.0 licence"), "{text}");
    assert!(text.contains("tags: like, bookmark\n"), "{text}");
}

#[test]
fn a_hostile_slug_creates_nothing_anywhere() {
    let env = Env::new("hostile");
    fs::create_dir_all(env.dir.join("readings/fulltext")).unwrap();
    for slug in [
        "../../escaped",
        "../fulltext/escaped",
        "sub/escaped",
        "sub\\escaped",
        "..",
        ".hidden",
        "a..b",
        "nul\0byte",
        "/tmp/hs-absolute-escape",
        "",
    ] {
        let (mut work, h, _) = make_records(&tweet(FULL, "likes"), DAY1);
        work.slug = slug.into();
        let mut works = vec![work];
        let mut by: HashMap<String, Vec<&Highlight>> = HashMap::new();
        by.insert(works[0].id.clone(), vec![&h]);
        let w = write_archive(env.dir.to_str().unwrap(), &mut works, &by).unwrap();
        assert_left_alone(&w, "not a plain file name");
        assert!(env.names().is_empty(), "{slug:?}");
        assert_eq!(works[0].slug, slug);
    }
    // Nothing beside the works directory either.
    let mut all = Vec::new();
    let mut dirs = vec![env.dir.clone()];
    while let Some(d) = dirs.pop() {
        for e in fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                dirs.push(p);
            } else {
                all.push(p);
            }
        }
    }
    assert!(all.is_empty(), "{all:?}");
    assert!(!Path::new("/tmp/hs-absolute-escape.md").exists());
    assert_eq!(
        file_name("alice-a-post-123").as_deref(),
        Some("alice-a-post-123.md")
    );
}

#[test]
fn a_tweet_work_with_no_highlight_and_a_file_already_is_reported() {
    let env = Env::new("nohighlight");
    env.import(&tweet(FULL, "likes"), DAY1);
    let file = env.only_file();
    let bytes = fs::read(&file).unwrap();
    let before = age(&file);

    let (work, _, _) = make_records(&tweet(FULL, "likes"), DAY2);
    let mut works = vec![work];
    let w = write_archive(env.dir.to_str().unwrap(), &mut works, &HashMap::new()).unwrap();
    assert_left_alone(&w, "gave no highlight");
    assert_eq!(fs::read(&file).unwrap(), bytes);
    assert_eq!(mtime(&file), before);
}
