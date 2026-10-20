//! The settings store (port of node/settings-store.ts): an in-memory view of `<dir>/settings.json`; every operation is lock → re-read →
//! pure operation → write → unlock, so the app and the CLI writing at once lose nothing. A watcher reloads outside changes and calls
//! `on_change` (invariant 6: within a second).

use crate::changes::{groups_of, Actor, ChangeEntry, Subject, Who, CHANGELOG_KEY, INVALID_SAVED_VALUE};
use crate::commands::CommandsAdapter;
use crate::file::{FileRead, SettingsFile, Step, MSG_UNARCHIVED, MSG_UNWRITTEN};
use crate::load::{load_state, LoadCtx, LoadProblem, Migrated, MigrateFn};
use crate::schema::{Schema, SettingType};
use crate::state::{self, Clock, Committed, Ctx, OpError, OpResult, SettingsState, SystemClock, Target};
use crate::transfer::{self, ImportPreview};
use crate::value::{bound, pretty};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const SETTINGS_FILE_NAME: &str = "settings.json";
pub const MSG_OTHER_APP: &str = "The settings file belongs to another app, so it is left as it is and changes are not saved";
pub const MSG_NEWER: &str = "The settings file was written by a newer version of the app; update the app before changing settings";

pub type LogFn = Arc<dyn Fn(&str) + Send + Sync>;
pub type ChangeFn = Arc<dyn Fn(&SettingsState) + Send + Sync>;

pub struct StoreOptions {
    pub dir: PathBuf,
    pub schema: Arc<Schema>,
    pub commands: Arc<dyn CommandsAdapter>,
    pub app_version: String,
    pub migrate: Option<MigrateFn>,
    /// Who the store's changes are logged as (you in the app; an agent in the CLI).
    pub who: Actor,
    pub root: Option<PathBuf>,
    pub file_name: Option<String>,
    pub clock: Arc<dyn Clock>,
    pub log: LogFn,
    pub on_change: Option<ChangeFn>,
}

impl StoreOptions {
    pub fn new(dir: PathBuf, schema: Arc<Schema>, commands: Arc<dyn CommandsAdapter>, app_version: &str) -> Self {
        StoreOptions {
            dir,
            schema,
            commands,
            app_version: app_version.into(),
            migrate: None,
            who: Actor::You,
            root: None,
            file_name: None,
            clock: Arc::new(SystemClock),
            log: Arc::new(|l| eprintln!("{l}")),
            on_change: None,
        }
    }
}

/// `refused` (a link, outside the root, unreadable), `other-app`, `newer-version`, `unarchived`.
#[derive(Debug, Clone, PartialEq)]
pub struct Problem {
    pub code: &'static str,
    pub error: String,
}

#[derive(Clone)]
struct Loaded {
    state: SettingsState,
    entries: Vec<ChangeEntry>,
    problem: Option<&'static str>,
    error: Option<String>,
    damaged: bool,
    migrated: bool,
}

struct Probe;
impl Clock for Probe {
    fn now(&self) -> f64 {
        0.0
    }
    fn new_id(&self) -> String {
        "probe".into()
    }
}

struct Inner {
    state: SettingsState,
    problem: Option<&'static str>,
    refused_error: Option<String>,
    last_dropped: Option<String>,
}

pub struct SettingsStore {
    pub file: SettingsFile,
    opts: StoreOptions,
    inner: Mutex<Inner>,
    watching: Mutex<bool>,
}

fn op_err(code: &'static str, error: impl Into<String>) -> OpError {
    state::fail(code, error)
}

impl SettingsStore {
    /// Opens the store: sweeps dead writers' temporary files, reads (repairing or migrating the file if needed).
    pub fn open(opts: StoreOptions) -> Arc<SettingsStore> {
        let path = opts.dir.join(opts.file_name.clone().unwrap_or_else(|| SETTINGS_FILE_NAME.into()));
        let schema = opts.schema.clone();
        let commands = opts.commands.clone();
        let migrate = opts.migrate.clone();
        let version = opts.app_version.clone();
        let accept = Box::new(move |raw: &Value| {
            let ctx = LoadCtx { schema: &schema, commands: commands.as_ref(), clock: &Probe, app_version: &version, migrate: migrate.as_ref() };
            raw.is_object() && load_state(Some(raw), &ctx).problem != Some(LoadProblem::NotSettings)
        });
        let file = SettingsFile::new(path, Some(opts.root.clone().unwrap_or_else(|| opts.dir.clone())), accept);
        let app = opts.schema.app.clone();
        let store = Arc::new(SettingsStore {
            file,
            opts,
            inner: Mutex::new(Inner { state: SettingsState::empty(&app), problem: None, refused_error: None, last_dropped: None }),
            watching: Mutex::new(false),
        });
        store.file.sweep();
        store.reload();
        store
    }

