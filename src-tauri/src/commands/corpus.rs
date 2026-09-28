//! Tauri commands for archive search over the Scout corpora. Each one runs
//! its facade call on the blocking pool (never the main thread) via
//! `crate::corpus`, which holds the logic and the tests.

use scout_corpus::api::{IndexStatusReport, SearchQuery};
use scout_corpus::{CitedPassage, SearchMode, SearchResults};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::corpus::{self, Answer, CorpusError, IndexJob};
use crate::meaning::{MeaningJob, MeaningState};
use crate::AppState;

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, CorpusError> + Send + 'static,
) -> Result<T, CorpusError> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .unwrap_or_else(|e| {
            Err(CorpusError {
                kind: "other",
                message: format!("archive search task failed: {e}"),
            })
        })
}

/// `scout search` over the named corpora (`query.in`; empty = every indexed
/// corpus). Takes the facade's `SearchQuery` as JSON, returns its results.
/// `mode` `semantic` or `hybrid` ranks by the meaning index (the session's
/// one model); anything else is full text.
#[tauri::command]
pub async fn corpus_search(
    app: AppHandle,
    query: SearchQuery,
) -> Result<Answer<SearchResults>, CorpusError> {
    blocking(move || {
        let engine = corpus::engine_from_env()?;
        let engine = match query.mode {
            SearchMode::Semantic | SearchMode::Hybrid => {
                app.state::<AppState>().meaning.search_engine(engine)?
            }
            _ => engine,
        };
        corpus::search(&engine, &query)
    })
    .await
}

/// The meaning index per corpus (current, stale, missing), what building the
/// rest would take, and the background build's progress.
#[tauri::command]
pub async fn corpus_meaning_state(app: AppHandle) -> Result<MeaningState, CorpusError> {
    blocking(move || {
        let engine = corpus::engine_from_env()?;
        Ok(app.state::<AppState>().meaning.state(&engine))
    })
    .await
}

/// "Build meaning index": start a background build of `corpora` (empty =
/// every corpus whose vectors are not current); progress arrives as
/// `corpus:meaning` events. Returns at once with the current job. Only the
/// user starts this: nothing builds vectors on its own.
#[tauri::command]
pub async fn corpus_meaning_build(app: AppHandle, corpora: Vec<String>) -> Result<MeaningJob, String> {
    let bg = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = bg.state::<AppState>();
        let job = state.meaning.build(
            corpus::engine_from_env,
            corpora,
            std::time::Duration::from_millis(400),
            &|job| {
                let _ = bg.emit("corpus:meaning", job);
            },
        );
        // Fresh vectors change what the rail and searches can use.
        if matches!(job, Some(ref j) if j.phase == "built") {
            let _ = bg.emit("corpus:meaning-built", ());
        }
    });
    Ok(app.state::<AppState>().meaning.job())
}

/// `scout cite <passage-id>`: the whole passage in its original text, with
/// the source file and line.
#[tauri::command]
pub async fn corpus_cite(passage_id: String) -> Result<Answer<CitedPassage>, CorpusError> {
    blocking(move || corpus::cite(&corpus::engine_from_env()?, &passage_id)).await
}

#[derive(Debug, Clone, Serialize)]
pub struct CorpusIndexState {
    /// `None` when the registry or status could not be read (see `job`).
    pub status: Option<IndexStatusReport>,
    pub job: IndexJob,
    pub running: bool,
}

/// `scout index status` plus what the background keeper is doing.
#[tauri::command]
pub async fn corpus_index_status(app: AppHandle) -> Result<CorpusIndexState, CorpusError> {
    let status = blocking(|| corpus::status(&corpus::engine_from_env()?))
        .await
        .ok()
        .map(|a| a.body);
    let state = app.state::<AppState>();
    Ok(CorpusIndexState {
        status,
        job: state.corpus_index.job(),
        running: state.corpus_index.is_running(),
    })
}

/// Start a background check + incremental `index build` of every missing or
/// stale corpus index; progress arrives as `corpus:index` events. Returns at
/// once with the current job (a refresh already running is left to finish).
#[tauri::command]
pub async fn corpus_index_refresh(app: AppHandle) -> Result<IndexJob, String> {
    spawn_refresh(app.clone());
    Ok(app.state::<AppState>().corpus_index.job())
}

pub fn spawn_refresh(app: AppHandle) {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let engine = || Ok(state.meaning.keeper_engine(corpus::engine_from_env()?));
        state.corpus_index.refresh(engine, &|job| {
            let _ = app.emit("corpus:index", job);
        });
    });
}

/// The quick finder's reading pane and ⌘⇧C: the cited passage, its citation
/// as HTML for rich-text targets, and the paragraph before it (writing).
#[tauri::command]
pub async fn corpus_passage(
    passage_id: String,
) -> Result<Answer<crate::corpus_copy::PassageView>, CorpusError> {
    blocking(move || crate::corpus_copy::passage(&corpus::engine_from_env()?, &passage_id)).await
}

/// The rail's counts: documents per corpus (`index status`), and works per
/// source for the highlights archive.
#[tauri::command]
pub async fn corpus_counts() -> Result<Vec<crate::corpus_copy::CorpusCount>, CorpusError> {
    blocking(|| crate::corpus_copy::counts(&corpus::engine_from_env()?)).await
}

/// The bundle id of the app the user came from (frontmost other than this
/// one), so "Auto" copies Markdown into WriteFlex and rich text elsewhere.
#[tauri::command]
pub async fn frontmost_other_app(app: AppHandle) -> Result<Option<String>, String> {
    let own = app.config().identifier.clone();
    tauri::async_runtime::spawn_blocking(move || crate::frontmost::other_frontmost_app(&own))
        .await
        .map_err(|e| e.to_string())
}

/// Esc in the quick finder: hide the app so focus returns to the app the
/// user came from (the window itself stays open).
#[tauri::command]
pub fn quick_finder_hide(app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        app.hide().map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        app.get_webview_window("main")
            .map(|w| w.minimize().map_err(|e| e.to_string()))
            .unwrap_or(Ok(()))
    }
}
