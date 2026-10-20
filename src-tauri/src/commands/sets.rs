//! Tauri commands for sets (HS-2A), "more like these" and the ArchiveScout
//! hand-off check. Each runs on the blocking pool. The logic and its tests
//! live in `crate::sets`, `crate::sets_export` and `crate::handoff`; this
//! file only connects them to the engine (`cite`, `verify-quote`, `similar`,
//! `search`) and to the app's archive path.

use scout_corpus::api::{CiteQuery, SearchQuery, SimilarQuery, VerifyQuoteQuery};
use scout_corpus::{CitedPassage, Engine, SearchMode};
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;

use crate::corpus::{self, CorpusError};
use crate::sets::{self, Found, Lookup, SetEdit, SetFile, SetItem, SetKind, SetStore, SetSummary, Status, Suggestion};
use crate::sets_export::{self, ExportKind};
use crate::AppState;

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.unwrap_or_else(|e| Err(format!("set task failed: {e}")))
}

fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

fn store(app: &AppHandle) -> SetStore {
    SetStore::new(&app.state::<AppState>().config().archive_path)
}

fn engine() -> Result<Engine, String> {
    corpus::engine_from_env().map_err(|e: CorpusError| e.message)
}

/// The engine behind [`Lookup`].
struct EngineLookup<'a>(&'a Engine);

