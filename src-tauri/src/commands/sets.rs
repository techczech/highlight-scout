//! Tauri commands for sets (HS-2A), "more like these" and the ArchiveScout
//! hand-off check. Each runs on the blocking pool. The logic and its tests
//! live in `crate::sets`, `crate::sets_export` and `crate::handoff`; this
//! file only connects them to the engine (`cite`, `verify-quote`, `similar`,
//! `search`) and to the app's archive path.

use scout_corpus::api::{CiteQuery, SearchQuery, SimilarQuery, VerifyQuoteQuery};
use scout_corpus::{CitedPassage, Engine, SearchMode};
use serde::Serialize;
use tauri::{AppHandle, Manager};

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
}

#[derive(Debug, Serialize)]
pub struct OpenedSet {
    pub id: String,
    pub title: String,
    pub kind: SetKind,
    pub query: Option<String>,
    pub items: Vec<ResolvedItem>,
}

/// Resolve every item of a set; ids that were repaired are written back.
/// When the engine is unavailable the items come back unresolved ("changed"
/// without a citation is wrong), so an engine error is an error.
fn open_set(st: &SetStore, id: &str, e: &Engine) -> Result<OpenedSet, String> {
    let mut set = st.load(id)?;
    let lookup = EngineLookup(e);
    let mut repaired = false;
    let mut items = vec![];
    for it in set.items.iter_mut() {
        let (status, found) = sets::resolve(it, &lookup);
        if status == Status::Repaired {
            if let Some(f) = &found {
                sets::apply_repair(it, f);
                repaired = true;
            }
        }
        let cited = found.and_then(|f| e.cite(&CiteQuery { passage_id: f.passage_id }).ok().map(|r| r.body));
        items.push(ResolvedItem { item: it.clone(), status, cited });
    }
    if repaired {
        st.save(id, &set)?;
    }
    Ok(OpenedSet { id: id.to_string(), title: set.title, kind: set.kind, query: set.query, items })
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
        Ok(SetSummary { id, title: set.title, kind: set.kind, items: set.items.len(), updated: set.updated })
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

/// Add a highlight the classic highlight list shows (no passage id there):
/// found in the highlights corpus by its text.
#[tauri::command]
pub async fn set_add_text(app: AppHandle, id: String, corpus: String, text: String) -> Result<SetSummary, String> {
    let st = store(&app);
    blocking(move || {
        let e = engine()?;
        let found = EngineLookup(&e).find_quote(&corpus, &text);
        let f = found.first().ok_or("that highlight is not in the search index yet")?;
        let c = e.cite(&CiteQuery { passage_id: f.passage_id.clone() }).map_err(|e| format!("{e:#}"))?.body;
        add(&st, &id, sets::item_from_cited(&c))
    })
    .await
}

fn add(st: &SetStore, id: &str, item: SetItem) -> Result<SetSummary, String> {
    let mut set = st.load(id)?;
    sets::add_item(&mut set, item, &today());
    st.save(id, &set)?;
    Ok(SetSummary { id: id.to_string(), title: set.title, kind: set.kind, items: set.items.len(), updated: set.updated })
}

/// Reorder, annotate, remove, pin or rename.
#[tauri::command]
pub async fn set_edit(app: AppHandle, id: String, edit: SetEdit) -> Result<SetSummary, String> {
    let st = store(&app);
    blocking(move || {
        let mut set = st.load(&id)?;
        sets::apply_edit(&mut set, &edit, &today())?;
        st.save(&id, &set)?;
        Ok(SetSummary { id, title: set.title, kind: set.kind, items: set.items.len(), updated: set.updated })
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
        let mut set = st.load(&id)?;
        sets::dismiss(&mut set, &passage_id, &today());
        st.save(&id, &set)
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

/// A saved-search set: pinned items stay, the rest follow the query.
#[tauri::command]
pub async fn set_refresh(app: AppHandle, id: String) -> Result<OpenedSet, String> {
    let st = store(&app);
    blocking(move || {
        let e = engine()?;
        let mut set = st.load(&id)?;
        if let Some(q) = set.query.clone() {
            let matches = saved_matches(&e, &q)?;
            if sets::refresh_saved(&mut set, matches, SAVED_CAP, &today()) {
                st.save(&id, &set)?;
            }
        }
        open_set(&st, &id, &e)
    })
    .await
}

#[derive(Debug, Serialize)]
pub struct ExportOut {
    pub file_name: String,
    pub text: String,
}

fn render_export(st: &SetStore, id: &str, kind: ExportKind) -> Result<ExportOut, String> {
    let e = engine()?;
    let opened = open_set(st, id, &e)?;
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
    blocking(move || render_export(&st, &id, kind)).await
}

/// Write an export to the file the user chose in the save dialog.
#[tauri::command]
pub async fn set_export_to(app: AppHandle, id: String, kind: ExportKind, path: String) -> Result<String, String> {
    let st = store(&app);
    blocking(move || {
        let out = render_export(&st, &id, kind)?;
        std::fs::write(&path, out.text).map_err(|e| format!("{path}: {e}"))?;
        Ok(path)
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
        let out = render_export(&st, &set_id, ExportKind::Citations).unwrap();
        assert_eq!(out.file_name, "metaphor-talk-citations.md");
        assert!(out.text.starts_with("# Metaphor talk\n\n1. "));
        // Only the sets dir was written in the archive root.
        let names: Vec<_> = std::fs::read_dir(&archive).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(names, ["sets"]);
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
}
