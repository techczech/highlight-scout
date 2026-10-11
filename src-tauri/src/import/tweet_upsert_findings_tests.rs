//! One test per finding of the second, third and fourth reviews of the tweet upsert,
//! each reproducing the finding's scenario (the third review's index
//! findings are in `x_index_ownership_tests.rs`). The first round's are in
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

#[test]
fn an_anchored_unclosed_collection_makes_the_file_nobodys() {
    // r3 P1: a manually edited work whose frontmatter leaves `note: &n [`
    // open above `source_system: x`, over a block reading `Budget…`.
    let env = Env::new("anchored");
    env.seed(&tweet("Budget…", "likes"), DAY1);
    let file = env.only_file();
    let edited = env.edit(|t| {
        let title = t.lines().nth(1).unwrap();
        t.replace(title, "title: A manually edited work")
            .replace("source_system: x\n", "note: &n [\nsource_system: x\n")
    });
    assert!(edited.contains("> Budget…\n"), "{edited}");
    let before = age(&file);

    let incoming = "Budget assumptions have changed";
    let (_, w) = env.import(&tweet(incoming, "likes"), DAY2);
    assert_eq!(fs::read_to_string(&file).unwrap(), edited);
    assert_eq!(mtime(&file), before);
    assert_eq!(w.tweets.unreadable, std::slice::from_ref(&file));
    assert_eq!((w.tweets.created, w.tweets.updated), (1, 0));
    assert!(Candidate::load(&file, &format!("x:{ID}"), incoming).is_err());
}

#[test]
fn a_file_with_an_unquoted_title_that_opens_a_quote_is_still_the_tweets() {
    // The shape the app writes: `title: @someone said: "x`, never closed.
    let env = Env::new("openquote");
    env.seed(&tweet(SHORT, "likes"), DAY1);
    let file = env.only_file();
    let edited = env.edit(|t| {
        let title = t.lines().nth(1).unwrap();
        t.replace(title, "title: @someone said: \"x")
    });
    let frontmatter = edited.split("---\n").nth(1).unwrap();
    assert!(yaml_rust2::YamlLoader::load_from_str(frontmatter).is_err());

    let (work, w) = env.import(&tweet(FULL, "bookmarks"), DAY2);
    assert_eq!((w.tweets.created, w.tweets.updated), (0, 1));
    assert!(w.tweets.unreadable.is_empty());
    assert_eq!(env.only_file(), file);
    let text = env.text();
    assert!(text.contains("title: @someone said: \"x\n"), "{text}");
    assert!(text.contains(&format!("> {FULL}\n")), "{text}");
    assert_eq!(work.title, "@someone said: \"x");
}

#[test]
fn a_title_that_runs_on_to_a_second_line_makes_the_file_nobodys() {
    let (env, file, w) = reimport_after("multilinetitle", |t| {
        let title = t.lines().nth(1).unwrap();
        t.replace(title, "title: @someone said: \"x\n  and more\"")
    });
    assert_eq!(w.tweets.unreadable, std::slice::from_ref(&file));
    assert_eq!((w.tweets.created, w.tweets.updated), (1, 0));
    assert_eq!(env.names().len(), 2);
    assert!(Candidate::load(&file, &format!("x:{ID}"), FULL).is_err());
}

