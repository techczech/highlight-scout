//! Archive search over the Scout corpora (writing, tweets, highlights) through
//! the scout-corpus facade (`scout_corpus::api::Engine`).
//!
//! The app calls the same library functions as the `scout` CLI (no search,
//! citation or indexing logic lives here); this module only
//! - maps facade errors to a typed [`CorpusError`] the UI can act on,
//! - keeps the indexes fresh: [`IndexKeeper::refresh`] checks `index status`
//!   and runs an incremental `index build` for every corpus whose index is
//!   missing or stale, one run at a time, reporting each step.
//!
//! Every call here is blocking (it opens SQLite files and walks the corpus
//! roots); the Tauri commands run them on the blocking thread pool.
//! Sources are only read; the index files live in scout's own index dir.

use scout_corpus::api::{CiteQuery, IndexBuildQuery, IndexStatusReport, SearchQuery};
use scout_corpus::{
    BuildReport, CitedPassage, Engine, IndexMissing, IndexStatus, NoIndexedCorpus, PassageNotFound,
    RegistryMissing, SearchResults,
};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// A facade error, classified so the UI can say what to do.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CorpusError {
    /// `registry_missing` | `index_missing` | `no_indexed_corpus` |
    /// `passage_not_found` | `other`.
    pub kind: &'static str,
    pub message: String,
}

impl CorpusError {
    pub fn from_anyhow(e: &anyhow::Error) -> CorpusError {
        let kind = if e.downcast_ref::<RegistryMissing>().is_some() {
            "registry_missing"
        } else if e.downcast_ref::<IndexMissing>().is_some() {
            "index_missing"
        } else if e.downcast_ref::<NoIndexedCorpus>().is_some() {
            "no_indexed_corpus"
        } else if e.downcast_ref::<PassageNotFound>().is_some() {
            "passage_not_found"
        } else {
            "other"
        };
        CorpusError {
            kind,
            message: format!("{e:#}"),
        }
    }

    /// Whether an index build could fix this error.
    pub fn wants_index(&self) -> bool {
        matches!(self.kind, "index_missing" | "no_indexed_corpus")
    }
}

impl std::fmt::Display for CorpusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// A facade response with the notes the CLI prints on stderr
/// (e.g. `scout: note: using writing; not indexed: tweets`).
#[derive(Debug, Clone, Serialize)]
pub struct Answer<T> {
    pub body: T,
    pub notes: Vec<String>,
}

fn answer<T>(r: anyhow::Result<scout_corpus::Reply<T>>) -> Result<Answer<T>, CorpusError> {
    r.map(|r| Answer {
        body: r.body,
        notes: r.notes,
    })
    .map_err(|e| CorpusError::from_anyhow(&e))
}

/// The engine over the user's registry and index dir, exactly as the CLI
/// loads them (`SCOUT_CONFIG`, `SCOUT_DATA_DIR` honoured). Loaded per call so
/// a registry edit takes effect without a restart.
pub fn engine_from_env() -> Result<Engine, CorpusError> {
    Engine::from_env().map_err(|e| CorpusError::from_anyhow(&e))
}

/// `scout search <query> [--in …] --json`.
pub fn search(engine: &Engine, q: &SearchQuery) -> Result<Answer<SearchResults>, CorpusError> {
    answer(engine.search(q))
}

/// `scout cite <passage-id> --json`: the whole passage, original text.
pub fn cite(engine: &Engine, passage_id: &str) -> Result<Answer<CitedPassage>, CorpusError> {
    answer(engine.cite(&CiteQuery {
        passage_id: passage_id.to_string(),
    }))
}

/// `scout index status --json`.
pub fn status(engine: &Engine) -> Result<Answer<IndexStatusReport>, CorpusError> {
    answer(engine.index_status())
}

/// Why a corpus index needs an incremental build, or `None` when it is current.
/// A corpus whose root is missing reports no stale counts and is left alone
/// once it has an index (a build could only fail).
pub fn build_reason(st: &IndexStatus) -> Option<&'static str> {
    if !st.exists {
        return Some("missing");
    }
    if !st.config_current {
        return Some("engine or registry changed");
    }
    match &st.stale {
        Some(s) if s.added + s.changed + s.removed > 0 => Some("stale"),
        _ => None,
    }
}

