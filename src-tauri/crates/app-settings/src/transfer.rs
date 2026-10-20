//! Export and import (port of transfer.ts). Export: settings changed from default and shortcut overrides, never keys. Import: a preview,
//! then one batch attributed to the file.

use crate::changes::{limits, Actor, ImportWho, SkippedItem, Who, MAX_CHANGES};
use crate::commands::CommandsAdapter;
use crate::schema::{Schema, SettingType};
use crate::state::{chord_of, command_id_error, commit, fail, override_error, settle, value_of, Ctx, OpResult, SettingsState, Wanted, Write};
use crate::changes::Subject;
use crate::value::{bound, iso, js_eq, norm, Obj};
use serde_json::{json, Value};

pub const EXPORT_FORMAT: &str = "bridle-settings-export";
pub const EXPORT_VERSION: u64 = 1;

/// The export file for this state (`exportFile`).
pub fn export_file(state: &SettingsState, schema: &Schema, commands: &dyn CommandsAdapter, app_version: &str, host_name: &str, now: f64) -> Value {
    let mut settings = Obj::new();
    for (k, v) in &state.settings {
        if schema.def(k).is_some_and(|d| d.kind != SettingType::Secret) {
            settings.insert(k.clone(), v.clone());
        }
    }
    let mut keys = Obj::new();
    for (id, chord) in &state.keys {
        if command_id_error(id, schema, commands).is_none() {
            keys.insert(id.clone(), chord.clone());
        }
    }
    json!({
        "format": EXPORT_FORMAT, "version": EXPORT_VERSION, "app": state.app, "appVersion": app_version, "exportedFrom": host_name,
        "exportedAt": iso(now), "settings": Value::Object(settings), "keys": Value::Object(keys),
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportChange {
    pub subject: Subject,
    pub from: Value,
    pub to: Value,
    pub section: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Refused {
    /// `unparsable`, `wrong-format`, `other-app`, `newer-version`, `too-many`.
    pub code: &'static str,
    pub message: String,
    pub app: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Source {
    pub app: String,
    pub app_version: Option<String>,
    pub exported_from: Option<String>,
    pub exported_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ImportPreview {
    pub changes: Vec<ImportChange>,
    pub skipped: Vec<SkippedItem>,
    pub already_match: usize,
    pub refused: Option<Refused>,
    pub source: Option<Source>,
}

impl ImportPreview {
    /// The TS `ImportPreview` shape.
    pub fn to_json(&self) -> Value {
        let mut o = Obj::new();
        o.insert(
            "changes".into(),
            Value::Array(
                self.changes
                    .iter()
                    .map(|c| {
                        let mut x = Obj::new();
                        match &c.subject {
                            Subject::Key(k) => x.insert("key".into(), json!(k)),
                            Subject::Command(k) => x.insert("commandId".into(), json!(k)),
                        };
                        x.insert("from".into(), c.from.clone());
                        x.insert("to".into(), c.to.clone());
                        x.insert("section".into(), json!(c.section));
                        Value::Object(x)
                    })
                    .collect(),
            ),
        );
        o.insert("skipped".into(), Value::Array(self.skipped.iter().map(SkippedItem::to_json).collect()));
        o.insert("alreadyMatch".into(), json!(self.already_match));
        if let Some(r) = &self.refused {
            let mut x = Obj::new();
            x.insert("code".into(), json!(r.code));
            x.insert("message".into(), json!(r.message));
            if let Some(a) = &r.app {
                x.insert("app".into(), json!(a));
            }
            o.insert("refused".into(), Value::Object(x));
        }
        if let Some(s) = &self.source {
            let mut x = Obj::new();
            x.insert("app".into(), json!(s.app));
            for (k, v) in [("appVersion", &s.app_version), ("exportedFrom", &s.exported_from), ("exportedAt", &s.exported_at)] {
                if let Some(v) = v {
                    x.insert(k.into(), json!(v));
                }
            }
            o.insert("source".into(), Value::Object(x));
        }
        Value::Object(o)
    }

    /// Reads a preview handed back by the window (the TS shape). Values are checked again on apply.
    pub fn from_json(v: &Value) -> Option<ImportPreview> {
        let o = v.as_object()?;
        let opt = |x: &Obj, k: &str| x.get(k).and_then(Value::as_str).map(str::to_string);
        let changes = o
            .get("changes")?
            .as_array()?
            .iter()
            .map(|c| {
                let c = c.as_object()?;
                let subject = match (opt(c, "key"), opt(c, "commandId")) {
                    (Some(k), None) => Subject::Key(k),
                    (None, Some(k)) => Subject::Command(k),
                    _ => return None,
                };
                Some(ImportChange { subject, from: norm(c.get("from").cloned().unwrap_or(Value::Null)), to: norm(c.get("to")?.clone()), section: opt(c, "section").unwrap_or_default() })
            })
            .collect::<Option<Vec<_>>>()?;
        let skipped = o.get("skipped").and_then(Value::as_array).map(|a| a.iter().filter_map(SkippedItem::from_json).collect()).unwrap_or_default();
        let refused = o.get("refused").and_then(Value::as_object).map(|r| Refused {
            code: "wrong-format",
            message: opt(r, "message").unwrap_or_else(|| "This is not an exported settings file".into()),
            app: opt(r, "app"),
        });
        let source = o.get("source").and_then(Value::as_object).map(|s| Source {
            app: opt(s, "app").unwrap_or_default(),
            app_version: opt(s, "appVersion"),
            exported_from: opt(s, "exportedFrom"),
            exported_at: opt(s, "exportedAt"),
        });
        Some(ImportPreview { changes, skipped, already_match: o.get("alreadyMatch").and_then(Value::as_u64).unwrap_or(0) as usize, refused, source })
    }
}

fn refuse(code: &'static str, message: impl Into<String>, app: Option<String>) -> ImportPreview {
    ImportPreview { refused: Some(Refused { code, message: message.into(), app }), ..Default::default() }
}

fn opt_str(v: Option<&Value>) -> Option<String> {
    v.and_then(Value::as_str).map(|s| bound(s, limits::NAME))
}

/// What importing a file's text would do (`previewImport`). Never fails.
pub fn preview_import(state: &SettingsState, text: &str, schema: &Schema, commands: &dyn CommandsAdapter) -> ImportPreview {
    let Ok(raw) = serde_json::from_str::<Value>(text) else { return refuse("unparsable", "This file could not be read as a settings file", None) };
    preview_import_value(state, &raw, schema, commands)
}

pub fn preview_import_value(state: &SettingsState, raw: &Value, schema: &Schema, commands: &dyn CommandsAdapter) -> ImportPreview {
    let Some(r) = raw.as_object() else { return refuse("unparsable", "This file could not be read as a settings file", None) };
    if r.get("format").and_then(Value::as_str) != Some(EXPORT_FORMAT) {
        return refuse("wrong-format", "This is not an exported settings file", None);
    }
    let v = r.get("version").and_then(Value::as_f64);
    if !v.is_some_and(|v| v.fract() == 0.0 && v >= 1.0) {
        return refuse("wrong-format", "This is not an exported settings file", None);
    }
    if v.unwrap_or(0.0) > EXPORT_VERSION as f64 {
        return refuse("newer-version", "This file was exported by a newer version of the app; update the app first", None);
    }
    if r.get("app").and_then(Value::as_str) != Some(state.app.as_str()) {
        let other = opt_str(r.get("app")).filter(|s| !s.is_empty());
        let msg = match &other {
            Some(o) => format!("These are settings for {o}, not for this app"),
            None => "These settings are for another app".into(),
        };
        return refuse("other-app", msg, opt_str(r.get("app")));
    }
    let empty = Obj::new();
    let settings = r.get("settings").and_then(Value::as_object).unwrap_or(&empty);
    let keys = r.get("keys").and_then(Value::as_object).unwrap_or(&empty);
    let mut p = ImportPreview {
        source: Some(Source {
            app: state.app.clone(),
            app_version: opt_str(r.get("appVersion")),
            exported_from: opt_str(r.get("exportedFrom")),
            exported_at: opt_str(r.get("exportedAt")),
        }),
        ..Default::default()
    };
    for (key, value) in settings {
        let value = norm(value.clone());
        let Some(def) = schema.def(key) else {
            p.skipped.push(SkippedItem::key(key, "This version of the app has no such setting"));
            continue;
        };
        if def.kind == SettingType::Secret {
            p.skipped.push(SkippedItem::key(key, "Keys are never imported"));
            continue;
        }
        if let Some(e) = schema.check_value(def, &value) {
            p.skipped.push(SkippedItem::key(key, &e));
            continue;
        }
        let from = value_of(state, schema, key).unwrap_or(Value::Null);
        if js_eq(&from, &value) {
            p.already_match += 1;
            continue;
        }
        if let Some(why) = schema.can_apply.as_ref().and_then(|f| f(def, &value)) {
            p.skipped.push(SkippedItem::key(key, &why));
            continue;
        }
        p.changes.push(ImportChange { subject: Subject::Key(key.clone()), from, to: value, section: def.section.clone() });
    }
    let mut wanted: Vec<Wanted> = Vec::new();
    for (id, chord) in keys {
        if let Some(e) = command_id_error(id, schema, commands) {
            let reason = if schema.def(id).is_some() { e } else { "This version of the app has no such command".into() };
            p.skipped.push(SkippedItem::cmd(id, &reason));
            continue;
        }
        if let Some(e) = override_error(id, chord, schema, commands) {
            p.skipped.push(SkippedItem::cmd(id, &e));
            continue;
        }
        let c = chord.as_str().unwrap_or("").to_string();
        if chord_of(state, id, commands) == c {
            p.already_match += 1;
            continue;
        }
        wanted.push(Wanted { command_id: id.clone(), chord: Some(c), of: String::new() });
    }
    let (accepted, rejected) = settle(&state.keys, &wanted, commands);
    for w in &wanted {
        if accepted.iter().any(|a| a.command_id == w.command_id) {
            p.changes.push(ImportChange {
                subject: Subject::Command(w.command_id.clone()),
                from: Value::String(chord_of(state, &w.command_id, commands)),
                to: Value::String(w.chord.clone().unwrap_or_default()),
                section: "shortcuts".into(),
            });
        }
    }
    for (w, reason) in rejected {
        p.skipped.push(SkippedItem::cmd(&w.command_id, &reason));
    }
    if p.changes.len() > MAX_CHANGES {
        return refuse("too-many", format!("This file would change {} settings; at most {MAX_CHANGES} can be imported at once", p.changes.len()), None);
    }
    p
}

/// Applies a preview as one batch, `who: import` (`applyImport`). `ctx.who` other than you is recorded as `by`.
pub fn apply_import(state: &SettingsState, preview: &ImportPreview, file: &str, ctx: &Ctx) -> OpResult {
    if let Some(r) = &preview.refused {
        return Err(fail("not-possible", r.message.clone()));
    }
    // A preview is only ever for this app (`preview_import` refuses any other); one that says otherwise was not made here.
    if preview.source.as_ref().map(|s| s.app.as_str()) != Some(state.app.as_str()) {
        return Err(fail("not-possible", "These settings are for another app"));
    }
    let mut writes = Vec::new();
    for c in &preview.changes {
        match &c.subject {
            Subject::Key(k) => {
                let ok = ctx.schema.def(k).is_some_and(|d| d.kind != SettingType::Secret && ctx.schema.check_value(d, &c.to).is_none());
                if !ok {
                    return Err(fail("invalid-value", format!("\"{k}\" can no longer take that value")));
                }
                if let Some(why) = ctx.schema.def(k).and_then(|d| ctx.schema.can_apply.as_ref().and_then(|f| f(d, &c.to))) {
                    return Err(fail("not-possible", format!("\"{k}\" cannot be imported: {why}")));
                }
                writes.push(Write { target: c.subject.clone(), to: c.to.clone(), to_default: false, who: None });
            }
            Subject::Command(id) => {
                if override_error(id, &c.to, ctx.schema, ctx.commands).is_some() {
                    return Err(fail("invalid-chord", format!("\"{id}\" can no longer take that shortcut")));
                }
                writes.push(Write { target: c.subject.clone(), to: c.to.clone(), to_default: false, who: None });
            }
        }
    }
    let src = preview.source.clone().unwrap_or_default();
    let b = |s: &Option<String>| s.as_ref().map(|x| bound(x, limits::NAME));
    let mut who = ImportWho { file: bound(file, limits::FILE), exported_from: b(&src.exported_from), exported_at: b(&src.exported_at), app_version: b(&src.app_version), skipped: None, by: None };
    if !preview.skipped.is_empty() {
        who.skipped = Some(
            preview
                .skipped
                .iter()
                .take(limits::SKIPPED)
                .map(|s| match &s.key {
                    Some(k) => SkippedItem::key(k, &s.reason),
                    None => SkippedItem::cmd(s.command_id.as_deref().unwrap_or(""), &s.reason),
                })
                .collect(),
        );
    }
    if let Actor::Agent(_) = &ctx.who {
        who.by = Some(ctx.who.clone());
    }
    commit(state, writes, ctx, &Who::Import(who), true)
}