impl Lookup for EngineLookup<'_> {
    fn cite(&self, id: &str) -> Option<Found> {
        let c = self.0.cite(&CiteQuery { passage_id: id.to_string() }).ok()?.body;
        Some(Found { passage_id: c.passage_id, rel_path: c.rel_path, quote: c.quote })
    }
    fn find_quote(&self, corpus: &str, quote: &str) -> Vec<Found> {
        self.0
            .verify_quote(&VerifyQuoteQuery { text: quote.to_string(), in_: vec![corpus.to_string()] })
            .map(|r| {
                r.body
                    .matches
                    .into_iter()
                    .map(|m| Found { passage_id: m.passage_id, rel_path: m.rel_path, quote: m.original })
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[derive(Debug, Serialize)]
pub struct ResolvedItem {
    pub item: SetItem,
    pub status: Status,
    /// The passage as the engine has it now, with its citation; `None` when the source changed.
    pub cited: Option<CitedPassage>,
    /// Saved-search sets: the user unpinned it, so it leaves at the next update
    /// (unless it carries a note or prose of theirs).
    pub kept_out: bool,
}

#[derive(Debug, Serialize)]
pub struct OpenedSet {
    pub id: String,
    pub title: String,
    pub kind: SetKind,
    pub query: Option<String>,
    pub items: Vec<ResolvedItem>,
}

/// Resolve every item of a set; ids that were repaired are written back
/// (under the set's lock, with the rest of the file as it was).
/// When the engine is unavailable the items come back unresolved ("changed"
/// without a citation is wrong), so an engine error is an error.
fn open_set(st: &SetStore, id: &str, e: &Engine) -> Result<OpenedSet, String> {
    let lookup = EngineLookup(e);
    st.update(id, |set| {
        let resolved = sets::repair_all(set, &lookup);
        let mut items = vec![];
        for (it, (status, found)) in set.items.iter().zip(resolved) {
            let cited = found.and_then(|f| e.cite(&CiteQuery { passage_id: f.passage_id }).ok().map(|r| r.body));
            let kept_out = set.kind == SetKind::SavedSearch && !it.pinned && set.excluded.contains(&it.passage);
            items.push(ResolvedItem { item: it.clone(), status, cited, kept_out });
        }
        Ok(OpenedSet { id: id.to_string(), title: set.title.clone(), kind: set.kind, query: set.query.clone(), items })
    })
}

fn summary(id: &str, set: &SetFile) -> SetSummary {
    SetSummary { id: id.to_string(), title: set.title.clone(), kind: set.kind, items: set.items.len(), updated: set.updated.clone() }
}

#[tauri::command]
pub async fn sets_list(app: AppHandle) -> Result<Vec<SetSummary>, String> {
    let st = store(&app);
    blocking(move || Ok(st.list())).await
}

/// A new set; a saved-search set is filled from its query at once.
#[tauri::command]
pub async fn set_create(app: AppHandle, title: String, query: Option<String>) -> Result<SetSummary, String> {
    let st = store(&app);
    blocking(move || {
        if title.trim().is_empty() {
            return Err("a set needs a title".into());
        }
        let query = query.map(|q| q.trim().to_string()).filter(|q| !q.is_empty());
        let kind = if query.is_some() { SetKind::SavedSearch } else { SetKind::Hand };
        let mut set = SetFile::new(&title, kind, query, &today());
        if set.kind == SetKind::SavedSearch {
            let matches = saved_matches(&engine()?, set.query.as_deref().unwrap_or(""))?;
            sets::refresh_saved(&mut set, matches, SAVED_CAP, &today());
        }
        let id = st.create(&set)?;
        Ok(summary(&id, &set))
    })
    .await
}

#[tauri::command]
pub async fn set_open(app: AppHandle, id: String) -> Result<OpenedSet, String> {
    let st = store(&app);
    blocking(move || open_set(&st, &id, &engine()?)).await
}

/// Add a passage (by engine id) to the end of a set.
#[tauri::command]
pub async fn set_add(app: AppHandle, id: String, passage_id: String) -> Result<SetSummary, String> {
    let st = store(&app);
    blocking(move || {
        let c = engine()?.cite(&CiteQuery { passage_id }).map_err(|e| format!("{e:#}"))?.body;
        add(&st, &id, sets::item_from_cited(&c))
    })
    .await
}

/// Where a work's highlights are in the highlights corpus: its Markdown
/// file in the archive (`readings/works/<slug>.md`, as `workMarkdownPath`).
pub fn work_rel_path(slug: &str) -> String {
    format!("readings/works/{slug}.md")
}

/// The passage holding `text` in the selected work's own file; another
/// work quoting the same words is never taken instead.
fn find_in_work(l: &dyn Lookup, corpus: &str, text: &str, work_slug: &str) -> Option<Found> {
    let rel = work_rel_path(work_slug);
    l.find_quote(corpus, text).into_iter().find(|f| f.rel_path == rel)
}

/// Add a highlight the classic highlight list shows (no passage id there):
/// found in the highlights corpus by its text, within the selected work.
#[tauri::command]
pub async fn set_add_text(app: AppHandle, id: String, corpus: String, text: String, work_slug: String) -> Result<SetSummary, String> {
    let st = store(&app);
    blocking(move || {
        let e = engine()?;
        let f = find_in_work(&EngineLookup(&e), &corpus, &text, &work_slug).ok_or("that highlight is not in the search index yet")?;
        let c = e.cite(&CiteQuery { passage_id: f.passage_id.clone() }).map_err(|e| format!("{e:#}"))?.body;
        add(&st, &id, sets::item_from_cited(&c))
    })
    .await
}

fn add(st: &SetStore, id: &str, item: SetItem) -> Result<SetSummary, String> {
    st.update(id, |set| {
        sets::add_item(set, item.clone(), &today());
        Ok(summary(id, set))
    })
}

/// Reorder, annotate, remove, pin or rename.
#[tauri::command]
pub async fn set_edit(app: AppHandle, id: String, edit: SetEdit) -> Result<SetSummary, String> {
    let st = store(&app);
    blocking(move || {
        st.update(&id, |set| {
            sets::apply_edit(set, &edit, &today())?;
            Ok(summary(&id, set))
        })
    })
    .await
}

/// "More like these…": the engine's `similar` over the set's resolved items.
#[tauri::command]
pub async fn set_suggest(app: AppHandle, id: String) -> Result<Vec<Suggestion>, String> {
    let st = store(&app);
    blocking(move || suggest_for(&st, &id, &engine()?)).await
}

fn suggest_for(st: &SetStore, id: &str, e: &Engine) -> Result<Vec<Suggestion>, String> {
    let opened = open_set(st, id, e)?;
    let rows: Vec<(Status, String)> = opened.items.iter().map(|r| (r.status, r.item.passage.clone())).collect();
    let seeds = sets::seeds(&rows);
    if seeds.is_empty() {
        return Err("add a passage to the set first: suggestions need something to resemble".into());
    }
    let set = st.load(id)?;
    let r = match e.similar(&SimilarQuery { seeds, in_: vec![], top: 30, exclude_seeds: true }) {
        Ok(r) => r.body,
        // The engine fails when nothing shares two terms with the seeds: no suggestions, not a failure.
        Err(err) if format!("{err:#}").contains("share") => return Ok(vec![]),
        Err(err) => return Err(format!("{err:#}")),
    };
    Ok(sets::filter_suggestions(sets::suggestions_from(&r), &set))
}

/// "Not this": the suggestion does not come back after a reopen.
#[tauri::command]
pub async fn set_dismiss(app: AppHandle, id: String, passage_id: String) -> Result<(), String> {
    let st = store(&app);
    blocking(move || {
        st.update(&id, |set| {
            sets::dismiss(set, &passage_id, &today());
            Ok(())
        })
    })
    .await
}

const SAVED_CAP: usize = 40;

/// The first passage of each document a saved search matches.
fn saved_matches(e: &Engine, query: &str) -> Result<Vec<SetItem>, String> {
    let r = corpus::search(e, &SearchQuery { query: query.to_string(), limit: SAVED_CAP, mode: SearchMode::Fts, ..Default::default() })
        .map_err(|e| e.message)?;
    let mut out = vec![];
    for d in r.body.results {
        let Some(h) = d.hits.first() else { continue };
        let Ok(c) = e.cite(&CiteQuery { passage_id: h.passage_id.clone() }) else { continue };
        out.push(sets::item_from_cited(&c.body));
    }
    Ok(out)
}

/// Refresh one saved-search set from its query; whether it changed. Moved
/// passages are repaired first, so an item whose passage moved keeps its
/// note and prose under the new id instead of being replaced by a fresh
/// match; pinned and annotated items stay (`sets::refresh_saved`).
fn refresh_one(st: &SetStore, id: &str, e: &Engine) -> Result<bool, String> {
    let lookup = EngineLookup(e);
    st.update(id, |set| {
        let Some(q) = set.query.clone() else { return Ok(false) };
        let before = set.clone();
        sets::repair_all(set, &lookup);
        let matches = saved_matches(e, &q)?;
        sets::refresh_saved(set, matches, SAVED_CAP, &today());
        Ok(*set != before)
    })
}

/// Every saved-search set follows its query again: run after an index build
/// (which follows every sync). Returns the ids that changed; one set that
/// cannot be refreshed does not stop the others.
fn refresh_all_saved(st: &SetStore, e: &Engine) -> Vec<String> {
    st.list()
        .into_iter()
        .filter(|s| s.kind == SetKind::SavedSearch)
        .filter(|s| matches!(refresh_one(st, &s.id, e), Ok(true)))
        .map(|s| s.id)
        .collect()
}

/// A saved-search set: pinned items stay, the rest follow the query.
#[tauri::command]
pub async fn set_refresh(app: AppHandle, id: String) -> Result<OpenedSet, String> {
    let st = store(&app);
    blocking(move || {
        let e = engine()?;
        refresh_one(&st, &id, &e)?;
        open_set(&st, &id, &e)
    })
    .await
}

/// After a sync or an index build: every saved-search set takes in its new matches.
#[tauri::command]
pub async fn sets_refresh_saved(app: AppHandle) -> Result<Vec<String>, String> {
    let st = store(&app);
    blocking(move || Ok(refresh_all_saved(&st, &engine()?))).await
}

#[derive(Debug, Serialize)]
pub struct ExportOut {
    pub file_name: String,
    pub text: String,
}

fn render_export(st: &SetStore, id: &str, kind: ExportKind, e: &Engine) -> Result<ExportOut, String> {
    let opened = open_set(st, id, e)?;
    let set = st.load(id)?;
    let items: Vec<_> = opened
        .items
        .iter()
        .map(|r| sets_export::export_item(&r.item, r.status, r.cited.as_ref()))
        .collect();
    Ok(ExportOut { file_name: kind.file_name(&set.title), text: sets_export::render(kind, &set, &items) })
}

#[tauri::command]
pub async fn set_export(app: AppHandle, id: String, kind: ExportKind) -> Result<ExportOut, String> {
    let st = store(&app);
    blocking(move || render_export(&st, &id, kind, &engine()?)).await
}

/// Render an export and write it only to the file `choose` returns (the
/// save dialog); `None` (cancelled) writes nothing.
fn export_to_chosen(
    st: &SetStore,
    id: &str,
    kind: ExportKind,
    e: &Engine,
    choose: impl FnOnce(&str) -> Result<Option<PathBuf>, String>,
) -> Result<Option<String>, String> {
    let out = render_export(st, id, kind, e)?;
    let Some(path) = choose(&out.file_name)? else { return Ok(None) };
    std::fs::write(&path, out.text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(Some(path.display().to_string()))
}

/// Save an export: the backend opens the save dialog itself and writes only
/// to the file the user chose there. The renderer never names a path.
#[tauri::command]
pub async fn set_export_save(app: AppHandle, id: String, kind: ExportKind) -> Result<Option<String>, String> {
    let st = store(&app);
    blocking(move || {
        export_to_chosen(&st, &id, kind, &engine()?, |name| {
            app.dialog()
                .file()
                .set_file_name(name)
                .add_filter("Markdown", &["md"])
                .blocking_save_file()
                .map(|p| p.into_path().map_err(|e| format!("{e}")))
                .transpose()
        })
    })
    .await
}

/// Whether ArchiveScout is installed (the Analyse button's two states).
#[tauri::command]
pub async fn archivescout_installed() -> bool {
    tauri::async_runtime::spawn_blocking(crate::handoff::installed).await.unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::fixture::{write, Fixture};
    use crate::corpus::IndexKeeper;

    fn build(fx: &Fixture) {
        IndexKeeper::default().refresh(|| Ok(fx.engine()), &|_| {}).unwrap();
    }

    fn id_of(e: &Engine, q: &str, corpus: &str) -> String {
        let r = e
            .search(&SearchQuery { query: q.into(), in_: vec![corpus.into()], mode: SearchMode::Fts, ..Default::default() })
            .unwrap();
        r.body.results[0].hits[0].passage_id.clone()
    }

    #[test]
    fn a_set_resolves_repairs_and_suggests_over_a_real_engine() {
        let fx = Fixture::new("sets");
        build(&fx);
        let e = fx.engine();
        let archive = fx.root.join("hs-archive");
        std::fs::create_dir_all(&archive).unwrap();
        let st = SetStore::new(archive.to_str().unwrap());
        let set_id = st.create(&SetFile::new("Metaphor talk", SetKind::Hand, None, "2026-10-20")).unwrap();

        // Add one writing passage and one highlight; both resolve.
        let w = id_of(&e, "frames a problem", "writing");
        let h = id_of(&e, "seeing one thing as another", "highlights");
        for pid in [&w, &h] {
            let c = e.cite(&CiteQuery { passage_id: pid.clone() }).unwrap().body;
            add(&st, &set_id, sets::item_from_cited(&c)).unwrap();
        }
        let opened = open_set(&st, &set_id, &e).unwrap();
        assert_eq!(opened.items.len(), 2);
        assert!(opened.items.iter().all(|i| i.status == Status::Ok && i.cited.is_some()));

        // Suggestions: from the tweet and the other writing passage, never the items.
        let sugg = suggest_for(&st, &set_id, &e).unwrap();
        assert!(!sugg.is_empty());
        assert!(sugg.iter().all(|g| g.passage_id != w && g.passage_id != h));
        assert!(sugg.iter().all(|g| !g.shared_terms.is_empty() && !g.similar_to.is_empty()));
        let first = sugg[0].passage_id.clone();
        let mut s = st.load(&set_id).unwrap();
        sets::dismiss(&mut s, &first, "d");
        st.save(&set_id, &s).unwrap();
        let again = suggest_for(&st, &set_id, &e).unwrap();
        assert!(again.iter().all(|g| g.passage_id != first), "a dismissed suggestion does not return");

        // The source is edited: the writing passage moves down the file. The id is repaired.
        write(
            &fx.sources[0],
            "essays/2016-generative-metaphor.md",
            "---\ntitle: Generative metaphor\ndate: 2016-06-23\ngenre: essay\n---\n\
             A new opening paragraph that was not there before.\n\n\
             A generative metaphor frames a problem and suggests its solution.\n\n\
             Schön’s *generative metaphor* is a model of thought.\n",
        );
        build(&fx);
        let opened = open_set(&st, &set_id, &fx.engine()).unwrap();
        assert_eq!(opened.items[0].status, Status::Repaired);
        assert_ne!(opened.items[0].item.passage, w);
        assert_eq!(opened.items[1].status, Status::Ok);
        // The repair is in the file, and the stored quote is untouched.
        let saved = st.load(&set_id).unwrap();
        assert_eq!(saved.items[0].passage, opened.items[0].item.passage);
        assert!(saved.items[0].quote.contains("frames a problem"));
        // Reopening is now plain Ok.
        assert_eq!(open_set(&st, &set_id, &fx.engine()).unwrap().items[0].status, Status::Ok);

        // The text is gone altogether: "source changed", stored quote kept.
        write(
            &fx.sources[0],
            "essays/2016-generative-metaphor.md",
            "---\ntitle: Generative metaphor\ndate: 2016-06-23\ngenre: essay\n---\nCompletely rewritten.\n",
        );
        build(&fx);
        let opened = open_set(&st, &set_id, &fx.engine()).unwrap();
        assert_eq!(opened.items[0].status, Status::Changed);
        assert!(opened.items[0].cited.is_none());
        assert!(opened.items[0].item.quote.contains("frames a problem"));

        // Exports go through the engine's citations.
        let out = render_export(&st, &set_id, ExportKind::Citations, &fx.engine()).unwrap();
        assert_eq!(out.file_name, "metaphor-talk-citations.md");
        assert!(out.text.starts_with("# Metaphor talk\n\n1. "));
        // Only the sets dir was written in the archive root.
        let names: Vec<_> = std::fs::read_dir(&archive).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(names, ["sets"]);
    }

    #[test]
    fn a_repair_keeps_the_users_own_frontmatter_and_prose() {
        let fx = Fixture::new("sets-prose");
        build(&fx);
        let e = fx.engine();
        let archive = fx.root.join("hs-archive");
        std::fs::create_dir_all(&archive).unwrap();
        let st = SetStore::new(archive.to_str().unwrap());
        let set_id = st.create(&SetFile::new("Talk", SetKind::Hand, None, "2026-10-20")).unwrap();
        let w = id_of(&e, "frames a problem", "writing");
        add(&st, &set_id, sets::item_from_cited(&e.cite(&CiteQuery { passage_id: w.clone() }).unwrap().body)).unwrap();
        // The user adds personal frontmatter and a paragraph by hand.
        let file = archive.join("sets").join(format!("{set_id}.md"));
        let text = std::fs::read_to_string(&file).unwrap();
        let text = text.replacen("kind: hand\n", "kind: hand\naudience: \"Oxford seminar\"\nmy-tags:\n  - keep\n", 1)
            + "\nWhy this one: it frames the whole talk.\n";
        std::fs::write(&file, &text).unwrap();
        // The passage moves down its file: opening the set repairs the id and rewrites the file.
        write(
            &fx.sources[0],
            "essays/2016-generative-metaphor.md",
            "---\ntitle: Generative metaphor\ndate: 2016-06-23\ngenre: essay\n---\n\
             A new opening paragraph that was not there before.\n\n\
             A generative metaphor frames a problem and suggests its solution.\n\n\
             Schön’s *generative metaphor* is a model of thought.\n",
        );
        build(&fx);
        let opened = open_set(&st, &set_id, &fx.engine()).unwrap();
        assert_eq!(opened.items[0].status, Status::Repaired);
        let after = std::fs::read_to_string(&file).unwrap();
        assert_ne!(after, text, "the repair was written");
        assert!(after.contains("audience: \"Oxford seminar\"\nmy-tags:\n  - keep\n"), "{after}");
        assert!(after.contains("\nWhy this one: it frames the whole talk.\n"), "{after}");
    }

    #[derive(Default)]
    struct Quotes(Vec<Found>);
    impl Lookup for Quotes {
        fn cite(&self, id: &str) -> Option<Found> {
            self.0.iter().find(|f| f.passage_id == id).cloned()
        }
        fn find_quote(&self, _c: &str, _q: &str) -> Vec<Found> {
            self.0.clone()
        }
    }

    #[test]
    fn a_highlight_is_added_from_the_selected_work_only() {
        let f = |id: &str, slug: &str| Found { passage_id: id.into(), rel_path: work_rel_path(slug), quote: "Same words".into() };
        let l = Quotes(vec![f("highlights:readings/works/first.md:5", "first"), f("highlights:readings/works/second.md:9", "second")]);
        assert_eq!(find_in_work(&l, "highlights", "Same words", "second").unwrap().passage_id, "highlights:readings/works/second.md:9");
        assert_eq!(find_in_work(&l, "highlights", "Same words", "first").unwrap().passage_id, "highlights:readings/works/first.md:5");
        assert!(find_in_work(&l, "highlights", "Same words", "third").is_none(), "never another work's passage");
    }

    #[test]
    fn an_export_is_written_only_where_the_dialog_chose() {
        let fx = Fixture::new("sets-export");
        build(&fx);
        let e = fx.engine();
        let archive = fx.root.join("hs-archive");
        std::fs::create_dir_all(&archive).unwrap();
        let st = SetStore::new(archive.to_str().unwrap());
        let set_id = st.create(&SetFile::new("Talk", SetKind::Hand, None, "d")).unwrap();
        let w = id_of(&e, "frames a problem", "writing");
        add(&st, &set_id, sets::item_from_cited(&e.cite(&CiteQuery { passage_id: w }).unwrap().body)).unwrap();
        let set_file = std::fs::read_to_string(archive.join("sets").join(format!("{set_id}.md"))).unwrap();
        // Cancelled: nothing is written anywhere.
        assert_eq!(export_to_chosen(&st, &set_id, ExportKind::Citations, &e, |_| Ok(None)).unwrap(), None);
        // Chosen: written there, under the name the dialog was offered.
        let dest = fx.root.join("chosen.md");
        let mut offered = String::new();
        let got = export_to_chosen(&st, &set_id, ExportKind::Citations, &e, |name| {
            offered = name.to_string();
            Ok(Some(dest.clone()))
        })
        .unwrap();
        assert_eq!(offered, "talk-citations.md");
        assert_eq!(got.as_deref(), Some(dest.to_str().unwrap()));
        assert!(std::fs::read_to_string(&dest).unwrap().starts_with("# Talk\n\n1. "));
        assert_eq!(std::fs::read_to_string(archive.join("sets").join(format!("{set_id}.md"))).unwrap(), set_file);
        // The command takes no path from the renderer.
        let src = include_str!("sets.rs");
        let sig = src.split("pub async fn set_export_save(").nth(1).unwrap().split(')').next().unwrap();
        assert!(!sig.contains("path"), "{sig}");
        assert!(!src.contains(&["fn set_export", "_to("].concat()));
    }

    #[test]
    fn saved_searches_take_in_new_matches_after_an_index_build() {
        let fx = Fixture::new("sets-follow");
        build(&fx);
        let archive = fx.root.join("hs-archive");
        std::fs::create_dir_all(&archive).unwrap();
        let st = SetStore::new(archive.to_str().unwrap());
        let mut s = SetFile::new("Scaffolding", SetKind::SavedSearch, Some("scaffolding".into()), "d");
        sets::refresh_saved(&mut s, saved_matches(&fx.engine(), "scaffolding").unwrap(), SAVED_CAP, "d");
        let id = st.create(&s).unwrap();
        let before = st.load(&id).unwrap().items.len();
        // A sync brings in a newly matching piece; the index is rebuilt.
        write(&fx.sources[0], "essays/2020-scaffolding.md", "---\ntitle: Scaffolding\ndate: 2020-01-01\ngenre: essay\n---\nScaffolding holds a learner up for a while.\n");
        build(&fx);
        assert_eq!(refresh_all_saved(&st, &fx.engine()), vec![id.clone()]);
        let after = st.load(&id).unwrap();
        assert_eq!(after.items.len(), before + 1);
        assert!(after.items.iter().any(|i| i.source_path == "essays/2020-scaffolding.md"));
        // Nothing new: nothing changes.
        assert!(refresh_all_saved(&st, &fx.engine()).is_empty());
    }

    #[test]
    fn a_saved_search_set_fills_from_its_query() {
        let fx = Fixture::new("sets-saved");
        build(&fx);
        let e = fx.engine();
        let matches = saved_matches(&e, "generative metaphor").unwrap();
        assert!(matches.len() >= 2, "{matches:?}");
        let mut s = SetFile::new("GM", SetKind::SavedSearch, Some("generative metaphor".into()), "d");
        assert!(sets::refresh_saved(&mut s, matches, SAVED_CAP, "d"));
        assert!(s.items.iter().all(|i| !i.quote.is_empty() && !i.pinned));
    }

    #[test]
    fn an_automatic_refresh_keeps_the_note_on_a_passage_that_moved() {
        let fx = Fixture::new("sets-moved-note");
        build(&fx);
        let archive = fx.root.join("hs-archive");
        std::fs::create_dir_all(&archive).unwrap();
        let st = SetStore::new(archive.to_str().unwrap());
        let mut s = SetFile::new("Frames", SetKind::SavedSearch, Some("frames".into()), "d");
        sets::refresh_saved(&mut s, saved_matches(&fx.engine(), "frames").unwrap(), SAVED_CAP, "d");
        let id = st.create(&s).unwrap();
        let old = st.load(&id).unwrap().items[0].passage.clone();
        // A note and a paragraph of his own on the (unpinned) match.
        st.update(&id, |s| {
            sets::apply_edit(s, &SetEdit::Note { passage: old.clone(), note: "Open the talk with this".into() }, "d")?;
            s.items[0].extra = "Why: it frames the talk.".into();
            Ok(())
        })
        .unwrap();
        // Text is inserted above the passage; the index is rebuilt; saved searches refresh.
        write(
            &fx.sources[0],
            "essays/2016-generative-metaphor.md",
            "---\ntitle: Generative metaphor\ndate: 2016-06-23\ngenre: essay\n---\n\
             A new opening paragraph that was not there before.\n\n\
             A generative metaphor frames a problem and suggests its solution.\n\n\
             Schön’s *generative metaphor* is a model of thought.\n",
        );
        build(&fx);
        refresh_all_saved(&st, &fx.engine());
        let after = st.load(&id).unwrap();
        let it = after.items.iter().find(|i| i.quote.contains("frames a problem")).expect("the item is still there");
        assert_ne!(it.passage, old, "under its passage's new id");
        assert_eq!(it.note, "Open the talk with this");
        assert_eq!(it.extra, "Why: it frames the talk.");
        assert_eq!(after.items.iter().filter(|i| i.quote.contains("frames a problem")).count(), 1);
    }
}
