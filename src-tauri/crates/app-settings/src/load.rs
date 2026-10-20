//! Reading a settings file's parsed contents into a valid state (port of load.ts). Tolerant per field; a file that is not a settings
//! file is reported; an app's legacy file goes through its `migrate` hook.

use crate::changes::{
    cap_changes, groups_of, sorted_targets, ChangeEntry, Subject, Who, CHANGELOG_KEY, INVALID_SAVED_SETTINGS, INVALID_SAVED_SHORTCUTS, INVALID_SAVED_VALUE,
    MAX_CHANGES, REPAIR_SUMMARY_LIMIT,
};
use crate::commands::CommandsAdapter;
use crate::schema::{Schema, SettingType};
use crate::state::{override_error, settle, Clock, SettingsState, Wanted, STATE_FORMAT, STATE_VERSION};
use crate::value::{bound, jlen, js_eq, norm, Obj};
use serde_json::Value;

/// What an app's `migrate` hook returns: values by setting key and overrides by command id.
#[derive(Debug, Clone, Default)]
pub struct Migrated {
    pub settings: Obj,
    pub keys: Obj,
}

pub type MigrateFn = std::sync::Arc<dyn Fn(&Value) -> Option<Migrated> + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadProblem {
    NotSettings,
    OtherApp,
    NewerVersion,
}

impl LoadProblem {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotSettings => "not-settings",
            Self::OtherApp => "other-app",
            Self::NewerVersion => "newer-version",
        }
    }
}

#[derive(Debug, Clone)]
pub struct LoadResult {
    pub state: SettingsState,
    /// Entries the loader added; non-empty means the file should be written back.
    pub entries: Vec<ChangeEntry>,
    pub problem: Option<LoadProblem>,
    pub migrated: bool,
    pub dropped: Vec<String>,
}

pub struct LoadCtx<'a> {
    pub schema: &'a Schema,
    pub commands: &'a dyn CommandsAdapter,
    pub clock: &'a dyn Clock,
    pub app_version: &'a str,
    pub migrate: Option<&'a MigrateFn>,
}

fn bad(p: LoadProblem, app: &str) -> LoadResult {
    LoadResult { state: SettingsState::empty(app), entries: vec![], problem: Some(p), migrated: false, dropped: vec![] }
}

/// `loadState`. `raw` None = no file.
pub fn load_state(raw: Option<&Value>, ctx: &LoadCtx) -> LoadResult {
    let app = ctx.schema.app.as_str();
    let raw = match raw {
        None | Some(Value::Null) => return LoadResult { state: SettingsState::empty(app), entries: vec![], problem: None, migrated: false, dropped: vec![] },
        Some(r) => r,
    };
    let who = Who::AppUpdate(bound(ctx.app_version, 200));
    if let Some(r) = raw.as_object().filter(|r| r.get("format").and_then(Value::as_str) == Some(STATE_FORMAT)) {
        if r.get("app").and_then(Value::as_str) != Some(app) {
            return bad(LoadProblem::OtherApp, app);
        }
        let version = r.get("version");
        if version.and_then(Value::as_f64).is_some_and(|v| v > STATE_VERSION as f64) {
            return bad(LoadProblem::NewerVersion, app);
        }
        if version.and_then(Value::as_f64) != Some(STATE_VERSION as f64) {
            return bad(LoadProblem::NotSettings, app);
        }
        let mut dropped = Vec::new();
        let (changes, oversized) = read_changes(r.get("changes"), ctx, &mut dropped);
        let settings = match r.get("settings") {
            Some(Value::Object(o)) => o.clone(),
            Some(_) => {
                dropped.push("settings".into());
                Obj::new()
            }
            None => Obj::new(),
        };
        let keys = match r.get("keys") {
            Some(Value::Object(o)) => o.clone(),
            Some(_) => {
                dropped.push("keys".into());
                Obj::new()
            }
            None => Obj::new(),
        };
        let (s, k, entries) = read_values(&settings, &keys, ctx, &who, false, &mut dropped, oversized);
        let state = SettingsState { app: app.into(), settings: s, keys: k, changes: with_loader_entries(&entries, &changes) };
        return LoadResult { state, entries, problem: None, migrated: false, dropped };
    }
    let old = ctx.migrate.and_then(|m| m(raw));
    let Some(old) = old else { return bad(LoadProblem::NotSettings, app) };
    let mut dropped = Vec::new();
    let (s, k, entries) = read_values(&old.settings, &old.keys, ctx, &who, true, &mut dropped, 0);
    let state = SettingsState { app: app.into(), settings: s, keys: k, changes: with_loader_entries(&entries, &[]) };
    LoadResult { state, entries, problem: None, migrated: true, dropped }
}