/// One corpus the keeper rebuilt, or failed to.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CorpusBuild {
    pub corpus: String,
    pub reason: String,
    pub report: Option<BuildReport>,
    pub error: Option<String>,
}

/// What the index keeper is doing or last did; emitted as `corpus:index`.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct IndexJob {
    /// `idle` (never run) | `checking` | `building` | `current` (nothing to
    /// build) | `built` | `failed` (some corpus failed) | `unavailable` (no
    /// registry, or status failed).
    pub phase: &'static str,
    /// The corpora being or last built.
    pub corpora: Vec<String>,
    pub builds: Vec<CorpusBuild>,
    /// One line for the status bar.
    pub message: String,
}

impl Default for IndexJob {
    fn default() -> Self {
        IndexJob {
            phase: "idle",
            corpora: vec![],
            builds: vec![],
            message: String::new(),
        }
    }
}

/// Keeps the corpus indexes current in the background, one run at a time.
#[derive(Default)]
pub struct IndexKeeper {
    running: AtomicBool,
    last: Mutex<IndexJob>,
}

struct Running<'a>(&'a AtomicBool);
impl Drop for Running<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

impl IndexKeeper {
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// The current or last job.
    pub fn job(&self) -> IndexJob {
        self.last.lock().map(|j| j.clone()).unwrap_or_default()
    }

    fn set(&self, job: IndexJob, emit: &dyn Fn(&IndexJob)) {
        if let Ok(mut l) = self.last.lock() {
            *l = job.clone();
        }
        emit(&job);
    }