    pub fn view(&self) -> SettingsState {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).state.clone()
    }

    pub fn schema(&self) -> &Schema {
        &self.opts.schema
    }

    pub fn commands(&self) -> &dyn CommandsAdapter {
        self.opts.commands.as_ref()
    }

    pub fn path(&self) -> &std::path::Path {
        &self.file.path
    }

    pub fn problem(&self) -> Option<Problem> {
        let g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let code = g.problem?;
        let error = match code {
            "other-app" => MSG_OTHER_APP.into(),
            "newer-version" => MSG_NEWER.into(),
            "unarchived" => MSG_UNARCHIVED.into(),
            _ => g.refused_error.clone().unwrap_or_default(),
        };
        Some(Problem { code, error })
    }

    fn ctx(&self) -> Ctx<'_> {
        Ctx { schema: &self.opts.schema, commands: self.opts.commands.as_ref(), clock: self.opts.clock.as_ref(), who: self.opts.who.clone() }
    }

    // ---- operations ----------------------------------------------------------------------------------------------------------------

    pub fn set(&self, key: &str, value: &Value) -> OpResult {
        self.mutate(&|s, c| state::set(s, key, value, c), false)
    }
    pub fn rebind(&self, id: &str, chord: &str) -> OpResult {
        self.mutate(&|s, c| state::rebind(s, id, chord, c), false)
    }
    pub fn rebind_many(&self, pairs: &[(String, Value)]) -> OpResult {
        self.mutate(&|s, c| state::rebind_many(s, pairs, c), false)
    }
    pub fn reset(&self, t: &Target) -> OpResult {
        self.mutate(&|s, c| state::reset(s, t, c), false)
    }
    pub fn reset_section(&self, section: &str) -> OpResult {
        self.mutate(&|s, c| state::reset_section(s, section, c), false)
    }
    pub fn reset_all(&self) -> OpResult {
        self.mutate(&|s, c| state::reset_all(s, c), false)
    }
    pub fn reset_entry(&self, id: &str) -> OpResult {
        self.mutate(&|s, c| state::reset_entry(s, id, c), false)
    }
    pub fn record_secret(&self, key: &str, from: &str, to: &str) -> OpResult {
        self.mutate(&|s, c| state::record_secret(s, key, from, to, c), false)
    }
    pub fn undo_batch(&self, batch: &str) -> OpResult {
        self.mutate(&|s, c| state::undo_batch(s, batch, c), false)
    }
    pub fn preview_import(&self, text: &str) -> ImportPreview {
        transfer::preview_import(&self.view(), text, &self.opts.schema, self.opts.commands.as_ref())
    }
    pub fn apply_import(&self, preview: &ImportPreview, file: &str) -> OpResult {
        self.mutate(&|s, c| transfer::apply_import(s, preview, file, c), false)
    }
    pub fn export_file(&self, host_name: &str) -> Value {
        transfer::export_file(&self.view(), &self.opts.schema, self.opts.commands.as_ref(), &self.opts.app_version, host_name, self.opts.clock.now())
    }

    /// Values from outside the settings file (an app's legacy file, its earlier browser storage), logged as `who: app-update` in one
    /// batch, one entry per value that really changes. `overwrite` false leaves settings and shortcuts that already have a stored
    /// value; true (re-migrating a legacy file the old version saved again) puts every valid value in place.
    pub fn migrate_values(&self, values: &Migrated, overwrite: bool) -> OpResult {
        let version = bound(&self.opts.app_version, 200);
        self.mutate(
            &|s, c| {
                let mut writes = Vec::new();
                for (k, v) in &values.settings {
                    let v = crate::value::norm(v.clone());
                    if let Some(d) = c.schema.def(k) {
                        if d.kind != SettingType::Secret && c.schema.check_value(d, &v).is_none() && (overwrite || !s.settings.contains_key(k)) {
                            writes.push(state::Write { target: Subject::Key(k.clone()), to: v, to_default: false, who: None });
                        }
                    }
                }
                for (id, chord) in &values.keys {
                    if state::override_error(id, chord, c.schema, c.commands).is_none() && (overwrite || !s.keys.contains_key(id)) {
                        writes.push(state::Write { target: Subject::Command(id.clone()), to: chord.clone(), to_default: false, who: None });
                    }
                }
                state::commit(s, writes, c, &Who::AppUpdate(version.clone()), false)
            },
            false,
        )
    }

    /// Writes the file as it is now (creating it with the defaults when there is none), with no changelog entry.
    pub fn touch(&self) -> OpResult {
        self.mutate(&|s, _| Ok(Committed { state: s.clone(), entries: vec![], batch: None, skipped: vec![] }), true)
    }

    pub fn refuse_writes(&self, reason: Option<&str>) {
        self.file.refuse_writes(reason);
    }

    // ---- internals -----------------------------------------------------------------------------------------------------------------

    fn load_from(&self, read: &FileRead) -> Loaded {
        let app = &self.opts.schema.app;
        match read {
            FileRead::Refused(e) => Loaded { state: SettingsState::empty(app), entries: vec![], problem: Some("refused"), error: Some(e.clone()), damaged: false, migrated: false },
            FileRead::Missing => Loaded { state: SettingsState::empty(app), entries: vec![], problem: None, error: None, damaged: false, migrated: false },
            FileRead::Damaged(_) => self.damaged_state(),
            FileRead::Ok(raw) => {
                let ctx = LoadCtx {
                    schema: &self.opts.schema,
                    commands: self.opts.commands.as_ref(),
                    clock: self.opts.clock.as_ref(),
                    app_version: &self.opts.app_version,
                    migrate: self.opts.migrate.as_ref(),
                };
                let r = load_state(Some(raw), &ctx);
                if !r.dropped.is_empty() {
                    let more = if r.dropped.len() > 20 { format!(" and {} more", r.dropped.len() - 20) } else { String::new() };
                    let line = format!("Settings: left out of {}: {}{more}", self.file.path.display(), r.dropped.iter().take(20).cloned().collect::<Vec<_>>().join(", "));
                    let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                    if g.last_dropped.as_deref() != Some(line.as_str()) {
                        (self.opts.log)(&line);
                    }
                    g.last_dropped = Some(line);
                }
                match r.problem {
                    Some(LoadProblem::NotSettings) => self.damaged_state(),
                    Some(p) => Loaded { state: r.state, entries: vec![], problem: Some(p.as_str()), error: None, damaged: false, migrated: false },
                    None => Loaded { state: r.state, entries: r.entries, problem: None, error: None, damaged: false, migrated: r.migrated },
                }
            }
        }
    }

    fn damaged_state(&self) -> Loaded {
        let e = ChangeEntry {
            id: self.opts.clock.new_id(),
            at: self.opts.clock.now(),
            subject: Subject::Key(CHANGELOG_KEY.into()),
            from: Value::Null,
            to: Value::Null,
            who: Who::AppUpdate(bound(&self.opts.app_version, 200)),
            batch: None,
            note: Some(INVALID_SAVED_VALUE.into()),
            targets: None,
            loader_order: false,
        };
        let mut state = SettingsState::empty(&self.opts.schema.app);
        state.changes = vec![e.clone()];
        Loaded { state, entries: vec![e], problem: None, error: None, damaged: true, migrated: false }
    }

    fn adopt(&self, next: Loaded) {
        let changed = {
            let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            g.problem = next.problem;
            g.refused_error = next.error;
            let changed = g.state.to_json() != next.state.to_json();
            g.state = next.state.clone();
            changed
        };
        if changed {
            if let Some(f) = &self.opts.on_change {
                f(&next.state);
            }
        }
    }

    fn fitted(&self, state: SettingsState, keep: usize) -> SettingsState {
        let max = self.file.max_bytes() as usize;
        let size = |s: &SettingsState| pretty(&s.to_json()).len();
        if size(&state) <= max {
            return state;
        }
        let mut groups = groups_of(&state.changes);
        let mut changes = state.changes.clone();
        while let Some(g) = groups.pop() {
            if changes.len() - g.len() < keep {
                break;
            }
            changes.truncate(changes.len() - g.len());
            let trial = SettingsState { changes: changes.clone(), ..state.clone() };
            if size(&trial) <= max {
                (self.opts.log)(&format!(
                    "Settings: the oldest {} changes were dropped to keep {} under {max} bytes",
                    state.changes.len() - changes.len(),
                    self.file.path.display()
                ));
                return trial;
            }
        }
        state
    }

    fn write_problem(l: &Loaded) -> Option<OpError> {
        match l.problem {
            Some("other-app") => Some(op_err("refused", MSG_OTHER_APP)),
            Some("newer-version") => Some(op_err("refused", MSG_NEWER)),
            Some("refused") => Some(op_err("refused", l.error.clone().unwrap_or_else(|| crate::file::MSG_LINKED.into()))),
            _ => None,
        }
    }

    fn mutate(&self, op: &dyn Fn(&SettingsState, &Ctx) -> OpResult, force: bool) -> OpResult {
        let mut outcome: Option<OpResult> = None;
        let mut before: Option<Loaded> = None;
        let mut after: Option<SettingsState> = None;
        let r = self.file.update(|read| {
            let loaded = self.load_from(read);
            if loaded.damaged {
                let aside = match read {
                    FileRead::Damaged(Some(p)) => format!(" as {}", p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()),
                    _ => String::new(),
                };
                (self.opts.log)(&format!("Settings: {} could not be read; it was set aside{aside} and the defaults loaded", self.file.path.display()));
            }
            before = Some(loaded.clone());
            if let Some(e) = Self::write_problem(&loaded) {
                outcome = Some(Err(e));
                return Step { write: None, result: () };
            }
            let res = op(&loaded.state, &self.ctx());
            let repair = !loaded.entries.is_empty() || loaded.migrated || force;
            match res {
                Err(e) => {
                    outcome = Some(Err(e));
                    let w = repair.then(|| self.fitted(loaded.state.clone(), loaded.entries.len()).to_json());
                    if w.is_some() {
                        after = None;
                    }
                    Step { write: w, result: () }
                }
                Ok(c) => {
                    let nothing = c.entries.is_empty() && !repair;
                    let next = if nothing { c.state.clone() } else { self.fitted(c.state.clone(), c.entries.len() + loaded.entries.len()) };
                    outcome = Some(Ok(Committed { state: next.clone(), ..c }));
                    after = Some(next.clone());
                    Step { write: (!nothing).then(|| next.to_json()), result: () }
                }
            }
        });
        match r {
            Err((e, _)) => {
                if e.unarchived {
                    self.adopt(Loaded { state: SettingsState::empty(&self.opts.schema.app), entries: vec![], problem: Some("unarchived"), error: Some(e.error.clone()), damaged: false, migrated: false });
                } else if let Some(b) = before {
                    if e.code != "locked" && e.code != "closed" {
                        self.adopt(b);
                    }
                }
                Err(op_err(e.code, e.error))
            }
            Ok(_) => {
                match (after, before) {
                    (Some(s), _) => self.adopt(Loaded { state: s, entries: vec![], problem: None, error: None, damaged: false, migrated: false }),
                    (None, Some(b)) => self.adopt(b),
                    _ => {}
                }
                outcome.unwrap_or_else(|| Err(op_err("unwritten", MSG_UNWRITTEN)))
            }
        }
    }

    fn needs_repair(&self, read: &FileRead) -> Option<bool> {
        match read {
            FileRead::Damaged(_) => Some(false),
            FileRead::Ok(raw) => {
                let ctx = LoadCtx { schema: &self.opts.schema, commands: self.opts.commands.as_ref(), clock: &Probe, app_version: &self.opts.app_version, migrate: self.opts.migrate.as_ref() };
                let r = load_state(Some(raw), &ctx);
                if r.problem.is_some() {
                    return None;
                }
                if r.migrated {
                    return Some(true);
                }
                if !r.entries.is_empty() {
                    return Some(false);
                }
                let secret = r.dropped.iter().any(|d| {
                    d.starts_with("changes: ") || d.strip_prefix("settings.").and_then(|k| self.opts.schema.def(k)).is_some_and(|d| d.kind == SettingType::Secret)
                });
                secret.then_some(true)
            }
            _ => None,
        }
    }

    /// Reads the file again (repairing it when it needs it) and calls `on_change` when the state differs.
    pub fn reload(&self) {
        let read = self.file.read_in_order();
        if let Some(purge) = self.needs_repair(&read) {
            let r = self.mutate(&|s, _| Ok(Committed { state: s.clone(), entries: vec![], batch: None, skipped: vec![] }), purge);
            match r {
                Ok(_) => return,
                Err(e) => {
                    if e.code != "refused" || matches!(read, FileRead::Damaged(_)) {
                        (self.opts.log)(&format!("Settings: {} needs repairing but could not be written: {}", self.file.path.display(), e.error));
                    }
                    if self.problem().is_some_and(|p| p.code == "unarchived") {
                        return;
                    }
                }
            }
        }
        let l = self.load_from(&read);
        self.adopt(l);
    }

    /// Watches the file from a background thread (polling its identity and modification time every 200 ms) and reloads on a change.
    pub fn watch(self: &Arc<Self>) {
        let mut w = self.watching.lock().unwrap_or_else(|e| e.into_inner());
        if *w {
            return;
        }
        *w = true;
        let weak = Arc::downgrade(self);
        std::thread::spawn(move || {
            let stamp = |p: &std::path::Path| {
                use std::os::unix::fs::MetadataExt;
                std::fs::symlink_metadata(p).ok().map(|m| (m.ino(), m.mtime(), m.mtime_nsec(), m.len()))
            };
            let mut last = weak.upgrade().and_then(|s| stamp(&s.file.path));
            loop {
                std::thread::sleep(Duration::from_millis(200));
                let Some(s) = weak.upgrade() else { return };
                let now = stamp(&s.file.path);
                if now != last {
                    last = now;
                    s.reload();
                }
            }
        });
    }
}