fn with_loader_entries(created: &[ChangeEntry], saved: &[ChangeEntry]) -> Vec<ChangeEntry> {
    let mut out = cap_changes(created.to_vec(), MAX_CHANGES);
    for g in groups_of(saved) {
        if out.len() + g.len() > MAX_CHANGES {
            break;
        }
        out.extend(g);
    }
    out
}

fn read_changes(raw: Option<&Value>, ctx: &LoadCtx, dropped: &mut Vec<String>) -> (Vec<ChangeEntry>, usize) {
    let Some(raw) = raw else { return (vec![], 0) };
    let Some(arr) = raw.as_array() else {
        dropped.push("changes".into());
        return (vec![], 0);
    };
    let batch_of = |c: &Value| c.as_object().and_then(|o| o.get("batch")).and_then(Value::as_str).map(str::to_string);
    let mut sizes: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for c in arr {
        if let Some(b) = batch_of(c) {
            *sizes.entry(b).or_default() += 1;
        }
    }
    let (mut list, mut secret, mut oversized) = (Vec::new(), 0, 0);
    for (i, c) in arr.iter().enumerate() {
        if let Some(b) = batch_of(c) {
            if sizes[&b] > MAX_CHANGES {
                oversized += 1;
                continue;
            }
        }
        let Some(mut e) = ChangeEntry::from_json(c) else {
            dropped.push(format!("changes[{i}]"));
            continue;
        };
        let is_secret = |k: &str| ctx.schema.def(k).is_some_and(|d| d.kind == SettingType::Secret);
        if e.command_id().is_some_and(is_secret) || (e.key().is_some_and(is_secret) && !e.is_secret_entry()) {
            secret += 1;
            continue;
        }
        if let Some(t) = e.targets.take() {
            let settings_kind = e.note.as_deref() == Some(INVALID_SAVED_SETTINGS);
            e.targets = Some(
                t.into_iter()
                    .filter(|x| {
                        if settings_kind {
                            ctx.schema.def(x).is_some_and(|d| d.kind != SettingType::Secret)
                        } else {
                            ctx.schema.def(x).is_none() && ctx.commands.is_command(x)
                        }
                    })
                    .collect(),
            );
        }
        list.push(e);
    }
    if secret > 0 {
        dropped.push(format!("changes: {secret} about a secret"));
    }
    (list, oversized)
}

struct Pending {
    subject: Subject,
    from: Value,
    to: Value,
    note: Option<&'static str>,
    targets: Option<Vec<String>>,
}