#[test]
fn a_tweet_work_without_an_id_is_refused_and_no_file_is_touched() {
    // r3 P1: `source_id` null or empty, the slug an existing tweet file's
    // name, an unrelated highlight. It must not reach the generic writer.
    let env = Env::new("noid");
    env.import(&tweet(FULL, "likes"), DAY1);
    let file = env.only_file();
    let bytes = fs::read(&file).unwrap();
    let before = age(&file);

    for missing in [None, Some(String::new())] {
        let (mut work, mut h, _) = make_records(&tweet("Something else entirely", "likes"), DAY2);
        work.source_id = missing;
        work.slug = file.file_stem().unwrap().to_string_lossy().into_owned();
        work.id = "json-w-1".into();
        h.container_id = work.id.clone();
        assert!(!is_tweet(&work));
        let mut works = vec![work];
        let mut by: HashMap<String, Vec<&Highlight>> = HashMap::new();
        by.insert(works[0].id.clone(), vec![&h]);
        let w = write_archive(env.dir.to_str().unwrap(), &mut works, &by).unwrap();

        assert_eq!((w.summary.written, w.summary.unchanged), (0, 0));
        assert_eq!(w.tweets.skipped.len(), 1, "{:?}", w.tweets);
        assert_eq!(w.tweets.skipped[0].reason, "tweet without an id");
        assert!(w.skipped_works.contains("json-w-1"));
        assert!(w.highlights.is_empty());
        assert_eq!(fs::read(&file).unwrap(), bytes);
        assert_eq!(mtime(&file), before);
        assert_eq!(env.names().len(), 1);
    }
}

#[test]
fn a_changed_link_after_the_same_words_does_not_replace_the_text() {
    // r3 P2.
    let env = Env::new("changedlink");
    let file = env.seed(&tweet("Read this… https://t.co/original", "likes"), DAY1);
    let bytes = fs::read(&file).unwrap();
    let before = age(&file);

    let (_, w) = env.import(&tweet("Read this https://t.co/other", "likes"), DAY2);
    assert_eq!((w.tweets.created, w.tweets.updated), (0, 0));
    assert_eq!(w.tweets.skipped.len(), 1, "{:?}", w.tweets);
    assert_eq!(fs::read(&file).unwrap(), bytes);
    assert_eq!(mtime(&file), before);
    assert_eq!(env.names().len(), 1);
}

#[test]
fn a_markdown_link_after_the_same_words_does_not_replace_the_text() {
    // r4 P2: Reader writes an external link whose visible text is its URL as
    // `[url](url)`; the brackets are not more of the tweet.
    let env = Env::new("markdownlink");
    let file = env.seed(&tweet("Read this… https://t.co/original", "likes"), DAY1);
    let bytes = fs::read(&file).unwrap();
    let before = age(&file);

    for link_only in [
        "Read this [https://example.org/report](https://example.org/report)",
        "Read this <https://example.org>",
        "Read this [example.org/report](https://example.org/report)",
        // r5: a destination with parentheses in it.
        WIKI_LINK,
        "Read this @someone #topic",
    ] {
        let (_, w) = env.import(&tweet(link_only, "bookmarks"), DAY2);
        assert_left_alone(&w, "none of the file's highlights");
        assert_eq!(fs::read(&file).unwrap(), bytes, "{link_only}");
        assert_eq!(mtime(&file), before, "{link_only}");
        assert_eq!(env.names().len(), 1, "{link_only}");
    }

    // Words after the stem, a labelled link among them, are the tweet.
    let whole = "Read this report by [the authors](https://example.org) today";
    let (_, w) = env.import(&tweet(whole, "likes"), DAY3);
    assert_eq!(
        (w.tweets.created, w.tweets.updated),
        (0, 1),
        "{:?}",
        w.tweets
    );
    assert!(w.tweets.skipped.is_empty());
    assert_eq!(env.only_file(), file);
    let text = env.text();
    assert!(text.contains(&format!("> {whole}\n")), "{text}");
    assert!(
        !text.contains("> Read this… https://t.co/original"),
        "{text}"
    );
    assert_eq!(w.highlights[&hid()].text, whole);
}

/// A work as a canonical JSON import can give it: not from X, any slug.
fn csv_record(n: usize, slug: &str) -> (Work, Highlight) {
    let (mut work, mut h, _) = make_records(&tweet(FULL, "likes"), DAY1);
    work.id = format!("csv-w-{n}");
    work.source_system = "csv".into();
    work.source_id = Some(format!("row-{n}"));
    work.kind = "book".into();
    work.slug = slug.into();
    h.id = format!("csv-h-{n}");
    h.container_id = work.id.clone();
    (work, h)
}

