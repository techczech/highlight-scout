//! The settings state (which is also the file, format v2), the one place a change is written (`commit`), the whole-set shortcut
//! judging (`settle`) and the operations (port of state.ts, keys.ts and operations.ts).

use crate::changes::{cap_changes, is_secret_status, limits, Actor, ChangeEntry, SkippedItem, Subject, Who, INVALID_SAVED_SETTINGS, MAX_CHANGES};
use crate::commands::CommandsAdapter;
use crate::schema::{chord_shape_error, Schema, SettingType};
use crate::value::{bound, jlen, js_eq, Obj};
use serde_json::{json, Value};

pub const STATE_FORMAT: &str = "bridle-settings";
pub const STATE_VERSION: u64 = 2;

#[derive(Debug, Clone, PartialEq)]
pub struct SettingsState {
    pub app: String,
    /// Only values that differ from the default; never a secret. Insertion order kept.
    pub settings: Obj,
    /// Shortcut overrides (command id to chord, "" = none); only chords that differ from the default.
    pub keys: Obj,
    /// Newest first.
    pub changes: Vec<ChangeEntry>,
}

impl SettingsState {
    pub fn empty(app: &str) -> Self {
        SettingsState { app: app.to_string(), settings: Obj::new(), keys: Obj::new(), changes: Vec::new() }
    }
    pub fn to_json(&self) -> Value {
        json!({
            "format": STATE_FORMAT,
            "version": STATE_VERSION,
            "app": self.app,
            "settings": Value::Object(self.settings.clone()),
            "keys": Value::Object(self.keys.clone()),
            "changes": self.changes.iter().map(ChangeEntry::to_json).collect::<Vec<_>>(),
        })
    }
    fn keys_vec(&self) -> Vec<(String, Value)> {
        self.keys.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }
}

/// Time and ids, injected so every operation is deterministic in tests.
pub trait Clock: Send + Sync {
    fn now(&self) -> f64;
    fn new_id(&self) -> String;
}

/// The real clock: wall time and random UUIDs.
pub struct SystemClock;
impl Clock for SystemClock {
    fn now(&self) -> f64 {
        crate::value::now_ms()
    }
    fn new_id(&self) -> String {
        crate::value::uuid()
    }
}

pub struct Ctx<'a> {
    pub schema: &'a Schema,
    pub commands: &'a dyn CommandsAdapter,
    pub clock: &'a dyn Clock,
    pub who: Actor,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpError {
    /// `unknown-key`, `unknown-command`, `secret`, `invalid-value`, `invalid-chord`, `not-found`, `already`, `not-possible`, `too-many`.
    pub code: &'static str,
    pub error: String,
    /// Only from `undo_batch`.
    pub skipped: Option<Vec<SkippedItem>>,
}