    /// Check every corpus and incrementally build those that are missing or
    /// stale, emitting each phase. Returns `None` without doing anything when
    /// a refresh is already running. Blocking.
    pub fn refresh(
        &self,
        engine: impl FnOnce() -> Result<Engine, CorpusError>,
        emit: &dyn Fn(&IndexJob),
    ) -> Option<IndexJob> {
        if self.running.swap(true, Ordering::SeqCst) {
            return None;
        }
        let _guard = Running(&self.running);
        self.set(
            IndexJob {
                phase: "checking",
                message: "Checking the archive indexes…".into(),
                ..IndexJob::default()
            },
            emit,
        );
        let done = |job: IndexJob| {
            self.set(job.clone(), emit);
            Some(job)
        };
        let engine = match engine() {
            Ok(e) => e,
            Err(e) => {
                return done(IndexJob {
                    phase: "unavailable",
                    message: format!("Archive search unavailable: {e}"),
                    ..IndexJob::default()
                })
            }
        };
        let report = match engine.index_status() {
            Ok(r) => r.body,
            Err(e) => {
                return done(IndexJob {
                    phase: "unavailable",
                    message: format!("Archive index status failed: {e:#}"),
                    ..IndexJob::default()
                })
            }
        };
        let todo: Vec<(String, &'static str)> = report
            .corpora
            .iter()
            .filter_map(|st| build_reason(st).map(|r| (st.corpus.clone(), r)))
            .collect();
        if todo.is_empty() {
            return done(IndexJob {
                phase: "current",
                message: "Archive indexes are up to date".into(),
                ..IndexJob::default()
            });
        }
        let corpora: Vec<String> = todo.iter().map(|(c, _)| c.clone()).collect();
        self.set(
            IndexJob {
                phase: "building",
                corpora: corpora.clone(),
                message: format!("Updating the archive index: {}…", describe(&todo)),
                ..IndexJob::default()
            },
            emit,
        );
        let builds: Vec<CorpusBuild> = todo
            .iter()
            .map(|(id, reason)| {
                let r = engine.index_build(&IndexBuildQuery {
                    ids: vec![id.clone()],
                    force: false,
                });
                match r {
                    Ok(r) => CorpusBuild {
                        corpus: id.clone(),
                        reason: reason.to_string(),
                        report: r.body.reports.into_iter().next(),
                        error: None,
                    },
                    Err(e) => CorpusBuild {
                        corpus: id.clone(),
                        reason: reason.to_string(),
                        report: None,
                        error: Some(format!("{e:#}")),
                    },
                }
            })
            .collect();
        let failed: Vec<String> = builds
            .iter()
            .filter_map(|b| b.error.as_ref().map(|e| format!("{}: {e}", b.corpus)))
            .collect();
        let job = if failed.is_empty() {
            IndexJob {
                phase: "built",
                message: format!("Archive index updated: {}", built_summary(&builds)),
                corpora,
                builds,
            }
        } else {
            IndexJob {
                phase: "failed",
                message: format!("Archive index update failed ({})", failed.join("; ")),
                corpora,
                builds,
            }
        };
        done(job)
    }
}

fn describe(todo: &[(String, &'static str)]) -> String {
    todo.iter()
        .map(|(c, r)| format!("{c} ({r})"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn built_summary(builds: &[CorpusBuild]) -> String {
    builds
        .iter()
        .filter_map(|b| {
            b.report.as_ref().map(|r| {
                if r.full {
                    format!("{} rebuilt ({} documents)", b.corpus, r.docs)
                } else {
                    format!(
                        "{} ({} changed, {} removed)",
                        b.corpus, r.indexed, r.removed
                    )
                }
            })
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// A temp registry over writing, tweets and a Highlight Scout archive, shared
/// by this module's tests and `corpus_copy`'s.
#[cfg(test)]
pub(crate) mod fixture {
    use scout_corpus::{Engine, Registry};
    use std::fs;
    use std::path::{Path, PathBuf};

    pub fn write(root: &Path, rel: &str, body: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, body).unwrap();
    }

    /// Writing, tweets and a Highlight Scout archive in a temp dir, with a
    /// registry naming all three (the shape of `~/.config/scout/corpora.toml`).
    pub struct Fixture {
        pub root: PathBuf,
        pub sources: Vec<PathBuf>,
        pub registry: PathBuf,
        pub indexes: PathBuf,
    }

    impl Fixture {
        pub fn new(name: &str) -> Fixture {
            let root = std::env::temp_dir().join(format!(
                "hs-corpus-{name}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            let (w, t, h) = (
                root.join("writing"),
                root.join("tweets"),
                root.join("archive"),
            );
            write(
                &w,
                "essays/2016-generative-metaphor.md",
                "---\ntitle: Generative metaphor\ndate: 2016-06-23\ngenre: essay\n---\n\
                 A generative metaphor frames a problem and suggests its solution.\n\n\
                 Schön’s *generative metaphor* is a model of thought.\n",
            );
            write(
                &t,
                "stream/2025-07.md",
                "---\ntitle: Stream\nvenue: \"Twitter/X (@techczech)\"\n---\n\n\
                 ## 2025-07-26T10:12:00Z — tweet 1949000000000000001 · kind: original\n\n\
                 <!-- tweet id=\"1949000000000000001\" -->\n~~~\nEvery generative metaphor is a model.\n~~~\n",
            );
            write(
                &h,
                "readings/works/schon-abc.md",
                "---\ntitle: Generative Metaphor and Social Policy\nauthor: Donald Schön\ntype: book\nsource_system: zotero\nsource_id: \"ABC\"\nsource_data: {\"date\":\"1979-00-00 1979\"}\n---\n\n\
                 > Generative metaphor is a process of seeing one thing as another.\n\n\
                 highlighted_at: 2024-01-15 | tags: linguistics\n\n---\n",
            );
            let registry = root.join("corpora.toml");
            fs::write(
                &registry,
                format!(
                    "[[corpus]]\nid = \"writing\"\nname = \"Writing\"\nkind = \"markdown-folder\"\npath = \"{}\"\nrequire_frontmatter = [\"genre\"]\ndefault_author = \"Dominik Lukeš\"\nlink = \"writeflex://open?path={{path}}&line={{line}}\"\n\n\
                     [[corpus]]\nid = \"tweets\"\nname = \"Tweets\"\nkind = \"markdown-folder\"\npath = \"{}\"\ndefault_author = \"Dominik Lukeš\"\ndocument_unit = \"tweet\"\nlink = \"writeflex://open?path={{path}}&line={{line}}\"\n\n\
                     [[corpus]]\nid = \"highlights\"\nname = \"Highlights\"\nkind = \"highlight-scout-archive\"\npath = \"{}\"\n",
                    w.display(),
                    t.display(),
                    h.display()
                ),
            )
            .unwrap();
            Fixture {
                indexes: root.join("data").join("indexes"),
                sources: vec![w, t, h],
                registry,
                root,
            }
        }

        pub fn engine(&self) -> Engine {
            Engine::new(Registry::load_from(&self.registry).unwrap(), &self.indexes)
        }

        /// Every source file with its bytes and mtime.
        pub fn snapshot(&self) -> Vec<(PathBuf, Vec<u8>, std::time::SystemTime)> {
            fn walk(p: &Path, out: &mut Vec<(PathBuf, Vec<u8>, std::time::SystemTime)>) {
                for e in fs::read_dir(p).unwrap() {
                    let p = e.unwrap().path();
                    if p.is_dir() {
                        walk(&p, out);
                    } else {
                        let m = fs::metadata(&p).unwrap().modified().unwrap();
                        out.push((p.clone(), fs::read(&p).unwrap(), m));
                    }
                }
            }
            let mut out = vec![];
            for s in &self.sources {
                walk(s, &mut out);
            }
            out.sort_by(|a, b| a.0.cmp(&b.0));
            out
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fixture::*;
    use super::*;
    use std::path::Path;

    fn phases(keeper: &IndexKeeper, engine: Engine) -> (Option<IndexJob>, Vec<&'static str>) {
        let seen = Mutex::new(vec![]);
        let job = keeper.refresh(|| Ok(engine), &|j| seen.lock().unwrap().push(j.phase));
        (job, seen.into_inner().unwrap())
    }

    #[test]
    fn missing_indexes_are_built_in_one_refresh_then_reported_current() {
        let fx = Fixture::new("build");
        let keeper = IndexKeeper::default();

        let err = search(
            &fx.engine(),
            &SearchQuery {
                query: "generative metaphor".into(),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert_eq!(err.kind, "no_indexed_corpus");
        assert!(err.wants_index());

        let (job, seen) = phases(&keeper, fx.engine());
        let job = job.unwrap();
        assert_eq!(seen, vec!["checking", "building", "built"]);
        assert_eq!(job.corpora, vec!["writing", "tweets", "highlights"]);
        assert!(job
            .builds
            .iter()
            .all(|b| b.reason == "missing" && b.error.is_none()));
        assert!(
            job.message
                .starts_with("Archive index updated: writing rebuilt"),
            "{}",
            job.message
        );
        assert_eq!(keeper.job(), job);
        assert!(!keeper.is_running());

        let (job, seen) = phases(&keeper, fx.engine());
        assert_eq!(seen, vec!["checking", "current"]);
        assert!(job.unwrap().corpora.is_empty());

        // An edited source makes that one corpus stale; the rebuild is incremental.
        std::thread::sleep(std::time::Duration::from_millis(20));
        write(
            &fx.sources[0],
            "essays/2016-generative-metaphor.md",
            "---\ntitle: Generative metaphor\ndate: 2016-06-23\ngenre: essay\n---\n\
             A generative metaphor frames a problem and suggests its solution. Revised.\n",
        );
        let st = status(&fx.engine()).unwrap().body;
        let reasons: Vec<_> = st.corpora.iter().map(build_reason).collect();
        assert_eq!(reasons, vec![Some("stale"), None, None]);
        let (job, _) = phases(&keeper, fx.engine());
        let job = job.unwrap();
        assert_eq!(job.corpora, vec!["writing"]);
        let r = job.builds[0].report.as_ref().unwrap();
        assert!(!r.full);
        assert_eq!(r.indexed, 1);
        assert_eq!(
            job.message,
            "Archive index updated: writing (1 changed, 0 removed)"
        );
    }

    #[test]
    fn search_over_three_corpora_then_cite_gives_the_original_passage() {
        let fx = Fixture::new("search");
        let keeper = IndexKeeper::default();
        phases(&keeper, fx.engine()).0.unwrap();
        let before = fx.snapshot();

        let r = search(
            &fx.engine(),
            &SearchQuery {
                query: "generative metaphor".into(),
                in_: vec!["writing".into(), "tweets".into(), "highlights".into()],
                ..Default::default()
            },
        )
        .unwrap();
        assert!(r.notes.is_empty(), "{:?}", r.notes);
        let body = &r.body;
        assert_eq!(body.corpora, vec!["writing", "tweets", "highlights"]);
        let mut corpora: Vec<&str> = body.results.iter().map(|d| d.corpus.as_str()).collect();
        corpora.sort();
        assert_eq!(corpora, vec!["highlights", "tweets", "writing"]);

        // The typed result is the CLI's JSON shape.
        let v = serde_json::to_value(body).unwrap();
        assert_eq!(v["schema_version"], 1);
        let essay = v["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["corpus"] == "writing")
            .unwrap();
        assert_eq!(essay["title"], "Generative metaphor");
        assert_eq!(essay["date_display"], "23 June 2016");
        let hit = &essay["hits"][0];
        let pid = hit["passage_id"].as_str().unwrap();
        assert!(
            pid.starts_with("writing:essays/2016-generative-metaphor.md:"),
            "{pid}"
        );

        // Cite: the whole passage as written (curly quote and Markdown kept),
        // with the absolute source file and its 1-based line.
        let second = essay["hits"]
            .as_array()
            .unwrap()
            .iter()
            .find(|h| h["quote"].as_str().unwrap().contains("Schön"))
            .unwrap();
        let c = cite(&fx.engine(), second["passage_id"].as_str().unwrap())
            .unwrap()
            .body;
        assert_eq!(
            c.quote,
            "Schön’s *generative metaphor* is a model of thought."
        );
        assert_eq!(c.line_start, 8);
        assert_eq!(
            Path::new(&c.path),
            fx.sources[0].join("essays/2016-generative-metaphor.md")
        );
        assert!(c
            .citation
            .markdown
            .starts_with("> Schön’s *generative metaphor*"));

        let hl = v["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["corpus"] == "highlights")
            .unwrap();
        let c = cite(&fx.engine(), hl["hits"][0]["passage_id"].as_str().unwrap())
            .unwrap()
            .body;
        assert_eq!(
            c.quote,
            "Generative metaphor is a process of seeing one thing as another."
        );
        assert_eq!(c.author.as_deref(), Some("Donald Schön"));

        // `in:` in the query narrows without --in.
        let r = search(
            &fx.engine(),
            &SearchQuery {
                query: "generative in:tweets".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(r.body.corpora, vec!["tweets"]);

        // No such passage is a typed error.
        let e = cite(&fx.engine(), "tweets:stream/2025-07.md:99").unwrap_err();
        assert_eq!(e.kind, "passage_not_found");

        // Searching, citing and checking status never write to a source.
        status(&fx.engine()).unwrap();
        assert_eq!(fx.snapshot(), before);
    }

    #[test]
    fn a_missing_registry_is_unavailable_and_a_second_refresh_waits_its_turn() {
        let keeper = IndexKeeper::default();
        let (job, seen) = {
            let seen = Mutex::new(vec![]);
            let job = keeper.refresh(
                || {
                    Err(CorpusError {
                        kind: "registry_missing",
                        message: "no corpus registry at /x".into(),
                    })
                },
                &|j| seen.lock().unwrap().push(j.phase),
            );
            (job.unwrap(), seen.into_inner().unwrap())
        };
        assert_eq!(seen, vec!["checking", "unavailable"]);
        assert_eq!(
            job.message,
            "Archive search unavailable: no corpus registry at /x"
        );

        // While one refresh runs, another returns at once without work.
        let fx = Fixture::new("overlap");
        let started = std::sync::Barrier::new(2);
        let release = std::sync::Barrier::new(2);
        std::thread::scope(|s| {
            s.spawn(|| {
                keeper.refresh(
                    || {
                        started.wait();
                        release.wait();
                        Ok(fx.engine())
                    },
                    &|_| {},
                )
            });
            started.wait();
            assert!(keeper.is_running());
            assert!(keeper.refresh(|| Ok(fx.engine()), &|_| {}).is_none());
            release.wait();
        });
        assert!(!keeper.is_running());
        assert_eq!(keeper.job().phase, "built");
    }
}