/// Every file under `dir`, relative to it, sorted.
fn files_under(dir: &Path) -> Vec<String> {
    let mut all = Vec::new();
    let mut dirs = vec![dir.to_path_buf()];
    while let Some(d) = dirs.pop() {
        for e in fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                dirs.push(p);
            } else {
                all.push(p.strip_prefix(dir).unwrap().display().to_string());
            }
        }
    }
    all.sort();
    all
}

#[test]
fn a_work_of_another_source_with_an_unsafe_slug_is_not_written_anywhere() {
    // r4 P1: `slug: "../victim"` on a csv work reached the generic writer,
    // which rendered over `readings/victim.md`.
    let outside = std::env::temp_dir().join(format!("hs-abs-escape-{}", std::process::id()));
    let outside_file = outside.with_extension("md");
    let _ = fs::remove_file(&outside_file);
    let absolute = outside.to_str().unwrap().to_string();

    // Once on the path with no tweet in the batch, once with one.
    for with_tweet in [false, true] {
        let env = Env::new(if with_tweet { "unsafe-mixed" } else { "unsafe" });
        let victim = env.dir.join("readings/victim.md");
        fs::write(&victim, "the user's own file").unwrap();
        // So that `sub/escaped.md` could have been written.
        fs::create_dir(env.works().join("sub")).unwrap();

        let hostile = ["../victim", absolute.as_str(), "sub/escaped", ".hidden", ""];
        let mut records: Vec<(Work, Highlight)> = hostile
            .iter()
            .enumerate()
            .map(|(n, slug)| csv_record(n, slug))
            .collect();
        records.insert(2, csv_record(100, "an-author-a-book-row-100"));
        records.push(csv_record(101, "an-author-another-book-row-101"));
        if with_tweet {
            let (work, h, _) = make_records(&tweet(FULL, "likes"), DAY1);
            records.push((work, h));
        }
        let mut works: Vec<Work> = records.iter().map(|(w, _)| w.clone()).collect();
        let mut by: HashMap<String, Vec<&Highlight>> = HashMap::new();
        for (w, h) in &records {
            by.insert(w.id.clone(), vec![h]);
        }
        let w = write_archive(env.dir.to_str().unwrap(), &mut works, &by).unwrap();

        // The others in the batch are written as ever.
        let mut expected = vec![
            "readings/victim.md".to_string(),
            "readings/works/an-author-a-book-row-100.md".to_string(),
            "readings/works/an-author-another-book-row-101.md".to_string(),
        ];
        if with_tweet {
            expected.push(format!("readings/works/{}.md", works.last().unwrap().slug));
            expected.sort();
        }
        assert_eq!(files_under(&env.dir), expected);
        assert_eq!(w.summary.written, expected.len() - 1);
        assert_eq!(w.tweets.created, usize::from(with_tweet));
        assert_eq!(fs::read_to_string(&victim).unwrap(), "the user's own file");
        assert!(!outside_file.exists());
        assert!(
            fs::read_to_string(env.works().join("an-author-a-book-row-100.md"))
                .unwrap()
                .contains("Apache 2.0 licence")
        );

        // Each refused work is reported by its source key, and kept from the
        // index by its id.
        let skipped: Vec<(String, &str)> = w
            .tweets
            .skipped
            .iter()
            .map(|s| (s.key.clone(), s.reason.as_str()))
            .collect();
        let want: Vec<(String, &str)> = (0..hostile.len())
            .map(|n| (format!("csv:row-{n} (work csv-w-{n})"), "unsafe file name"))
            .collect();
        assert_eq!(skipped, want);
        let ids: HashSet<String> = (0..hostile.len()).map(|n| format!("csv-w-{n}")).collect();
        assert_eq!(w.skipped_works, ids);
        assert!(w
            .tweets
            .log_message()
            .unwrap()
            .contains("csv:row-0 (work csv-w-0) (unsafe file name)"));
        // The slugs are as they came; nothing was renamed into safety.
        assert_eq!(works[0].slug, "../victim");
    }
}