fn read_values(settings: &Obj, keys: &Obj, ctx: &LoadCtx, who: &Who, log_valid: bool, dropped: &mut Vec<String>, oversized: usize) -> (Obj, Obj, Vec<ChangeEntry>) {
    let mut records: Vec<Pending> = Vec::new();
    let mut bad_settings: Vec<Pending> = Vec::new();
    let mut bad_keys: Vec<Pending> = Vec::new();
    let mut out_settings = Obj::new();
    for (key, value) in settings {
        let value = norm(value.clone());
        let Some(def) = ctx.schema.def(key) else {
            dropped.push(format!("settings.{}", bound(key, 200)));
            continue;
        };
        if def.kind == SettingType::Secret {
            dropped.push(format!("settings.{key}"));
            continue;
        }
        if ctx.schema.check_value(def, &value).is_none() {
            if js_eq(&value, &def.default) {
                continue;
            }
            out_settings.insert(key.clone(), value.clone());
            if log_valid {
                records.push(Pending { subject: Subject::Key(key.clone()), from: def.default.clone(), to: value, note: None, targets: None });
            }
            continue;
        }
        bad_settings.push(Pending { subject: Subject::Key(key.clone()), from: Value::Null, to: def.default.clone(), note: Some(INVALID_SAVED_VALUE), targets: None });
    }
    let mut candidate: Vec<Wanted> = Vec::new();
    for (id, chord) in keys {
        if !ctx.commands.is_command(id) || ctx.schema.def(id).is_some() || jlen(id) > 200 {
            dropped.push(format!("keys.{}", bound(id, 200)));
            continue;
        }
        if override_error(id, chord, ctx.schema, ctx.commands).is_some() {
            bad_keys.push(Pending { subject: Subject::Command(id.clone()), from: Value::Null, to: Value::String(ctx.commands.default_chord(id)), note: Some(INVALID_SAVED_VALUE), targets: None });
            continue;
        }
        let c = chord.as_str().unwrap_or("").to_string();
        if c == ctx.commands.default_chord(id) {
            continue;
        }
        candidate.push(Wanted { command_id: id.clone(), chord: Some(c), of: String::new() });
    }
    let (accepted, rejected) = settle(&Obj::new(), &candidate, ctx.commands);
    let mut out_keys = Obj::new();
    for w in accepted {
        let c = w.chord.clone().unwrap_or_default();
        out_keys.insert(w.command_id.clone(), Value::String(c.clone()));
        if log_valid {
            records.push(Pending { subject: Subject::Command(w.command_id.clone()), from: Value::String(ctx.commands.default_chord(&w.command_id)), to: Value::String(c), note: None, targets: None });
        }
    }
    for (w, _) in rejected {
        bad_keys.push(Pending {
            subject: Subject::Command(w.command_id.clone()),
            from: Value::Null,
            to: Value::String(ctx.commands.default_chord(&w.command_id)),
            note: Some(INVALID_SAVED_VALUE),
            targets: None,
        });
    }
    let summed = |list: Vec<Pending>, note: &'static str| -> Vec<Pending> {
        if list.len() > REPAIR_SUMMARY_LIMIT {
            let ids: Vec<String> = list
                .iter()
                .map(|p| match &p.subject {
                    Subject::Key(k) | Subject::Command(k) => k.clone(),
                })
                .collect();
            vec![Pending {
                subject: Subject::Key(CHANGELOG_KEY.into()),
                from: crate::value::num(list.len() as f64),
                to: Value::Null,
                note: Some(note),
                targets: Some(sorted_targets(ids.iter().map(String::as_str))),
            }]
        } else {
            list
        }
    };
    let mut pending: Vec<Pending> = records;
    if oversized > 0 {
        pending.push(Pending { subject: Subject::Key(CHANGELOG_KEY.into()), from: crate::value::num(oversized as f64), to: Value::Null, note: Some(INVALID_SAVED_VALUE), targets: None });
    }
    pending.extend(summed(bad_settings, INVALID_SAVED_SETTINGS));
    pending.extend(summed(bad_keys, INVALID_SAVED_SHORTCUTS));
    let at = ctx.clock.now();
    let ids: Vec<String> = pending.iter().map(|_| ctx.clock.new_id()).collect();
    let batch = if pending.len() > 1 { Some(ctx.clock.new_id()) } else { None };
    let use_batch = batch.is_some() && pending.len() <= MAX_CHANGES;
    let mut entries: Vec<ChangeEntry> = pending
        .into_iter()
        .zip(ids)
        .map(|(p, id)| ChangeEntry {
            id,
            at,
            subject: p.subject,
            from: p.from,
            to: p.to,
            who: who.clone(),
            batch: if use_batch { batch.clone() } else { None },
            note: p.note.map(str::to_string),
            targets: p.targets,
            loader_order: true,
        })
        .collect();
    entries.reverse();
    (out_settings, out_keys, entries)
}
