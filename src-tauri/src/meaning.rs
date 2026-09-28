//! The meaning index: passage vectors for semantic and hybrid search over
//! every Scout corpus (scout-core v0.3.0, `Engine::with_embedder`).
//!
//! [`Meaning`] is the app's one handle on it:
//! - it owns the embedder for the whole app session, so the model loads once
//!   (on the first embed), not per query;
//! - [`Meaning::state`] says, per corpus, whether its vectors are `current`,
//!   `stale` or `missing`, with the time and size a build would take;
//! - [`Meaning::build`] builds the vectors in the background, one run at a
//!   time, reporting progress. Nothing else ever starts a build, and no
//!   search downloads the model: [`Meaning::search_engine`] refuses while
//!   the model is not on disk.
//!
//! Every call is blocking (SQLite, the model); the commands run them on the
//! blocking pool.

use crate::corpus::{CorpusError, INDEX_WRITE};
use scout_corpus::api::{BuildEvent, IndexBuildQuery};
use scout_corpus::semantic::store::{self, Freshness};
use scout_corpus::{index, Corpus, Embedder, Engine};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Bytes of vector store per indexed passage (measured on the three corpora
/// on Macek, 2026-09-28: 265 MB for 127,810 passages; short passages get none).
const BYTES_PER_PASSAGE: f64 = 2_080.0;
/// Passages embedded per second by paraphrase-multilingual-MiniLM-L12-v2 on
/// Macek (about 9 min for the same 127,810).
const PASSAGES_PER_SECOND: f64 = 235.0;

/// The embedding model the app uses: `SCOUT_EMBEDDER=hash` gives the
/// deterministic fake (tests, no model); otherwise the ONNX model
/// (`SCOUT_EMBED_MODEL`, default minilm-l12) in [`model_dir`].
pub fn embedder_from_env() -> Option<Arc<dyn Embedder>> {
    if std::env::var("SCOUT_EMBEDDER").as_deref() == Ok("hash") {
        return Some(Arc::new(scout_corpus::HashEmbedder::default()));
    }
    let key = std::env::var("SCOUT_EMBED_MODEL")
        .unwrap_or_else(|_| scout_corpus::semantic::embed::DEFAULT_MODEL.into());
    scout_corpus::semantic::embed::OnnxEmbedder::new(&key, model_dir())
        .ok()
        .map(|e| Arc::new(e) as Arc<dyn Embedder>)
}

/// Where the model files are looked for (and downloaded to, on an explicit
/// build): `SCOUT_MODEL_DIR`, else `HF_HOME`, else `~/local-models` when it
/// holds a Hugging Face cache (a Finder-launched app does not see the
/// shell's `HF_HOME`), else scout's own models dir.
pub fn model_dir() -> PathBuf {
    for var in ["SCOUT_MODEL_DIR", "HF_HOME"] {
        if let Some(v) = std::env::var_os(var).filter(|v| !v.is_empty()) {
            return PathBuf::from(v);
        }
    }
    if let Some(home) = dirs::home_dir() {
        let local = home.join("local-models");
        let has_cache = std::fs::read_dir(&local)
            .map(|rd| {
                rd.flatten()
                    .any(|e| e.file_name().to_string_lossy().starts_with("models--"))
            })
            .unwrap_or(false);
        if has_cache {
            return local;
        }
    }
    index::models_dir()
}

/// One corpus's vectors.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CorpusVectors {
    pub corpus: String,
    /// `current` | `stale` (the index changed since, or another model) |
    /// `missing` | `no_index` (the corpus has no full-text index yet).
    pub state: &'static str,
    pub why: Option<String>,
    /// Indexed passages (what a build embeds, short ones excepted).
    pub passages: i64,
}

/// What building the vectors that are not current would take.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Estimate {
    pub corpora: Vec<String>,
    pub minutes: u64,
    pub megabytes: u64,
    /// The one-off model download, when the model is not on disk yet.
    pub download: Option<String>,
}

/// What a meaning-index build is doing or last did; emitted as `corpus:meaning`.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MeaningJob {
    /// `idle` | `building` | `built` | `failed`.
    pub phase: &'static str,
    pub corpora: Vec<String>,
    /// The corpus being embedded, and how far it is.
    pub corpus: Option<String>,
    pub done: usize,
    pub total: usize,
    pub message: String,
}

impl Default for MeaningJob {
    fn default() -> Self {
        MeaningJob {
            phase: "idle",
            corpora: vec![],
            corpus: None,
            done: 0,
            total: 0,
            message: String::new(),
        }
    }
}