pub fn fail(code: &'static str, error: impl Into<String>) -> OpError {
    OpError { code, error: error.into(), skipped: None }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Committed {
    pub state: SettingsState,
    pub entries: Vec<ChangeEntry>,
    pub batch: Option<String>,
    /// Only from `undo_batch`.
    pub skipped: Vec<SkippedItem>,
}

pub type OpResult = Result<Committed, OpError>;

/// A setting's value now: stored, or its default. None for a secret or an unknown key.
pub fn value_of(state: &SettingsState, schema: &Schema, key: &str) -> Option<Value> {
    let def = schema.def(key)?;
    if def.kind == SettingType::Secret {
        return None;
    }
    Some(state.settings.get(key).cloned().unwrap_or_else(|| def.default.clone()))
}

/// A command's effective chord.
pub fn chord_of(state: &SettingsState, id: &str, commands: &dyn CommandsAdapter) -> String {
    match state.keys.get(id) {
        Some(v) => v.as_str().unwrap_or("").to_string(),
        None => commands.default_chord(id),
    }
}

fn current_of(state: &SettingsState, schema: &Schema, t: &Subject, commands: &dyn CommandsAdapter) -> Value {
    match t {
        Subject::Key(k) => value_of(state, schema, k).unwrap_or(Value::Null),
        Subject::Command(c) => Value::String(chord_of(state, c, commands)),
    }
}

pub(crate) struct Write {
    pub target: Subject,
    pub to: Value,
    pub to_default: bool,
    pub who: Option<Who>,
}

fn bounded_who(w: Who) -> Who {
    match w {
        Who::You => Who::You,
        Who::Agent(n) => Actor::Agent(n).bounded().into(),
        Who::Reset { of, by } => Who::Reset { of, by: by.map(|b| b.bounded()) },
        Who::Import(mut i) => {
            i.by = i.by.map(|b| b.bounded());
            Who::Import(i)
        }
        other => other,
    }
}

/// Applies writes in order, logging one entry per target that really changed (see state.ts `commit`).
pub(crate) fn commit(state: &SettingsState, writes: Vec<Write>, ctx: &Ctx, who: &Who, force_batch: bool) -> OpResult {
    let mut next = state.clone();
    let mut pending: Vec<(Subject, Value, Value, Who)> = Vec::new();
    let mut keys_changed = false;
    for w in writes {
        let from = current_of(&next, ctx.schema, &w.target, ctx.commands);
        let by = bounded_who(w.who.clone().unwrap_or_else(|| who.clone()));
        match &w.target {
            Subject::Key(k) => {
                let Some(def) = ctx.schema.def(k) else { continue };
                if def.kind == SettingType::Secret {
                    continue;
                }
                let to = if w.to_default { def.default.clone() } else { w.to.clone() };
                if to.is_null() {
                    continue;
                }
                if js_eq(&to, &def.default) {
                    next.settings.shift_remove(k);
                } else {
                    next.settings.insert(k.clone(), to.clone());
                }
                if js_eq(&from, &to) {
                    continue;
                }
                pending.push((w.target.clone(), from, to, by));
            }
            Subject::Command(id) => {
                if ctx.schema.def(id).is_some() {
                    continue;
                }
                let def = ctx.commands.default_chord(id);
                let to = if w.to_default { def.clone() } else { w.to.as_str().unwrap_or("").to_string() };
                if to == def {
                    next.keys.shift_remove(id);
                } else {
                    next.keys.insert(id.clone(), Value::String(to.clone()));
                }
                if from.as_str() == Some(to.as_str()) {
                    continue;
                }
                keys_changed = true;
                pending.push((w.target.clone(), from, Value::String(to), by));
            }
        }
    }
    if keys_changed {
        if let Some(r) = ctx.commands.resolve(&next.keys_vec()).rejected.into_iter().next() {
            return Err(fail("invalid-chord", r.reason));
        }
    }
    if pending.len() > MAX_CHANGES {
        return Err(fail("too-many", format!("That would change {} settings at once; at most {MAX_CHANGES} can be changed together", pending.len())));
    }
    let at = ctx.clock.now();
    let ids: Vec<String> = pending.iter().map(|_| ctx.clock.new_id()).collect();
    let batch = if pending.len() > 1 || (force_batch && !pending.is_empty()) { Some(ctx.clock.new_id()) } else { None };
    let mut entries: Vec<ChangeEntry> = pending
        .into_iter()
        .zip(ids)
        .map(|((subject, from, to, who), id)| ChangeEntry { id, at, subject, from, to, who, batch: batch.clone(), note: None, targets: None, loader_order: false })
        .collect();
    entries.reverse();
    let mut all = entries.clone();
    all.extend(state.changes.iter().cloned());
    next.changes = cap_changes(all, MAX_CHANGES);
    Ok(Committed { state: next, entries, batch, skipped: Vec::new() })
}

// ---- shortcuts judged whole ---------------------------------------------------------------------------------------------------------

pub(crate) fn command_id_error(id: &str, schema: &Schema, commands: &dyn CommandsAdapter) -> Option<String> {
    if id.is_empty() || jlen(id) > limits::ID {
        return Some("That is not a command".into());
    }
    if schema.def(id).is_some() {
        return Some("That is a setting, not a command".into());
    }
    if !commands.is_command(id) {
        return Some(format!("There is no command \"{id}\""));
    }
    None
}

pub(crate) fn override_error(id: &str, chord: &Value, schema: &Schema, commands: &dyn CommandsAdapter) -> Option<String> {
    command_id_error(id, schema, commands).or_else(|| chord_shape_error(chord))
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Wanted {
    pub command_id: String,
    /// None = back to the default.
    pub chord: Option<String>,
    pub of: String,
}

fn apply_wanted(base: &Obj, ws: &[&Wanted], commands: &dyn CommandsAdapter) -> Vec<(String, Value)> {
    let mut out = base.clone();
    for w in ws {
        match &w.chord {
            Some(c) if *c != commands.default_chord(&w.command_id) => {
                out.insert(w.command_id.clone(), Value::String(c.clone()));
            }
            _ => {
                out.shift_remove(&w.command_id);
            }
        }
    }
    out.into_iter().collect()
}

/// `settle`: a maximal subset of `wanted` (deterministic by input order) that `resolve` accepts whole with `base`.
pub(crate) fn settle(base: &Obj, wanted: &[Wanted], commands: &dyn CommandsAdapter) -> (Vec<Wanted>, Vec<(Wanted, String)>) {
    let judge = |idx: &[usize]| {
        let ws: Vec<&Wanted> = idx.iter().map(|&i| &wanted[i]).collect();
        commands.resolve(&apply_wanted(base, &ws, commands)).rejected
    };
    let mut kept: Vec<usize> = (0..wanted.len()).collect();
    let mut removed: Vec<usize> = Vec::new();
    loop {
        let r = judge(&kept);
        if r.is_empty() || kept.is_empty() {
            break;
        }
        let named: Vec<&str> = r.iter().map(|x| x.command_id.as_str()).collect();
        let pos = kept.iter().rposition(|&i| named.contains(&wanted[i].command_id.as_str())).unwrap_or(kept.len() - 1);
        removed.push(kept.remove(pos));
    }
    for i in 0..wanted.len() {
        if !removed.contains(&i) {
            continue;
        }
        let trial: Vec<usize> = (0..wanted.len()).filter(|&x| x == i || kept.contains(&x)).collect();
        if judge(&trial).is_empty() {
            kept = trial;
            removed.retain(|&x| x != i);
        }
    }
    let rejected = (0..wanted.len())
        .filter(|i| removed.contains(i))
        .map(|i| {
            let mut t = kept.clone();
            t.push(i);
            let r = judge(&t);
            let reason = r.iter().find(|x| x.command_id == wanted[i].command_id).or(r.first()).map(|x| x.reason.clone()).unwrap_or_else(|| "Conflicts with another shortcut".into());
            (wanted[i].clone(), reason)
        })
        .collect();
    (kept.into_iter().map(|i| wanted[i].clone()).collect(), rejected)
}

// ---- operations ---------------------------------------------------------------------------------------------------------------------

/// Changes one setting.
pub fn set(state: &SettingsState, key: &str, value: &Value, ctx: &Ctx) -> OpResult {
    let Some(def) = ctx.schema.def(key) else { return Err(fail("unknown-key", format!("There is no setting \"{key}\""))) };
    if def.kind == SettingType::Secret {
        return Err(fail("secret", "Keys are set in Accounts and keys and kept in Keychain, never in the settings file"));
    }
    if let Some(e) = ctx.schema.check_value(def, value) {
        return Err(fail("invalid-value", e));
    }
    let value = crate::value::norm(value.clone());
    commit(state, vec![Write { target: Subject::Key(key.into()), to: value, to_default: false, who: None }], ctx, &ctx.who.clone().into(), false)
}

/// Binds a chord to a command ("" = no shortcut).
pub fn rebind(state: &SettingsState, id: &str, chord: &str, ctx: &Ctx) -> OpResult {
    rebind_many(state, &[(id.to_string(), Value::String(chord.to_string()))], ctx)
}

/// Several shortcuts in one batch (Swap / Replace); all or none.
pub fn rebind_many(state: &SettingsState, pairs: &[(String, Value)], ctx: &Ctx) -> OpResult {
    let mut ids: Vec<&str> = pairs.iter().map(|p| p.0.as_str()).collect();
    ids.sort();
    ids.dedup();
    if ids.len() != pairs.len() {
        return Err(fail("invalid-chord", "A command can take only one shortcut at a time"));
    }
    for (id, chord) in pairs {
        if let Some(e) = command_id_error(id, ctx.schema, ctx.commands) {
            return Err(fail("unknown-command", e));
        }
        if let Some(e) = override_error(id, chord, ctx.schema, ctx.commands) {
            return Err(fail("invalid-chord", e));
        }
    }
    let writes = pairs.iter().map(|(id, c)| Write { target: Subject::Command(id.clone()), to: c.clone(), to_default: false, who: None }).collect();
    commit(state, writes, ctx, &ctx.who.clone().into(), false)
}

/// Logs a key saved or removed: one entry, status words only, never the value.
pub fn record_secret(state: &SettingsState, key: &str, from: &str, to: &str, ctx: &Ctx) -> OpResult {
    let Some(def) = ctx.schema.def(key) else { return Err(fail("unknown-key", format!("There is no setting \"{}\"", bound(key, limits::ID)))) };
    if def.kind != SettingType::Secret {
        return Err(fail("invalid-value", format!("\"{key}\" is not a key")));
    }
    if !is_secret_status(&json!(from)) || !is_secret_status(&json!(to)) {
        return Err(fail("invalid-value", "A key's change is logged as connected or missing"));
    }
    let e = ChangeEntry {
        id: ctx.clock.new_id(),
        at: ctx.clock.now(),
        subject: Subject::Key(key.into()),
        from: json!(from),
        to: json!(to),
        who: ctx.who.bounded().into(),
        batch: None,
        note: None,
        targets: None,
        loader_order: false,
    };
    let mut next = state.clone();
    let mut all = vec![e.clone()];
    all.extend(state.changes.iter().cloned());
    next.changes = cap_changes(all, MAX_CHANGES);
    Ok(Committed { state: next, entries: vec![e], batch: None, skipped: Vec::new() })
}

/// What a reset targets.
#[derive(Debug, Clone)]
pub enum Target {
    Key(String),
    Command(String),
}

/// One setting or one shortcut back to its default.
pub fn reset(state: &SettingsState, target: &Target, ctx: &Ctx) -> OpResult {
    let w = match target {
        Target::Key(k) => {
            let Some(def) = ctx.schema.def(k) else { return Err(fail("unknown-key", format!("There is no setting \"{k}\""))) };
            if def.kind == SettingType::Secret {
                return Err(fail("secret", "Keys are removed in Accounts and keys"));
            }
            Subject::Key(k.clone())
        }
        Target::Command(c) => {
            if let Some(e) = command_id_error(c, ctx.schema, ctx.commands) {
                return Err(fail("unknown-command", e));
            }
            Subject::Command(c.clone())
        }
    };
    commit(state, vec![Write { target: w, to: Value::Null, to_default: true, who: None }], ctx, &ctx.who.clone().into(), false)
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChangedFromDefault {
    pub settings: Vec<String>,
    pub keys: Vec<String>,
    pub count: usize,
}

/// What differs from the default in one section, or everywhere.
pub fn changed_from_default(state: &SettingsState, schema: &Schema, section: Option<&str>) -> ChangedFromDefault {
    let settings: Vec<String> = schema
        .settings
        .iter()
        .filter(|d| d.kind != SettingType::Secret && section.is_none_or(|s| d.section == s) && state.settings.contains_key(&d.key))
        .map(|d| d.key.clone())
        .collect();
    let keys: Vec<String> = if section.is_none() || section == Some("shortcuts") { state.keys.keys().cloned().collect() } else { Vec::new() };
    let count = settings.len() + keys.len();
    ChangedFromDefault { settings, keys, count }
}

fn reset_targets(state: &SettingsState, ctx: &Ctx, section: Option<&str>) -> OpResult {
    let c = changed_from_default(state, ctx.schema, section);
    let mut writes: Vec<Write> = c.settings.into_iter().map(|k| Write { target: Subject::Key(k), to: Value::Null, to_default: true, who: None }).collect();
    writes.extend(c.keys.into_iter().map(|k| Write { target: Subject::Command(k), to: Value::Null, to_default: true, who: None }));
    commit(state, writes, ctx, &ctx.who.clone().into(), false)
}

pub fn reset_section(state: &SettingsState, section: &str, ctx: &Ctx) -> OpResult {
    if ctx.schema.section(section).is_none() {
        return Err(fail("not-found", format!("There is no section \"{section}\"")));
    }
    reset_targets(state, ctx, Some(section))
}

pub fn reset_all(state: &SettingsState, ctx: &Ctx) -> OpResult {
    reset_targets(state, ctx, None)
}

fn restore_error(ctx: &Ctx, t: &Subject, value: &Value) -> Option<String> {
    match t {
        Subject::Key(k) => {
            if value.is_null() {
                return Some("There is no earlier value to put back".into());
            }
            let Some(def) = ctx.schema.def(k) else { return Some("That setting no longer exists".into()) };
            ctx.schema.check_value(def, value)
        }
        Subject::Command(c) => override_error(c, value, ctx.schema, ctx.commands),
    }
}

fn restore_write(t: Subject, value: Value, of: &str, ctx: &Ctx) -> Write {
    let to_default = value.is_null();
    Write { target: t, to: value, to_default, who: Some(Who::Reset { of: of.into(), by: Some(ctx.who.clone()) }) }
}

/// Puts an entry's earlier value back; the reset is a new entry.
pub fn reset_entry(state: &SettingsState, entry_id: &str, ctx: &Ctx) -> OpResult {
    let Some(e) = state.changes.iter().find(|x| x.id == entry_id) else { return Err(fail("not-found", "That change is no longer in the list")) };
    if e.note.is_some() {
        return Err(fail("not-possible", "There is no earlier value to put back"));
    }
    if e.key().and_then(|k| ctx.schema.def(k)).is_some_and(|d| d.kind == SettingType::Secret) {
        return Err(fail("not-possible", "A key cannot be put back from here; enter it again in Accounts and keys"));
    }
    if js_eq(&current_of(state, ctx.schema, &e.subject, ctx.commands), &e.from) {
        return Err(fail("already", "It already has its earlier value"));
    }
    if let Some(err) = restore_error(ctx, &e.subject, &e.from) {
        return Err(fail("not-possible", err));
    }
    let r = commit(state, vec![restore_write(e.subject.clone(), e.from.clone(), &e.id, ctx)], ctx, &ctx.who.clone().into(), false).map_err(|x| fail("not-possible", x.error))?;
    if r.entries.is_empty() {
        return Err(fail("already", "It already has its earlier value"));
    }
    Ok(r)
}

const CHANGED_SINCE: &str = "Changed again since; its later value is kept";

/// Undo import (or any batch), in one new batch; targets changed again since are kept and listed in `skipped`.
pub fn undo_batch(state: &SettingsState, batch_id: &str, ctx: &Ctx) -> OpResult {
    let Some(first) = state.changes.iter().position(|e| e.batch.as_deref() == Some(batch_id)) else {
        return Err(fail("not-found", "That batch is no longer in the list"));
    };
    let mut later: Vec<String> = Vec::new();
    for e in &state.changes[..first] {
        later.push(e.subject.id());
        for t in e.targets.iter().flatten() {
            let s = if e.note.as_deref() == Some(INVALID_SAVED_SETTINGS) { Subject::Key(t.clone()) } else { Subject::Command(t.clone()) };
            later.push(s.id());
        }
    }
    // newest first: the first entry per target names the change, the last holds the value before the batch
    let mut by_target: Vec<(String, Subject, Value, String, bool)> = Vec::new();
    for e in state.changes.iter().filter(|e| e.batch.as_deref() == Some(batch_id)) {
        let id = e.subject.id();
        if let Some(seen) = by_target.iter_mut().find(|x| x.0 == id) {
            seen.2 = e.from.clone();
        } else {
            by_target.push((id, e.subject.clone(), e.from.clone(), e.id.clone(), e.note.is_some()));
        }
    }
    let mut skipped: Vec<SkippedItem> = Vec::new();
    let label = |t: &Subject, reason: &str| match t {
        Subject::Key(k) => SkippedItem { key: Some(bound(k, limits::ID)), command_id: None, reason: reason.into() },
        Subject::Command(c) => SkippedItem { key: None, command_id: Some(bound(c, limits::ID)), reason: reason.into() },
    };
    let mut writes: Vec<Write> = Vec::new();
    let mut wanted: Vec<Wanted> = Vec::new();
    for (id, t, prior, of, noted) in by_target {
        if noted {
            continue;
        }
        if js_eq(&current_of(state, ctx.schema, &t, ctx.commands), &prior) {
            continue;
        }
        if later.contains(&id) {
            skipped.push(label(&t, CHANGED_SINCE));
            continue;
        }
        if let Some(err) = restore_error(ctx, &t, &prior) {
            skipped.push(label(&t, &err));
            continue;
        }
        match &t {
            Subject::Command(c) => wanted.push(Wanted { command_id: c.clone(), chord: prior.as_str().map(str::to_string), of }),
            Subject::Key(_) => writes.push(restore_write(t, prior, &of, ctx)),
        }
    }
    let (accepted, rejected) = settle(&state.keys, &wanted, ctx.commands);
    for w in accepted {
        writes.push(restore_write(Subject::Command(w.command_id.clone()), Value::String(w.chord.clone().unwrap_or_default()), &w.of, ctx));
    }
    for (w, reason) in rejected {
        skipped.push(SkippedItem { key: None, command_id: Some(bound(&w.command_id, limits::ID)), reason });
    }
    match commit(state, writes, ctx, &ctx.who.clone().into(), true) {
        Err(mut e) => {
            e.skipped = Some(skipped);
            Err(e)
        }
        Ok(r) if r.entries.is_empty() => {
            let mut e = if skipped.is_empty() { fail("already", "It is already undone") } else { fail("not-possible", "Every setting in it was changed again since") };
            e.skipped = Some(skipped);
            Err(e)
        }
        Ok(mut r) => {
            r.skipped = skipped;
            Ok(r)
        }
    }
}

/// The newest import batch (`undo-import --last`).
pub fn last_import_batch(state: &SettingsState) -> Option<String> {
    state.changes.iter().find(|e| matches!(e.who, Who::Import(_)) && e.batch.is_some()).and_then(|e| e.batch.clone())
}
