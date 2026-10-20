//! The changelog (port of changes.ts): one entry per setting or shortcut changed, with who. Newest first, at most 300 entries, a batch
//! never split. Entries are rebuilt from their known fields when read, and written in that same field order.

use crate::value::{finite, jlen, num, Obj};
use serde_json::{json, Value};

pub const MAX_CHANGES: usize = 300;
pub const REPAIR_SUMMARY_LIMIT: usize = 20;
pub const CHANGELOG_KEY: &str = "changes";
pub const INVALID_SAVED_VALUE: &str = "INVALID_SAVED_VALUE";
pub const INVALID_SAVED_SETTINGS: &str = "INVALID_SAVED_SETTINGS";
pub const INVALID_SAVED_SHORTCUTS: &str = "INVALID_SAVED_SHORTCUTS";
const NOTES: [&str; 3] = [INVALID_SAVED_VALUE, INVALID_SAVED_SETTINGS, INVALID_SAVED_SHORTCUTS];

pub mod limits {
    pub const ID: usize = 200;
    pub const NAME: usize = 200;
    pub const FILE: usize = 1024;
    pub const TEXT: usize = 4096;
    pub const SKIPPED: usize = 50;
}

#[derive(Debug, Clone, PartialEq)]
pub enum Actor {
    You,
    Agent(String),
}

