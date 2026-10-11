//! Tests for `tweet_upsert`, in a scratch archive under the temp directory.

use super::*;
use crate::import::tweet_common::{make_records, TweetInput};
use std::time::{Duration, SystemTime};

const ID: &str = "1904944923159445914";
const SHORT: &str = "Voice Chat + Video Chat! Just in Qwen Chat. We open-source the model behind all this, under the… https://t.co/LHQOQrl9Ha";
const FULL: &str = "Voice Chat + Video Chat! Just in Qwen Chat. We open-source the model behind all this, under the Apache 2.0 licence, and here is the rest of a long post.";
/// r5: a Markdown link whose destination holds parentheses.
const WIKI_LINK: &str = "Read this [en.wikipedia.org/wiki/Function…](https://en.wikipedia.org/wiki/Function_(mathematics))";
const DAY1: &str = "2026-06-20T00:00:00+00:00";
const DAY2: &str = "2026-06-21T00:00:00+00:00";
const DAY3: &str = "2026-06-22T00:00:00+00:00";

struct Env {
    dir: PathBuf,
}

impl Env {
    fn new(name: &str) -> Env {
        let dir =
            std::env::temp_dir().join(format!("hs-tweet-upsert-{}-{}", name, std::process::id()));
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
    fn text(&self) -> String {
        fs::read_to_string(self.only_file()).unwrap()
    }
    /// Rewrite the only file through `edit`, and return what it now holds.
    fn edit(&self, edit: impl Fn(&str) -> String) -> String {
        let file = self.only_file();
        let before = fs::read_to_string(&file).unwrap();
        let after = edit(&before);
        assert_ne!(before, after, "the edit must change the file");
        fs::write(&file, &after).unwrap();
        after
    }
    /// Put a tweet's file in the archive the way the plain writer does,
    /// whatever its text (how files from before this importer got there).
    fn seed(&self, t: &TweetInput, now: &str) -> PathBuf {
        let (work, h, _) = make_records(t, now);
        let mut by: HashMap<String, Vec<&Highlight>> = HashMap::new();
        by.insert(work.id.clone(), vec![&h]);
        archive_meta::write_archive(self.dir.to_str().unwrap(), std::slice::from_ref(&work), &by)
            .unwrap();
        self.works().join(format!("{}.md", work.slug))
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

fn hid() -> String {
    format!("x-{ID}")
}

fn wid() -> String {
    format!("x-w-{ID}")
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

const NO_SCRIPT_PAGE: &str = "JavaScript is not available.\n\nWe’ve detected that JavaScript is disabled in this browser. Please enable JavaScript or switch to a supported browser to continue using twitter.com. You can see a list of supported browsers in our Help Center.\n\nHelp Center";

/// The import left the archive and (through `Written`) the index alone.
fn assert_left_alone(w: &Written, reason_has: &str) {
    assert_eq!(
        (w.tweets.created, w.tweets.updated, w.tweets.unchanged),
        (0, 0, 0)
    );
    assert_eq!(w.tweets.skipped.len(), 1, "{:?}", w.tweets);
    assert_eq!(w.tweets.skipped[0].key, format!("x:{ID}"));
    assert!(
        w.tweets.skipped[0].reason.contains(reason_has),
        "{}",
        w.tweets.skipped[0].reason
    );
    assert!(w.highlights.is_empty(), "no merged state for the index");
    assert!(w.skipped_works.contains(&wid()));
    assert!(w.tweets.log_message().unwrap().contains(&format!("x:{ID}")));
}

#[test]
fn the_same_tweet_twice_is_one_file_and_the_second_run_writes_nothing() {
    let env = Env::new("same");
    let (_, first) = env.import(&tweet(FULL, "likes"), DAY1);
    assert_eq!(first.tweets.created, 1);
    let file = env.only_file();
    let bytes = fs::read(&file).unwrap();
    let before = age(&file);

    let (_, again) = env.import(&tweet(FULL, "likes"), DAY2);
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
    let (first, _) = env.import(&tweet(SHORT, "likes"), DAY1);
    let name = env.names();
    let head_before: Vec<String> = env.text().lines().take(9).map(String::from).collect();

    let (work, w) = env.import(&tweet(FULL, "likes"), DAY2);
    assert_eq!((w.tweets.created, w.tweets.updated), (0, 1));
    assert_eq!(env.names(), name, "old filename kept, no second file");
    assert_eq!(format!("{}.md", work.slug), name[0]);
    let text = env.text();
    assert!(text.contains("Apache 2.0 licence"), "{text}");
    assert!(!text.contains("t.co/LHQOQrl9Ha"), "{text}");
    assert!(
        text.contains("highlighted_at: 2025-03-26 | tags: like\n"),
        "{text}"
    );
    // The frontmatter is as it was, but for updated_at.
    for (i, (was, is)) in head_before.iter().zip(text.lines()).enumerate() {
        if was.starts_with("updated_at: ") {
            assert_eq!(is, format!("updated_at: {DAY2}"));
        } else {
            assert_eq!(was, is, "frontmatter line {i}");
        }
    }
    assert_eq!(w.highlights[&hid()].text, FULL);
    // The index is given the title the file still has.
    assert_eq!(work.title, first.title);
}

#[test]
fn shorter_text_changes_nothing() {
    let env = Env::new("shorter");
    env.import(&tweet(FULL, "likes"), DAY1);
    let file = env.only_file();
    let bytes = fs::read(&file).unwrap();
    let before = age(&file);
    for worse in [SHORT, "Voice Chat + Video Chat!"] {
        let (work, w) = env.import(&tweet(worse, "likes"), DAY2);
        assert_eq!(w.tweets.unchanged, 1, "{worse}");
        assert_eq!(fs::read(env.only_file()).unwrap(), bytes, "{worse}");
        assert_eq!(mtime(&file), before, "{worse}");
        // The index is given what the file holds, not the worse text.
        assert_eq!(w.highlights[&hid()].text, FULL);
        assert!(work
            .title
            .starts_with("Voice Chat + Video Chat! Just in Qwen"));
    }
}

#[test]
fn an_incoming_error_page_is_not_written_as_a_new_file_or_as_an_update() {
    let env = Env::new("errorin");
    for page in [NO_SCRIPT_PAGE, "Something went wrong. Try reloading."] {
        let (_, w) = env.import(&tweet(page, "likes"), DAY1);
        assert_left_alone(&w, "error page");
        assert!(env.names().is_empty(), "no file for an error page");
    }

    env.import(&tweet(FULL, "likes"), DAY1);
    let file = env.only_file();
    let bytes = fs::read(&file).unwrap();
    let before = age(&file);
    // Not even its bookmark tag reaches the file.
    let (_, w) = env.import(&tweet(NO_SCRIPT_PAGE, "bookmarks"), DAY2);
    assert_left_alone(&w, "error page");
    assert_eq!(fs::read(env.only_file()).unwrap(), bytes);
    assert_eq!(mtime(&file), before);
}

#[test]
fn text_in_the_archive_is_never_replaced_for_looking_like_an_error_page() {
    // An error page an older import saved: it stays, the tweet is skipped.
    let env = Env::new("errorheld");
    let mut junk = tweet(NO_SCRIPT_PAGE, "likes");
    junk.author_handle = None;
    let file = env.seed(&junk, DAY1);
    let bytes = fs::read(&file).unwrap();
    let (_, w) = env.import(&tweet(FULL, "bookmarks"), DAY2);
    assert_left_alone(&w, "none of the file's highlights");
    assert_eq!(env.names().len(), 1);
    assert_eq!(fs::read(&file).unwrap(), bytes);
    drop(env);

    // r2 P1: a real tweet that opens like an error message, against a later
    // export that gives only its first two words.
    let env = Env::new("errorprose");
    let whole = "Something went wrong with our deployment. Here is the complete incident analysis, in full, which nobody would want to lose.";
    env.import(&tweet(whole, "likes"), DAY1);
    let file = env.only_file();
    let bytes = fs::read(&file).unwrap();
    let before = age(&file);
    let (_, w) = env.import(&tweet("Something went", "likes"), DAY2);
    assert_eq!((w.tweets.updated, w.tweets.unchanged), (0, 1));
    assert_eq!(fs::read(&file).unwrap(), bytes);
    assert_eq!(mtime(&file), before);
    assert_eq!(w.highlights[&hid()].text, whole);
}

#[test]
fn a_missing_date_is_filled_and_an_existing_one_is_kept() {
    let env = Env::new("dates");
    let mut undated = tweet(FULL, "likes");
    undated.created_at = None;
    undated.tweet_id = "1234567890".into(); // before the snowflake era
    let (_, w) = env.import(&undated, DAY1);
    assert_eq!(w.tweets.created, 1, "an undated tweet still imports");
    assert!(env.text().contains("\n\ntags: like\n"));

    let mut dated = tweet(FULL, "likes");
    dated.tweet_id = "1234567890".into();
    dated.created_at = Some("2010-05-01T00:00:00Z".into());
    let (_, w) = env.import(&dated, DAY2);
    assert_eq!(w.tweets.updated, 1, "{:?} {:?}", w.tweets, env.names());
    assert!(env
        .text()
        .contains("highlighted_at: 2010-05-01 | tags: like\n"));

    dated.created_at = Some("2011-11-11T00:00:00Z".into());
    let (_, w) = env.import(&dated, DAY3);
    assert_eq!(w.tweets.unchanged, 1);
    assert!(env
        .text()
        .contains("highlighted_at: 2010-05-01 | tags: like\n"));
    assert_eq!(
        w.highlights["x-1234567890"].created_at.as_deref(),
        Some("2010-05-01"),
        "the index gets the date the file keeps"
    );
}

#[test]
fn an_emptied_date_is_filled() {
    let env = Env::new("emptydate");
    env.import(&tweet(FULL, "likes"), DAY1);
    env.edit(|t| {
        t.replace(
            "highlighted_at: 2025-03-26 | tags: like\n",
            "highlighted_at:  | tags: like\n",
        )
    });
    let (_, w) = env.import(&tweet(FULL, "likes"), DAY2);
    assert_eq!(w.tweets.updated, 1);
    assert!(
        env.text()
            .contains("highlighted_at: 2025-03-26 | tags: like\n"),
        "{}",
        env.text()
    );
}

#[test]
fn like_then_bookmark_gives_both_tags() {
    let env = Env::new("tags");
    env.import(&tweet(FULL, "likes"), DAY1);
    let (_, w) = env.import(&tweet(FULL, "bookmarks"), DAY2);
    assert_eq!(w.tweets.updated, 1);
    let text = env.text();
    assert!(
        text.contains("highlighted_at: 2025-03-26 | tags: like, bookmark\n"),
        "{text}"
    );
    assert_eq!(text.matches("> Voice Chat").count(), 1);
    // And once both are there, either import changes nothing.
    let (_, w) = env.import(&tweet(FULL, "likes"), DAY3);
    assert_eq!(w.tweets.unchanged, 1);
}

#[test]
fn a_like_and_a_bookmark_in_one_batch_are_one_block_with_both_tags() {
    let env = Env::new("batch");
    let (work, like, _) = make_records(&tweet(FULL, "likes"), DAY1);
    let (_, bookmark, _) = make_records(&tweet(FULL, "bookmarks"), DAY1);
    let mut works = vec![work];
    let mut by: HashMap<String, Vec<&Highlight>> = HashMap::new();
    by.insert(works[0].id.clone(), vec![&like, &bookmark]);
    write_archive(env.dir.to_str().unwrap(), &mut works, &by).unwrap();
    let text = env.text();
    assert_eq!(text.matches("> Voice Chat").count(), 1, "{text}");
    assert!(text.contains("tags: like, bookmark\n"), "{text}");
}

#[test]
fn a_note_an_extra_tag_and_an_extra_highlight_survive_reimport() {
    let env = Env::new("user");
    env.import(&tweet(SHORT, "likes"), DAY1);
    let file = env.only_file();
    let edited = env.edit(|t| {
        t.replace(
            "highlighted_at: 2025-03-26 | tags: like\n",
            "highlighted_at: 2025-03-26 | tags: like, voice-models\n\nMy note on this.\n\n---\n\n> a line I pulled out myself\n\ntags: mine\n",
        )
    });

    // Same text: nothing to do, the edits stay byte for byte.
    let before = age(&file);
    let (_, w) = env.import(&tweet(SHORT, "likes"), DAY2);
    assert_eq!(w.tweets.unchanged, 1);
    assert_eq!(env.text(), edited);
    assert_eq!(mtime(&file), before);
    let merged = &w.highlights[&hid()];
    assert_eq!(merged.note.as_deref(), Some("My note on this."));
    assert_eq!(merged.tags, ["like", "voice-models"]);

    // Fuller text and a new tag: the tweet block changes, the rest stays.
    let (_, w) = env.import(&tweet(FULL, "bookmarks"), DAY3);
    assert_eq!(w.tweets.updated, 1);
    let text = env.text();
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
    let (short_work, short_h, _) = make_records(&tweet(SHORT, "likes"), DAY1);
    let (mut full_work, full_h, _) = make_records(&tweet(FULL, "likes"), DAY2);
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

    let (work, w) = env.import(&tweet(FULL, "bookmarks"), DAY3);
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
fn works_from_other_sources_are_written_as_before() {
    let env = Env::new("mixed");
    let (tweet_work, tweet_h, _) = make_records(&tweet(FULL, "likes"), DAY1);
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

// ---- One test per first-round review finding ----

#[test]
fn another_work_at_the_tweets_filename_is_never_touched() {
    let env = Env::new("occupant");
    let slug = make_records(&tweet(FULL, "likes"), DAY1).0.slug;
    let occupant = env.works().join(format!("{slug}.md"));
    let other = "---\ntitle: Somebody else's work\ntype: book\nsource_system: kindle\nsource_id: \"B00\"\nimported_at: 2026-01-01T00:00:00+00:00\nupdated_at: 2026-01-01T00:00:00+00:00\nsource_data: {}\n---\n\n> Voice Chat + Video Chat! Just in Qwen Chat.\n\ntags: like\n\n---\n\n";
    fs::write(&occupant, other).unwrap();
    let before = age(&occupant);

    let (work, w) = env.import(&tweet(FULL, "likes"), DAY2);
    assert_eq!(w.tweets.created, 1);
    assert_eq!(fs::read_to_string(&occupant).unwrap(), other);
    assert_eq!(mtime(&occupant), before);
    assert_eq!(work.slug, format!("{slug}-2"));
    assert_eq!(env.names(), [format!("{slug}-2.md"), format!("{slug}.md")]);

    // The next import finds the tweet by its key, under the suffixed name.
    let (work, w) = env.import(&tweet(FULL, "bookmarks"), DAY3);
    assert_eq!((w.tweets.created, w.tweets.updated), (0, 1));
    assert_eq!(work.slug, format!("{slug}-2"));
    assert_eq!(env.names().len(), 2);
    assert_eq!(fs::read_to_string(&occupant).unwrap(), other);
}

#[test]
fn a_name_taken_at_publication_is_never_written_over() {
    // Publication is the only check there is: whatever holds the name at
    // that moment (a file, a directory, a link to nowhere) keeps it.
    let env = Env::new("taken");
    let slug = make_records(&tweet(FULL, "likes"), DAY1).0.slug;
    let name = |n: &str| env.works().join(format!("{slug}{n}.md"));
    fs::write(name(""), "someone else's bytes").unwrap();
    fs::create_dir(name("-2")).unwrap();
    std::os::unix::fs::symlink(env.dir.join("nowhere"), name("-3")).unwrap();

    assert!(!archive_meta::create_exclusive(&name(""), b"ours").unwrap());
    assert!(!archive_meta::create_exclusive(&name("-3"), b"ours").unwrap());
    assert_eq!(env.names().len(), 3, "no temp file left: {:?}", env.names());

    let (work, w) = env.import(&tweet(FULL, "likes"), DAY2);
    assert_eq!(w.tweets.created, 1);
    assert_eq!(work.slug, format!("{slug}-4"));
    assert_eq!(
        fs::read_to_string(name("")).unwrap(),
        "someone else's bytes"
    );
    assert!(name("-2").is_dir());
    assert!(name("-3").symlink_metadata().unwrap().is_symlink());
    assert!(
        !env.dir.join("nowhere").exists(),
        "the link was not followed"
    );
    assert!(fs::read_to_string(name("-4"))
        .unwrap()
        .contains("Apache 2.0 licence"));
    assert_eq!(env.names().len(), 4);
}

#[test]
fn a_tweet_block_the_app_cannot_read_back_leaves_the_users_highlight_alone() {
    let env = Env::new("reordered");
    env.import(&tweet(SHORT, "likes"), DAY1);
    // The user reorders the tweet's metadata and adds a highlight.
    let edited = env.edit(|t| {
        t.replace(
            "highlighted_at: 2025-03-26 | tags: like\n",
            "tags: like | highlighted_at: 2025-03-26\n\n---\n\n> my own separate highlight\n\ntags: mine\n",
        )
    });
    let before = age(&env.only_file());

    let (_, w) = env.import(&tweet(FULL, "bookmarks"), DAY2);
    assert_left_alone(&w, "cannot read back");
    assert_eq!(env.text(), edited);
    assert_eq!(mtime(&env.only_file()), before);
}

#[test]
fn several_blocks_that_could_be_the_tweet_or_none_leave_the_file_alone() {
    let env = Env::new("ambiguous");
    env.import(&tweet(SHORT, "likes"), DAY1);
    // An older build wrote a like and a bookmark as two blocks.
    let twice = env.edit(|t| {
        t.replace(
            "tags: like\n",
            &format!(
                "tags: like\n\n---\n\n> {SHORT}\n\nhighlighted_at: 2025-03-26 | tags: bookmark\n"
            ),
        )
    });
    let (_, w) = env.import(&tweet(FULL, "likes"), DAY2);
    assert_left_alone(&w, "more than one");
    assert_eq!(env.text(), twice);

    // Two highlights, neither of them this tweet's text.
    let neither = env.edit(|t| t.replace(SHORT, "something the user rewrote entirely"));
    let (_, w) = env.import(&tweet(FULL, "likes"), DAY3);
    assert_left_alone(&w, "none of");
    assert_eq!(env.text(), neither);
}

#[test]
fn a_multiline_yaml_title_is_left_exactly_as_it_is() {
    let env = Env::new("multiline");
    env.import(&tweet(SHORT, "likes"), DAY1);
    env.edit(|t| {
        let title = t.lines().nth(1).unwrap();
        assert!(title.starts_with("title: "));
        t.replace(title, "title: |\n  My manually edited title")
    });

    let (work, w) = env.import(&tweet(FULL, "likes"), DAY2);
    assert_eq!(w.tweets.updated, 1);
    let text = env.text();
    assert!(
        text.starts_with("---\ntitle: |\n  My manually edited title\nauthor: Alibaba_Qwen\n"),
        "{text}"
    );
    assert!(text.contains("Apache 2.0 licence"), "{text}");
    let fm = tweet_frontmatter::read(text.as_bytes()).readable().unwrap();
    assert_eq!(fm.get("title").as_deref(), Some("My manually edited title"));
    assert_eq!(fm.key().as_deref(), Some(format!("x:{ID}").as_str()));
    assert_eq!(fm.get("updated_at").as_deref(), Some(DAY2));
    assert_eq!(work.title, "My manually edited title");
}

#[test]
fn an_updated_at_that_is_not_a_plain_line_is_left_and_the_block_still_updates() {
    let env = Env::new("updatedat");
    env.import(&tweet(SHORT, "likes"), DAY1);
    env.edit(|t| {
        t.replace(
            &format!("updated_at: {DAY1}\n"),
            &format!("updated_at: >-\n  {DAY1}\n"),
        )
    });
    let (_, w) = env.import(&tweet(FULL, "likes"), DAY2);
    assert_eq!(w.tweets.updated, 1);
    let text = env.text();
    assert!(
        text.contains(&format!("updated_at: >-\n  {DAY1}\n")),
        "{text}"
    );
    assert!(text.contains("Apache 2.0 licence"), "{text}");
}

#[test]
fn a_shorter_opening_with_a_long_parent_quote_does_not_replace_the_whole_text() {
    let env = Env::new("context");
    let whole = "First sentence of the post. Second sentence of the post. Third sentence, which a later source leaves out.";
    env.import(&tweet(whole, "likes"), DAY1);
    let file = env.only_file();
    let bytes = fs::read(&file).unwrap();
    let before = age(&file);

    let mut opening = tweet("First sentence of the post.", "likes");
    opening.parent_text = Some("A very long parent tweet that makes the assembled body much longer than the complete paragraph already in the archive, by a wide margin, so that a length count would prefer it.".into());
    opening.parent_handle = Some("someone".into());
    opening.images =
        vec!["https://pbs.twimg.com/media/AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA.jpg".into()];
    let (_, w) = env.import(&opening, DAY2);
    assert_eq!(w.tweets.unchanged, 1);
    assert_eq!(fs::read(&file).unwrap(), bytes);
    assert_eq!(mtime(&file), before);
    assert_eq!(w.highlights[&hid()].text, whole);
}

#[test]
fn context_the_file_already_has_is_kept_when_the_text_is_replaced() {
    let env = Env::new("keepcontext");
    let mut cut = tweet(SHORT, "likes");
    cut.article_urls = vec!["https://chat.qwen.ai".into()];
    env.import(&cut, DAY1);

    let mut whole = tweet(FULL, "likes");
    whole.quoted_text = Some("a quoted tweet the first export lacked".into());
    env.import(&whole, DAY2);
    let text = env.text();
    assert!(
        text.contains("the rest of a long post.\n> \n> 🔗 https://chat.qwen.ai\n\nhighlighted_at"),
        "{text}"
    );
    assert!(
        !text.contains("a quoted tweet"),
        "incoming context is not added: {text}"
    );
}

#[test]
fn a_tweet_that_really_ends_in_an_ellipsis() {
    // In the archive whole: kept against a shorter and against another text.
    let env = Env::new("ellipsis-kept");
    let whole = "Wait for it, because the ending is the point...";
    env.import(&tweet(whole, "likes"), DAY1);
    let bytes = fs::read(env.only_file()).unwrap();
    for other in [
        "Wait for it",
        "Wait for it, because the ending is the point",
    ] {
        let (_, w) = env.import(&tweet(other, "likes"), DAY2);
        assert_eq!(w.tweets.unchanged, 1, "{other}");
        assert_eq!(fs::read(env.only_file()).unwrap(), bytes, "{other}");
    }
    drop(env);

    // Arriving whole over a fragment that carries no cut-off mark: the
    // fragment is kept (the tweet is recognised, so its tag still lands).
    let env = Env::new("ellipsis-nomark");
    env.import(&tweet("Wait for it, because", "likes"), DAY1);
    let (_, w) = env.import(&tweet(whole, "bookmarks"), DAY2);
    assert_eq!(w.tweets.updated, 1);
    let text = env.text();
    assert!(text.contains("> Wait for it, because\n\n"), "{text}");
    assert!(text.contains("tags: like, bookmark\n"), "{text}");
    assert_eq!(w.highlights[&hid()].text, "Wait for it, because");
}

#[test]
fn identity_is_found_in_quoted_commented_and_reordered_frontmatter() {
    let env = Env::new("identity");
    env.import(&tweet(SHORT, "likes"), DAY1);
    // Renamed by hand, identity respelled in valid YAML and moved.
    let respelled = env.edit(|t| {
        t.replace("source_system: x\n", "")
            .replace(
                &format!("source_id: \"{ID}\"\n"),
                &format!("source_id: '{ID}' # the tweet\n"),
            )
            .replace(
                "---\n\n>",
                "\"source_system\": \"x\" # moved to the end\n---\n\n>",
            )
    });
    assert!(respelled.contains("\"source_system\": \"x\""));
    fs::rename(env.only_file(), env.works().join("renamed-by-hand.md")).unwrap();

    let (work, w) = env.import(&tweet(FULL, "likes"), DAY2);
    assert_eq!((w.tweets.created, w.tweets.updated), (0, 1));
    assert_eq!(env.names(), ["renamed-by-hand.md"]);
    assert_eq!(work.slug, "renamed-by-hand");
    assert!(env.text().contains("Apache 2.0 licence"));
    assert!(env
        .text()
        .contains(&format!("source_id: '{ID}' # the tweet\n")));
}

#[test]
fn a_crlf_file_is_found_by_key_and_left_alone() {
    let env = Env::new("crlf");
    env.import(&tweet(FULL, "likes"), DAY1);
    let crlf = env.edit(|t| t.replace('\n', "\r\n"));
    let before = age(&env.only_file());

    for text in [SHORT, FULL, "Voice Chat + Video Chat! Just in Qwen Chat. We open-source the model behind all this, under the Apache 2.0 licence, and here is the rest of a long post. And more."] {
        let (_, w) = env.import(&tweet(text, "bookmarks"), DAY2);
        assert_left_alone(&w, "not laid out");
        assert_eq!(env.text(), crlf, "no second file, same bytes");
        assert_eq!(mtime(&env.only_file()), before);
    }
}

#[test]
fn trailing_text_after_the_last_rule_leaves_the_file_alone() {
    let env = Env::new("unreadable");
    env.import(&tweet(SHORT, "likes"), DAY1);
    let odd = env.edit(|t| format!("{t}hand-written text after the last rule"));
    let (_, w) = env.import(&tweet(FULL, "likes"), DAY2);
    assert_left_alone(&w, "not laid out");
    assert_eq!(env.text(), odd);
}

#[test]
fn a_file_changed_after_the_replacement_was_prepared_is_not_overwritten() {
    let env = Env::new("race");
    env.import(&tweet(SHORT, "likes"), DAY1);
    let file = env.only_file();
    let key = format!("x:{ID}");
    let c = Candidate::load(&file, &key, FULL).unwrap();
    let ours = String::from_utf8(c.original.clone())
        .unwrap()
        .replace(SHORT, FULL);

    // The replacement is on disk, in its temp file…
    let prepared = PreparedReplacement::new(&file, ours.as_bytes()).unwrap();
    assert_eq!(env.names().len(), 2);
    // …and only then another writer adds a tag.
    fs::write(
        &file,
        String::from_utf8(c.original.clone())
            .unwrap()
            .replace("tags: like\n", "tags: like, bookmark\n"),
    )
    .unwrap();
    let theirs = fs::read(&file).unwrap();
    assert!(!prepared.publish_if_target_is(&c.original).unwrap());
    assert_eq!(fs::read(&file).unwrap(), theirs);
    assert_eq!(env.names().len(), 1, "the temp file is dropped");

    // A target that has gone is not recreated either.
    let prepared = PreparedReplacement::new(&file, ours.as_bytes()).unwrap();
    fs::remove_file(&file).unwrap();
    assert!(!prepared.publish_if_target_is(&theirs).unwrap());
    assert!(env.names().is_empty());

    // Unchanged since loading: the write goes through.
    fs::write(&file, &theirs).unwrap();
    let prepared = PreparedReplacement::new(&file, ours.as_bytes()).unwrap();
    assert!(prepared.publish_if_target_is(&theirs).unwrap());
    assert_eq!(env.text(), ours);
    assert_eq!(env.names().len(), 1);
}

#[test]
fn the_text_rule() {
    // A cut-off text is continued by the whole one.
    assert_eq!(
        replacement("abc def… https://t.co/x1", "abc defg").as_deref(),
        Some("abc defg")
    );
    assert_eq!(
        replacement("abc def...", "abc def ghi").as_deref(),
        Some("abc def ghi")
    );
    assert_eq!(replacement(SHORT, FULL).as_deref(), Some(FULL));
    // One prose word after the stem is enough, beside a link or not.
    for rest in [
        " report by [the authors](https://example.org) today",
        " report https://example.org",
        " 2 https://t.co/b",
        " 報告 https://t.co/b",
        " ok.",
        " today, https://t.co/b",
        "s https://t.co/b",
    ] {
        let whole = format!("Read this{rest}");
        assert_eq!(
            replacement("Read this… https://t.co/a", &whole).as_deref(),
            Some(whole.as_str()),
            "{whole:?}"
        );
    }
    assert_eq!(
        replacement("Tom &amp; Jerry…", "Tom & Jerry forever").as_deref(),
        Some("Tom & Jerry forever")
    );
    // Everything else keeps what is there.
    for (existing, incoming) in [
        (FULL, SHORT),
        ("my comment and more", "my comment"),
        ("my comment", "my comment and more"),
        ("same  text", "same text"),
        ("Great pic", "Great pic https://t.co/abc"),
        ("Great pic https://t.co/abc", "Great pic"),
        // r2: a trailing link is content, not a cut-off mark.
        (
            "Read this https://t.co/important",
            "Read this updated explanation",
        ),
        (
            "Read this https://t.co/important",
            "Read this https://t.co/important and more",
        ),
        ("one thing", "another thing entirely, and longer"),
        ("abc…", "abd and more"),
        // The same cut with another link, or no more than the cut itself.
        ("abc… https://t.co/one", "abc… https://t.co/two"),
        // r3: a link, changed or longer or one more of them, is not text.
        ("Read this… https://t.co/a", "Read this https://t.co/b"),
        ("Read this…", "Read this https://t.co/b"),
        (
            "Read this… https://t.co/a",
            "Read this https://t.co/b http://example.com/c",
        ),
        ("Read https://t.co/a…", "Read https://t.co/abcdef"),
        // r4, r5: a token that could be part of a link, however the link
        // is written, is not prose; nor is a mention, a hashtag, punctuation.
        (
            "Read this… https://t.co/a",
            "Read this [https://example.org/report](https://example.org/report)",
        ),
        (
            "Read this… https://t.co/a",
            "Read this <https://example.org>",
        ),
        ("Read this… https://t.co/a", WIKI_LINK),
        (
            "Read this… https://t.co/a",
            "Read this [example.org/report](https://example.org/report)",
        ),
        ("Read this… https://t.co/a", "Read this example.org/report"),
        ("Read this… https://t.co/a", "Read this site.com."),
        ("Read this… https://t.co/a", "Read this www.example"),
        (
            "Read this… https://t.co/a",
            "Read this [](https://example.org)",
        ),
        ("Read this… https://t.co/a", "Read this @someone #topic"),
        ("Read this… https://t.co/a", "Read this ! https://t.co/b"),
        ("Read this… https://t.co/a", "Read this! https://t.co/b"),
        ("Read this…", "Read this!?"),
        ("abc… https://t.co/one", "abc…"),
        ("abc…", "abc"),
        ("…", "anything"),
        ("a", ""),
        // More context alone is not more tweet.
        (
            "my comment",
            "my comment\n\n— Quoting @carol:\n> the original",
        ),
        ("https://t.co/abc", "words at last"),
        // Text in the archive that reads like an error page is still kept.
        ("Something went wrong. Try reloading.", "the real tweet"),
        (NO_SCRIPT_PAGE, "the real tweet"),
    ] {
        assert_eq!(
            replacement(existing, incoming),
            None,
            "{existing:?} <- {incoming:?}"
        );
    }
    // Existing context is kept, incoming context dropped.
    assert_eq!(
        replacement(
            "abc…\n\n🔗 https://one.example",
            "abc def\n\n🔗 https://two.example"
        )
        .as_deref(),
        Some("abc def\n\n🔗 https://one.example")
    );
    assert_eq!(
        replacement("abc…", "abc def\n\n🔗 https://two.example").as_deref(),
        Some("abc def")
    );
}

#[test]
fn which_blocks_count_as_the_tweet() {
    for (a, b) in [
        ("same  text", "same text"),
        ("Tom &amp; Jerry", "Tom & Jerry"),
        ("my comment", "my comment and more"),
        (SHORT, FULL),
        ("my comment\n\n🔗 https://one.example", "my comment"),
        ("![image](https://a/b.jpg)", "![image](https://a/b.jpg)"),
    ] {
        assert!(related(a, b) && related(b, a), "{a:?} / {b:?}");
    }
    for (a, b) in [
        ("My budget assumptions", FULL),
        (
            "Read this https://t.co/important",
            "Read this updated explanation",
        ),
        ("abc…", "abd and more"),
        ("abc… https://t.co/one", "abc… https://t.co/two"),
        // r4, r5: the same words and a link written as Markdown.
        ("Read this… https://t.co/a", WIKI_LINK),
        ("Read this… https://t.co/a", "Read this @someone #topic"),
        (
            "Read this… https://t.co/a",
            "Read this [https://example.org/report](https://example.org/report)",
        ),
        (
            "Read this… https://t.co/a",
            "Read this <https://example.org>",
        ),
        // No text of their own, and not the same body.
        ("![image](https://a/b.jpg)", "![image](https://a/c.jpg)"),
        ("![image](https://a/b.jpg)", "words"),
        ("", "words"),
    ] {
        assert!(!related(a, b) && !related(b, a), "{a:?} / {b:?}");
    }
}

/// One test per second-round review finding.
#[path = "tweet_upsert_findings_tests.rs"]
mod findings;