/// The meaning index's state for the UI (`corpus_meaning_state`).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MeaningState {
    /// The app has an embedding model at all.
    pub available: bool,
    pub model: Option<String>,
    /// The model files are on disk (no download needed).
    pub model_ready: bool,
    pub corpora: Vec<CorpusVectors>,
    /// For the corpora whose vectors are not current; `None` when all are.
    pub estimate: Option<Estimate>,
    pub job: MeaningJob,
    pub running: bool,
}

/// Minutes and megabytes to embed `passages`.
pub fn estimate(passages: i64) -> (u64, u64) {
    let p = passages.max(0) as f64;
    let minutes = (p / PASSAGES_PER_SECOND / 60.0).ceil() as u64;
    let mb = (p * BYTES_PER_PASSAGE / 1_000_000.0).ceil() as u64;
    (minutes.max(1), mb.max(1))
}

fn note_line(note: &str) -> String {
    note.trim_start_matches("scout: note: ").to_string()
}

struct Running<'a>(&'a AtomicBool);
impl Drop for Running<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// The session's embedder and its one background build.
pub struct Meaning {
    embedder: Option<Arc<dyn Embedder>>,
    running: AtomicBool,
    last: Mutex<MeaningJob>,
}

impl Meaning {
    pub fn new(embedder: Option<Arc<dyn Embedder>>) -> Meaning {
        Meaning {
            embedder,
            running: AtomicBool::new(false),
            last: Mutex::new(MeaningJob::default()),
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn job(&self) -> MeaningJob {
        self.last.lock().map(|j| j.clone()).unwrap_or_default()
    }

    /// The model files are on disk: using it downloads nothing.
    pub fn model_ready(&self) -> bool {
        self.embedder
            .as_ref()
            .is_some_and(|e| e.download_note().is_none())
    }

    /// `engine` with the session's embedder when the model is on disk
    /// (index builds then keep existing vectors current), else unchanged.
    pub fn keeper_engine(&self, engine: Engine) -> Engine {
        match &self.embedder {
            Some(e) if self.model_ready() => engine.with_embedder(e.clone()),
            _ => engine,
        }
    }

    /// `engine` ready for semantic or hybrid search, or why it cannot be.
    pub fn search_engine(&self, engine: Engine) -> Result<Engine, CorpusError> {
        let Some(e) = &self.embedder else {
            return Err(unavailable("This build of the app has no embedding model."));
        };
        if !self.model_ready() {
            return Err(unavailable(
                "Semantic search needs the meaning index, which is not built yet.",
            ));
        }
        Ok(engine.with_embedder(e.clone()))
    }

    /// Each corpus's vectors, and what building the rest would take.
    pub fn state(&self, engine: &Engine) -> MeaningState {
        let model = self.embedder.as_ref().map(|e| e.model_id());
        let status = engine.index_status().ok().map(|r| r.body.corpora);
        let passages = |id: &str| {
            status
                .as_ref()
                .and_then(|s| s.iter().find(|c| c.corpus == id))
                .map(|c| c.passages)
                .unwrap_or(0)
        };
        let corpora: Vec<CorpusVectors> = engine
            .registry()
            .corpora
            .iter()
            .map(|cfg| {
                let c = Corpus::from_config(cfg.clone(), engine.index_dir());
                let (state, why) = vectors_state(&c, model.as_deref());
                CorpusVectors {
                    corpus: cfg.id.clone(),
                    state,
                    why,
                    passages: passages(&cfg.id),
                }
            })
            .collect();
        let todo: Vec<&CorpusVectors> = corpora
            .iter()
            .filter(|c| c.state == "missing" || c.state == "stale")
            .collect();
        let download = self
            .embedder
            .as_ref()
            .and_then(|e| e.download_note())
            .map(|n| note_line(&n));
        let estimate = if todo.is_empty() && download.is_none() {
            None
        } else {
            let (minutes, megabytes) = estimate(todo.iter().map(|c| c.passages).sum());
            Some(Estimate {
                corpora: todo.iter().map(|c| c.corpus.clone()).collect(),
                minutes: if todo.is_empty() { 0 } else { minutes },
                megabytes: if todo.is_empty() { 0 } else { megabytes },
                download,
            })
        };
        MeaningState {
            available: self.embedder.is_some(),
            model,
            model_ready: self.model_ready(),
            corpora,
            estimate,
            job: self.job(),
            running: self.is_running(),
        }
    }

    fn set(&self, job: MeaningJob, emit: &dyn Fn(&MeaningJob)) {
        if let Ok(mut l) = self.last.lock() {
            *l = job.clone();
        }
        emit(&job);
    }

    /// Build the vectors of `ids` (empty: every corpus whose vectors are not
    /// current), downloading the model first if it is not on disk. Emits
    /// progress at most every `every`. Returns `None` at once when a build is
    /// already running. Blocking.
    pub fn build(
        &self,
        engine: impl FnOnce() -> Result<Engine, CorpusError>,
        ids: Vec<String>,
        every: Duration,
        emit: &dyn Fn(&MeaningJob),
    ) -> Option<MeaningJob> {
        if self.running.swap(true, Ordering::SeqCst) {
            return None;
        }
        let _guard = Running(&self.running);
        let failed = |message: String| MeaningJob {
            phase: "failed",
            message,
            ..MeaningJob::default()
        };
        let Some(embedder) = self.embedder.clone() else {
            let job = failed("This build of the app has no embedding model.".into());
            self.set(job.clone(), emit);
            return Some(job);
        };
        let engine = match engine() {
            Ok(e) => e.with_embedder(embedder),
            Err(e) => {
                let job = failed(format!("Meaning index unavailable: {e}"));
                self.set(job.clone(), emit);
                return Some(job);
            }
        };
        let ids = if ids.is_empty() {
            self.state(&engine)
                .corpora
                .into_iter()
                .filter(|c| c.state != "current")
                .map(|c| c.corpus)
                .collect()
        } else {
            ids
        };
        if ids.is_empty() {
            let job = MeaningJob {
                phase: "built",
                message: "The meaning index is up to date".into(),
                ..MeaningJob::default()
            };
            self.set(job.clone(), emit);
            return Some(job);
        }
        let started = MeaningJob {
            phase: "building",
            corpora: ids.clone(),
            message: format!("Building the meaning index: {}…", ids.join(", ")),
            ..MeaningJob::default()
        };
        self.set(started.clone(), emit);
        let mut last_emit = Instant::now();
        let mut on = |ev: BuildEvent| {
            let job = match ev {
                BuildEvent::Download(note) => MeaningJob {
                    message: note_line(&note),
                    ..started.clone()
                },
                BuildEvent::Embedding {
                    corpus,
                    done,
                    total,
                } => {
                    if done < total && last_emit.elapsed() < every {
                        return;
                    }
                    MeaningJob {
                        message: format!(
                            "Building the meaning index: {corpus} {done} of {total} passages…"
                        ),
                        corpus: Some(corpus),
                        done,
                        total,
                        ..started.clone()
                    }
                }
            };
            last_emit = Instant::now();
            self.set(job, emit);
        };
        let result = {
            let _write = INDEX_WRITE.lock().unwrap_or_else(|p| p.into_inner());
            engine.index_build_with(
                &IndexBuildQuery {
                    ids: ids.clone(),
                    force: false,
                    semantic: true,
                },
                &mut on,
            )
        };
        let job = match result {
            Ok(r) => {
                let embedded: usize = r
                    .body
                    .reports
                    .iter()
                    .filter_map(|b| b.vectors.as_ref().map(|v| v.embedded))
                    .sum();
                MeaningJob {
                    phase: "built",
                    corpora: ids,
                    message: format!(
                        "Meaning index built: {} ({embedded} passages embedded)",
                        r.body
                            .reports
                            .iter()
                            .map(|b| b.corpus.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    ..MeaningJob::default()
                }
            }
            Err(e) => MeaningJob {
                corpora: ids,
                ..failed(format!("Meaning index build failed: {e:#}"))
            },
        };
        self.set(job.clone(), emit);
        Some(job)
    }
}

fn unavailable(message: &str) -> CorpusError {
    CorpusError {
        kind: "meaning_unavailable",
        message: message.into(),
    }
}

/// A corpus's vectors against its full-text index under `model`.
fn vectors_state(c: &Corpus, model: Option<&str>) -> (&'static str, Option<String>) {
    if !c.index_path().exists() {
        return ("no_index", None);
    }
    let Some(model) = model else {
        return ("missing", None);
    };
    let conn = match index::open_existing(&c.config, c.index_path()) {
        Ok(c) => c,
        Err(e) => return ("no_index", Some(format!("{e:#}"))),
    };
    match store::freshness(&conn, &c.vectors_path(), model) {
        Ok(Freshness::Current) => ("current", None),
        Ok(Freshness::Missing) => ("missing", None),
        Ok(Freshness::Stale(why)) => ("stale", Some(why)),
        Err(e) => ("stale", Some(format!("{e:#}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::fixture::Fixture;
    use crate::corpus::{search, IndexKeeper};
    use scout_corpus::api::SearchQuery;
    use scout_corpus::{HashEmbedder, SearchMode};

    fn hash() -> Meaning {
        Meaning::new(Some(Arc::new(HashEmbedder::default())))
    }

    fn states(m: &Meaning, e: &Engine) -> Vec<(String, &'static str)> {
        m.state(e)
            .corpora
            .into_iter()
            .map(|c| (c.corpus, c.state))
            .collect()
    }

    fn built(fx: &Fixture) {
        IndexKeeper::default()
            .refresh(|| Ok(fx.engine()), &|_| {})
            .unwrap();
    }

    #[test]
    fn estimate_matches_the_three_corpora_on_this_mac() {
        // 127,810 passages: about 9 minutes and 265 MB (ticket 09).
        let (min, mb) = estimate(127_810);
        assert!((9..=10).contains(&min), "{min}");
        assert!((250..=280).contains(&mb), "{mb}");
        assert_eq!(estimate(0), (1, 1));
    }

    #[test]
    fn vectors_go_missing_building_current_then_stale_after_an_edit() {
        let fx = Fixture::new("meaning-states");
        let m = hash();
        let st = m.state(&fx.engine());
        assert!(st.corpora.iter().all(|c| c.state == "no_index"));

        built(&fx);
        let st = m.state(&fx.engine());
        assert_eq!(
            states(&m, &fx.engine()),
            vec![
                ("writing".into(), "missing"),
                ("tweets".into(), "missing"),
                ("highlights".into(), "missing"),
            ]
        );
        let est = st.estimate.unwrap();
        assert_eq!(est.corpora, vec!["writing", "tweets", "highlights"]);
        assert_eq!(est.download, None, "the hash fake needs no download");
        assert!(st.model_ready);

        // One build, with progress, then every corpus current.
        let seen = Mutex::new(vec![]);
        let job = m
            .build(|| Ok(fx.engine()), vec![], Duration::ZERO, &|j| {
                seen.lock().unwrap().push(j.phase)
            })
            .unwrap();
        let seen = seen.into_inner().unwrap();
        assert_eq!(seen.first(), Some(&"building"));
        assert_eq!(seen.last(), Some(&"built"));
        assert_eq!(job.phase, "built", "{}", job.message);
        assert_eq!(job.corpora, vec!["writing", "tweets", "highlights"]);
        assert!(!m.is_running());
        let st = m.state(&fx.engine());
        assert!(st.corpora.iter().all(|c| c.state == "current"), "{:?}", st.corpora);
        assert_eq!(st.estimate, None);

        // A second build has nothing to do.
        let job = m.build(|| Ok(fx.engine()), vec![], Duration::ZERO, &|_| {}).unwrap();
        assert_eq!(job.message, "The meaning index is up to date");

        // An edit rebuilt by a keeper WITHOUT the embedder leaves writing stale…
        std::thread::sleep(Duration::from_millis(20));
        crate::corpus::fixture::write(
            &fx.sources[0],
            "essays/2016-generative-metaphor.md",
            "---\ntitle: Generative metaphor\ndate: 2016-06-23\ngenre: essay\n---\n\
             A generative metaphor frames a problem and suggests its solution. Revised text here.\n",
        );
        built(&fx);
        assert_eq!(states(&m, &fx.engine())[0], ("writing".into(), "stale"));
        // …and a keeper WITH it keeps the vectors current.
        std::thread::sleep(Duration::from_millis(20));
        crate::corpus::fixture::write(
            &fx.sources[0],
            "essays/2016-generative-metaphor.md",
            "---\ntitle: Generative metaphor\ndate: 2016-06-23\ngenre: essay\n---\n\
             A generative metaphor frames a problem and suggests its solution. Revised again here.\n",
        );
        m.build(|| Ok(fx.engine()), vec![], Duration::ZERO, &|_| {}).unwrap();
        std::thread::sleep(Duration::from_millis(20));
        crate::corpus::fixture::write(
            &fx.sources[0],
            "essays/2016-generative-metaphor.md",
            "---\ntitle: Generative metaphor\ndate: 2016-06-23\ngenre: essay\n---\n\
             A generative metaphor frames a problem and suggests a solution. Third revision.\n",
        );
        IndexKeeper::default()
            .refresh(|| Ok(m.keeper_engine(fx.engine())), &|_| {})
            .unwrap();
        assert_eq!(states(&m, &fx.engine())[0], ("writing".into(), "current"));
    }

    #[test]
    fn semantic_search_spans_writing_and_highlights_once_vectors_exist() {
        let fx = Fixture::new("meaning-search");
        let m = hash();
        built(&fx);
        let q = |mode| SearchQuery {
            query: "a generative metaphor is seeing one thing as another".into(),
            in_: vec!["writing".into(), "highlights".into()],
            mode,
            ..Default::default()
        };

        // No vectors yet: semantic says why; hybrid falls back to full text with a note.
        let e = search(&m.search_engine(fx.engine()).unwrap(), &q(SearchMode::Semantic)).unwrap_err();
        assert_eq!(e.kind, "meaning_unavailable", "{}", e.message);

        m.build(|| Ok(fx.engine()), vec![], Duration::ZERO, &|_| {}).unwrap();
        let r = search(&m.search_engine(fx.engine()).unwrap(), &q(SearchMode::Semantic)).unwrap();
        assert_eq!(r.body.mode.as_deref(), Some("semantic"));
        let mut corpora: Vec<&str> = r.body.results.iter().map(|d| d.corpus.as_str()).collect();
        corpora.sort();
        corpora.dedup();
        assert_eq!(corpora, vec!["highlights", "writing"]);

        let r = search(&m.search_engine(fx.engine()).unwrap(), &q(SearchMode::Hybrid)).unwrap();
        assert_eq!(r.body.mode.as_deref(), Some("hybrid"));

        // Keyword (auto from the app) is full text even with an embedder attached.
        let r = search(&m.search_engine(fx.engine()).unwrap(), &q(SearchMode::Auto)).unwrap();
        assert_eq!(r.body.mode, None);
    }

    /// The ticket's check on the real archive and model (never downloads:
    /// `search_engine` refuses without the model on disk).
    /// `cargo test real_archive -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_archive_the_mind_as_a_machine_spans_writing_and_highlights() {
        let m = Meaning::new(embedder_from_env());
        let engine = crate::corpus::engine_from_env().unwrap();
        let st = m.state(&engine);
        eprintln!("model dir {:?}; {:?}", model_dir(), st.corpora);
        let engine = m.search_engine(engine).unwrap();
        for mode in [SearchMode::Hybrid, SearchMode::Semantic] {
            let t = Instant::now();
            let r = search(
                &engine,
                &SearchQuery {
                    query: "the mind as a machine".into(),
                    in_: vec!["writing".into(), "highlights".into()],
                    mode,
                    ..Default::default()
                },
            )
            .unwrap();
            let corpora: std::collections::BTreeSet<&str> =
                r.body.results.iter().map(|d| d.corpus.as_str()).collect();
            eprintln!("{mode:?} {:?} in {:?}: {corpora:?}", r.body.mode, t.elapsed());
            assert!(corpora.contains("writing") && corpora.contains("highlights"));
        }
    }

    #[test]
    fn no_model_on_disk_means_no_search_and_no_silent_download() {
        struct NotDownloaded;
        impl Embedder for NotDownloaded {
            fn model_id(&self) -> String {
                "fake@1".into()
            }
            fn embed_passages(&self, _: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
                panic!("must not embed")
            }
            fn embed_query(&self, _: &str) -> anyhow::Result<Vec<f32>> {
                panic!("must not embed")
            }
            fn download_note(&self) -> Option<String> {
                Some("scout: note: downloading embedding model fake (about 490 MB) once to /x".into())
            }
        }
        let fx = Fixture::new("meaning-nomodel");
        let m = Meaning::new(Some(Arc::new(NotDownloaded)));
        built(&fx);
        assert!(!m.model_ready());
        let e = m.search_engine(fx.engine()).unwrap_err();
        assert_eq!(e.kind, "meaning_unavailable");
        // The keeper's engine gets no embedder, so an index build embeds nothing.
        assert!(m.keeper_engine(fx.engine()).embedder().is_none());
        let st = m.state(&fx.engine());
        assert_eq!(
            st.estimate.unwrap().download.as_deref(),
            Some("downloading embedding model fake (about 490 MB) once to /x")
        );

        let none = Meaning::new(None);
        assert!(!none.state(&fx.engine()).available);
        assert_eq!(
            none.build(|| Ok(fx.engine()), vec![], Duration::ZERO, &|_| {}).unwrap().phase,
            "failed"
        );
    }
}