impl Actor {
    pub fn to_json(&self) -> Value {
        match self {
            Actor::You => json!({ "kind": "you" }),
            Actor::Agent(n) => json!({ "kind": "agent", "name": n }),
        }
    }
    pub fn bounded(&self) -> Actor {
        match self {
            Actor::Agent(n) => Actor::Agent(crate::value::bound(n, limits::NAME)),
            Actor::You => Actor::You,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SkippedItem {
    pub key: Option<String>,
    pub command_id: Option<String>,
    pub reason: String,
}

impl SkippedItem {
    pub fn key(k: &str, reason: &str) -> Self {
        SkippedItem { key: Some(crate::value::bound(k, limits::ID)), command_id: None, reason: crate::value::bound(reason, limits::TEXT) }
    }
    pub fn cmd(c: &str, reason: &str) -> Self {
        SkippedItem { key: None, command_id: Some(crate::value::bound(c, limits::ID)), reason: crate::value::bound(reason, limits::TEXT) }
    }
    pub fn to_json(&self) -> Value {
        let mut o = Obj::new();
        if let Some(k) = &self.key {
            o.insert("key".into(), json!(k));
        }
        if let Some(c) = &self.command_id {
            o.insert("commandId".into(), json!(c));
        }
        o.insert("reason".into(), json!(self.reason));
        Value::Object(o)
    }
    pub fn from_json(v: &Value) -> Option<Self> {
        let o = v.as_object()?;
        let reason = is_str(o.get("reason"), limits::TEXT)?;
        if let (Some(k), None) = (is_str(o.get("key"), limits::ID), o.get("commandId")) {
            return Some(SkippedItem { key: Some(k), command_id: None, reason });
        }
        if let (Some(c), None) = (is_str(o.get("commandId"), limits::ID), o.get("key")) {
            return Some(SkippedItem { key: None, command_id: Some(c), reason });
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ImportWho {
    pub file: String,
    pub exported_from: Option<String>,
    pub exported_at: Option<String>,
    pub app_version: Option<String>,
    pub skipped: Option<Vec<SkippedItem>>,
    pub by: Option<Actor>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Who {
    You,
    Agent(String),
    AppUpdate(String),
    Import(ImportWho),
    Reset { of: String, by: Option<Actor> },
}

impl From<Actor> for Who {
    fn from(a: Actor) -> Who {
        match a {
            Actor::You => Who::You,
            Actor::Agent(n) => Who::Agent(n),
        }
    }
}

impl Who {
    pub fn kind(&self) -> &'static str {
        match self {
            Who::You => "you",
            Who::Agent(_) => "agent",
            Who::AppUpdate(_) => "app-update",
            Who::Import(_) => "import",
            Who::Reset { .. } => "reset",
        }
    }
    pub fn to_json(&self) -> Value {
        match self {
            Who::You => Actor::You.to_json(),
            Who::Agent(n) => Actor::Agent(n.clone()).to_json(),
            Who::AppUpdate(v) => json!({ "kind": "app-update", "version": v }),
            Who::Reset { of, by } => {
                let mut o = Obj::new();
                o.insert("kind".into(), json!("reset"));
                o.insert("of".into(), json!(of));
                if let Some(b) = by {
                    o.insert("by".into(), b.to_json());
                }
                Value::Object(o)
            }
            Who::Import(w) => {
                let mut o = Obj::new();
                o.insert("kind".into(), json!("import"));
                o.insert("file".into(), json!(w.file));
                for (k, v) in [("exportedFrom", &w.exported_from), ("exportedAt", &w.exported_at), ("appVersion", &w.app_version)] {
                    if let Some(v) = v {
                        o.insert(k.into(), json!(v));
                    }
                }
                if let Some(s) = &w.skipped {
                    o.insert("skipped".into(), Value::Array(s.iter().map(SkippedItem::to_json).collect()));
                }
                if let Some(b) = &w.by {
                    o.insert("by".into(), b.to_json());
                }
                Value::Object(o)
            }
        }
    }
    /// `readWho`: rebuilt from its known fields only.
    pub fn from_json(v: &Value) -> Option<Who> {
        if let Some(a) = read_actor(v) {
            return Some(a.into());
        }
        let o = v.as_object()?;
        match o.get("kind")?.as_str()? {
            "app-update" => Some(Who::AppUpdate(is_str(o.get("version"), limits::NAME)?)),
            "reset" => {
                let of = is_str(o.get("of"), limits::ID)?;
                match o.get("by") {
                    None => Some(Who::Reset { of, by: None }),
                    Some(b) => Some(Who::Reset { of, by: Some(read_actor(b)?) }),
                }
            }
            "import" => {
                let mut w = ImportWho { file: is_str(o.get("file"), limits::FILE)?, ..Default::default() };
                for (k, slot) in [("exportedFrom", &mut w.exported_from), ("exportedAt", &mut w.exported_at), ("appVersion", &mut w.app_version)] {
                    if let Some(x) = o.get(k) {
                        *slot = Some(is_str(Some(x), limits::NAME)?);
                    }
                }
                if let Some(s) = o.get("skipped") {
                    let a = s.as_array()?;
                    if a.len() > limits::SKIPPED {
                        return None;
                    }
                    w.skipped = Some(a.iter().map(SkippedItem::from_json).collect::<Option<Vec<_>>>()?);
                }
                if let Some(b) = o.get("by") {
                    w.by = Some(read_actor(b)?);
                }
                Some(Who::Import(w))
            }
            _ => None,
        }
    }
    /// `actorOf`: the Changes filter group; a reset counts for whoever pressed it.
    pub fn actor_kind(&self) -> &'static str {
        match self {
            Who::Reset { by: Some(Actor::Agent(_)), .. } => "agent",
            Who::Reset { .. } => "you",
            other => other.kind(),
        }
    }
}

fn read_actor(v: &Value) -> Option<Actor> {
    let o = v.as_object()?;
    match o.get("kind")?.as_str()? {
        "you" => Some(Actor::You),
        "agent" => Some(Actor::Agent(is_str(o.get("name"), limits::NAME)?)),
        _ => None,
    }
}

pub(crate) fn is_str(v: Option<&Value>, max: usize) -> Option<String> {
    v.and_then(Value::as_str).filter(|s| jlen(s) <= max).map(str::to_string)
}

fn is_value(v: Option<&Value>) -> Option<Value> {
    match v? {
        Value::Null => Some(Value::Null),
        Value::Bool(b) => Some(Value::Bool(*b)),
        n @ Value::Number(_) => finite(n).map(num),
        Value::String(s) if jlen(s) <= limits::TEXT => Some(Value::String(s.clone())),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Subject {
    Key(String),
    Command(String),
}

impl Subject {
    pub fn id(&self) -> String {
        match self {
            Subject::Key(k) => format!("setting:{k}"),
            Subject::Command(c) => format!("key:{c}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChangeEntry {
    pub id: String,
    /// ms since 1970.
    pub at: f64,
    pub subject: Subject,
    pub from: Value,
    pub to: Value,
    pub who: Who,
    pub batch: Option<String>,
    pub note: Option<String>,
    pub targets: Option<Vec<String>>,
    /// Written by the loader (`readValues`): its fields go in that order (`note`, `targets` before `batch`), as the TS loader writes
    /// them. Entries read from a file are written in `readEntry` order.
    pub loader_order: bool,
}

impl ChangeEntry {
    pub fn key(&self) -> Option<&str> {
        match &self.subject {
            Subject::Key(k) => Some(k),
            _ => None,
        }
    }
    pub fn command_id(&self) -> Option<&str> {
        match &self.subject {
            Subject::Command(c) => Some(c),
            _ => None,
        }
    }
    pub fn to_json(&self) -> Value {
        let mut o = Obj::new();
        o.insert("id".into(), json!(self.id));
        o.insert("at".into(), num(self.at));
        match &self.subject {
            Subject::Key(k) => o.insert("key".into(), json!(k)),
            Subject::Command(c) => o.insert("commandId".into(), json!(c)),
        };
        o.insert("from".into(), self.from.clone());
        o.insert("to".into(), self.to.clone());
        o.insert("who".into(), self.who.to_json());
        if !self.loader_order {
            if let Some(b) = &self.batch {
                o.insert("batch".into(), json!(b));
            }
        }
        if let Some(n) = &self.note {
            o.insert("note".into(), json!(n));
        }
        if let Some(t) = &self.targets {
            o.insert("targets".into(), json!(t));
        }
        if self.loader_order {
            if let Some(b) = &self.batch {
                o.insert("batch".into(), json!(b));
            }
        }
        Value::Object(o)
    }
    /// `readEntry`.
    pub fn from_json(v: &Value) -> Option<ChangeEntry> {
        let o = v.as_object()?;
        let id = is_str(o.get("id"), limits::ID)?;
        let at = o.get("at").filter(|x| x.is_number()).and_then(finite)?;
        let has_key = o.contains_key("key");
        if has_key == o.contains_key("commandId") {
            return None;
        }
        let subject = if has_key { Subject::Key(is_str(o.get("key"), limits::ID)?) } else { Subject::Command(is_str(o.get("commandId"), limits::ID)?) };
        let from = is_value(o.get("from"))?;
        let to = is_value(o.get("to"))?;
        let who = Who::from_json(o.get("who")?)?;
        let batch = match o.get("batch") {
            None => None,
            Some(b) => Some(is_str(Some(b), limits::ID)?),
        };
        let note = match o.get("note") {
            None => None,
            Some(n) => Some(n.as_str().filter(|s| NOTES.contains(s))?.to_string()),
        };
        let summary = note.as_deref().is_some_and(is_summary_note);
        let targets = if summary { o.get("targets").and_then(Value::as_array).map(|a| sorted_targets(a.iter().filter_map(Value::as_str))) } else { None };
        Some(ChangeEntry { id, at, subject, from, to, who, batch, note, targets, loader_order: false })
    }
    /// `isSecretEntry`: holds only what `recordSecret` writes.
    pub fn is_secret_entry(&self) -> bool {
        matches!(self.subject, Subject::Key(_))
            && is_secret_status(&self.from)
            && is_secret_status(&self.to)
            && matches!(self.who, Who::You | Who::Agent(_))
            && self.batch.is_none()
            && self.note.is_none()
            && self.targets.is_none()
    }
}

pub fn is_summary_note(n: &str) -> bool {
    n == INVALID_SAVED_SETTINGS || n == INVALID_SAVED_SHORTCUTS
}

pub fn is_secret_status(v: &Value) -> bool {
    matches!(v.as_str(), Some("connected") | Some("missing"))
}

/// Bounded ids, no repeats, sorted (by UTF-16 code units, as JavaScript sorts).
pub fn sorted_targets<'a>(ids: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut v: Vec<String> = ids.filter(|s| !s.is_empty() && jlen(s) <= limits::ID).map(str::to_string).collect();
    v.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    v.dedup();
    v
}

/// Groups that must stay together, in list order: a batch, or a single entry.
pub fn groups_of(list: &[ChangeEntry]) -> Vec<Vec<ChangeEntry>> {
    let mut order: Vec<String> = Vec::new();
    let mut groups: std::collections::HashMap<String, Vec<ChangeEntry>> = std::collections::HashMap::new();
    for (i, e) in list.iter().enumerate() {
        let g = match &e.batch {
            Some(b) => format!("b:{b}"),
            None => format!("e:{i}"),
        };
        if !groups.contains_key(&g) {
            order.push(g.clone());
        }
        groups.entry(g).or_default().push(e.clone());
    }
    order.into_iter().map(|g| groups.remove(&g).unwrap_or_default()).collect()
}

/// `capChanges`.
pub fn cap_changes(list: Vec<ChangeEntry>, cap: usize) -> Vec<ChangeEntry> {
    if list.len() <= cap {
        return list;
    }
    let mut out = Vec::new();
    for g in groups_of(&list) {
        if g.len() > cap {
            continue;
        }
        if out.len() + g.len() > cap {
            break;
        }
        out.extend(g);
    }
    out
}
