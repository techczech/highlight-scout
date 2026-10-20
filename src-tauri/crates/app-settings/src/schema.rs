//! The settings schema (port of `@bridle/settings` schema.ts). Read from the same JSON a TS `defineSchema` input serialises to, so
//! one declaration drives the window (TS) and the store and CLI (Rust).

use crate::value::{finite, has_control, jlen, js_eq, js_string, norm};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingType {
    Toggle,
    Choice,
    Text,
    Number,
    Path,
    Chord,
    Secret,
}

impl SettingType {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "toggle" => Self::Toggle,
            "choice" => Self::Choice,
            "text" => Self::Text,
            "number" => Self::Number,
            "path" => Self::Path,
            "chord" => Self::Chord,
            "secret" => Self::Secret,
            _ => return None,
        })
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Toggle => "toggle",
            Self::Choice => "choice",
            Self::Text => "text",
            Self::Number => "number",
            Self::Path => "path",
            Self::Chord => "chord",
            Self::Secret => "secret",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChoiceOption {
    pub value: Value,
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct SettingDef {
    pub key: String,
    pub label: String,
    pub section: String,
    pub group: Option<String>,
    pub kind: SettingType,
    /// `Value::Null` for a secret.
    pub default: Value,
    pub options: Vec<ChoiceOption>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub unit: Option<String>,
    pub help: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Section {
    pub id: String,
    pub label: String,
    pub icon: String,
    pub shared: bool,
    pub pinned: bool,
}

/// Extra rule after the type check (TS `validate`), by setting.
pub type ValidateFn = Arc<dyn Fn(&SettingDef, &Value) -> Option<String> + Send + Sync>;
/// Whether an imported value can apply on this machine (TS `canApply`); a string is the skip reason.
pub type CanApplyFn = Arc<dyn Fn(&SettingDef, &Value) -> Option<String> + Send + Sync>;

#[derive(Clone)]
pub struct Schema {
    pub app: String,
    pub sections: Vec<Section>,
    pub settings: Vec<SettingDef>,
    by_key: HashMap<String, usize>,
    pub validate: Option<ValidateFn>,
    pub can_apply: Option<CanApplyFn>,
}

impl std::fmt::Debug for Schema {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Schema").field("app", &self.app).field("settings", &self.settings.len()).finish()
    }
}

const SHARED: [(&str, &str, &str); 6] = [
    ("general", "General", "settings"),
    ("appearance", "Appearance", "palette"),
    ("shortcuts", "Shortcuts", "keyboard"),
    ("accounts", "Accounts and keys", "key-round"),
    ("changes", "Changes", "clipboard-list"),
    ("about", "About", "info"),
];

fn str_field(o: &serde_json::Map<String, Value>, k: &str) -> Option<String> {
    o.get(k).and_then(Value::as_str).map(str::to_string)
}

impl Schema {
    /// `defineSchema` over the JSON form of its input: `{ app, sections?, settings }`. Errors are programming mistakes.
    pub fn from_json(input: &Value) -> Result<Schema, String> {
        let o = input.as_object().ok_or("schema must be an object")?;
        let app = str_field(o, "app").ok_or("schema has no app")?;
        let ok_app = !app.is_empty()
            && app.chars().next().is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
            && app.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || ".-_".contains(c));
        if !ok_app {
            return Err(format!("app id \"{app}\" must be lower case letters, digits, . _ -"));
        }
        let mut sections: Vec<Section> = Vec::new();
        let shared = |i: usize, pinned: bool| Section {
            id: SHARED[i].0.into(),
            label: SHARED[i].1.into(),
            icon: SHARED[i].2.into(),
            shared: true,
            pinned,
        };
        for i in 0..4 {
            sections.push(shared(i, false));
        }
        for s in o.get("sections").and_then(Value::as_array).cloned().unwrap_or_default() {
            let so = s.as_object().ok_or("section must be an object")?;
            sections.push(Section {
                id: str_field(so, "id").ok_or("section has no id")?,
                label: str_field(so, "label").unwrap_or_default(),
                icon: str_field(so, "icon").unwrap_or_default(),
                shared: false,
                pinned: false,
            });
        }
        sections.push(shared(4, true));
        sections.push(shared(5, true));
        let mut seen = std::collections::HashSet::new();
        for s in &sections {
            if !seen.insert(s.id.clone()) {
                return Err(format!("section \"{}\" is declared twice", s.id));
            }
        }
        let mut settings = Vec::new();
        let mut by_key = HashMap::new();
        for d in o.get("settings").and_then(Value::as_array).cloned().unwrap_or_default() {
            let dobj = d.as_object().ok_or("setting must be an object")?;
            let key = str_field(dobj, "key").unwrap_or_default();
            if key.is_empty() || by_key.contains_key(&key) {
                return Err(format!("setting key \"{key}\" is empty or declared twice"));
            }
            if jlen(&key) > 200 || key == "changes" {
                return Err(format!("setting key \"{key}\" is too long or reserved"));
            }
            let section = str_field(dobj, "section").unwrap_or_default();
            if !sections.iter().any(|s| s.id == section && !s.pinned) {
                return Err(format!("setting \"{key}\" names section \"{section}\", which settings cannot live in"));
            }
            let kind = dobj
                .get("type")
                .and_then(Value::as_str)
                .and_then(SettingType::parse)
                .ok_or_else(|| format!("setting \"{key}\" has no valid type"))?;
            let options: Vec<ChoiceOption> = dobj
                .get("options")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|x| {
                            let xo = x.as_object()?;
                            Some(ChoiceOption { value: norm(xo.get("value")?.clone()), label: str_field(xo, "label").unwrap_or_default() })
                        })
                        .collect()
                })
                .unwrap_or_default();
            if kind == SettingType::Choice && options.is_empty() {
                return Err(format!("choice setting \"{key}\" has no options"));
            }
            let def = SettingDef {
                key: key.clone(),
                label: str_field(dobj, "label").unwrap_or_default(),
                section,
                group: str_field(dobj, "group"),
                kind,
                default: norm(dobj.get("default").cloned().unwrap_or(Value::Null)),
                options,
                min: dobj.get("min").and_then(Value::as_f64),
                max: dobj.get("max").and_then(Value::as_f64),
                unit: str_field(dobj, "unit"),
                help: str_field(dobj, "help"),
            };
            by_key.insert(key, settings.len());
            settings.push(def);
        }
        let mut schema = Schema { app, sections, settings, by_key, validate: None, can_apply: None };
        for d in &schema.settings {
            if d.kind == SettingType::Secret {
                if !d.default.is_null() {
                    return Err(format!("secret setting \"{}\" must have default null", d.key));
                }
            } else {
                let err = if d.default.is_null() { Some("has no default".to_string()) } else { schema.check_value(d, &d.default) };
                if let Some(e) = err {
                    return Err(format!("setting \"{}\" default: {e}", d.key));
                }
            }
        }
        schema.by_key.shrink_to_fit();
        Ok(schema)
    }

    pub fn with_validate(mut self, f: ValidateFn) -> Self {
        self.validate = Some(f);
        self
    }

    pub fn with_can_apply(mut self, f: CanApplyFn) -> Self {
        self.can_apply = Some(f);
        self
    }

    pub fn def(&self, key: &str) -> Option<&SettingDef> {
        self.by_key.get(key).map(|&i| &self.settings[i])
    }

    pub fn section(&self, id: &str) -> Option<&Section> {
        self.sections.iter().find(|s| s.id == id)
    }

    /// Why `value` cannot be this setting's value, or None when it can (`checkValue`).
    pub fn check_value(&self, def: &SettingDef, value: &Value) -> Option<String> {
        match def.kind {
            SettingType::Secret => return Some("Keys are kept in Keychain, never in the settings file".into()),
            SettingType::Toggle => {
                if !value.is_boolean() {
                    return Some("Must be on or off".into());
                }
            }
            SettingType::Choice => {
                if !def.options.iter().any(|o| js_eq(&o.value, value) && same_type(&o.value, value)) {
                    return Some("Not one of the choices".into());
                }
            }
            SettingType::Number => {
                let Some(n) = (if value.is_number() { finite(value) } else { None }) else {
                    return Some("Must be a number".into());
                };
                if let Some(min) = def.min {
                    if n < min {
                        return Some(format!("Must be at least {}", js_string(&crate::value::num(min))));
                    }
                }
                if let Some(max) = def.max {
                    if n > max {
                        return Some(format!("Must be at most {}", js_string(&crate::value::num(max))));
                    }
                }
            }
            SettingType::Text => {
                let Some(s) = value.as_str() else { return Some("Must be text".into()) };
                if jlen(s) > MAX_TEXT || s.contains('\u{0}') {
                    return Some("That text cannot be saved".into());
                }
            }
            SettingType::Path => {
                let Some(s) = value.as_str() else { return Some("Must be a path".into()) };
                if jlen(s) > MAX_TEXT || has_control(s) {
                    return Some("That path cannot be saved".into());
                }
            }
            SettingType::Chord => {
                if let Some(e) = chord_shape_error(value) {
                    return Some(e);
                }
            }
        }
        self.validate.as_ref().and_then(|f| f(def, value))
    }

    /// Typed text (a CLI argument) to a checked value (`parseValue`).
    pub fn parse_value(&self, def: &SettingDef, raw: &str) -> Result<Value, String> {
        let value = match def.kind {
            SettingType::Secret => return Err("Keys are set in Accounts and keys, not here".into()),
            SettingType::Toggle => {
                let t = raw.trim().to_lowercase();
                if ["on", "true", "yes", "1"].contains(&t.as_str()) {
                    Value::Bool(true)
                } else if ["off", "false", "no", "0"].contains(&t.as_str()) {
                    Value::Bool(false)
                } else {
                    return Err("Must be on or off".into());
                }
            }
            SettingType::Number => {
                let n = js_number(raw).ok_or("Must be a number")?;
                crate::value::num(n)
            }
            SettingType::Choice => {
                let t = raw.trim();
                let o = def
                    .options
                    .iter()
                    .find(|x| js_string(&x.value) == t)
                    .or_else(|| def.options.iter().find(|x| x.label.to_lowercase() == t.to_lowercase()));
                match o {
                    Some(o) => o.value.clone(),
                    None => {
                        let list: Vec<String> = def.options.iter().map(|x| js_string(&x.value)).collect();
                        return Err(format!("Not one of the choices: {}", list.join(", ")));
                    }
                }
            }
            _ => Value::String(raw.to_string()),
        };
        match self.check_value(def, &value) {
            Some(e) => Err(e),
            None => Ok(value),
        }
    }
}

fn same_type(a: &Value, b: &Value) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
}

/// `Number(raw)` for typed text: blank is not a number; hex, exponents and decimals are.
fn js_number(raw: &str) -> Option<f64> {
    let t = raw.trim();
    if t.is_empty() {
        return None;
    }
    let n = if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        i64::from_str_radix(h, 16).ok()? as f64
    } else if t.chars().any(|c| c.is_alphabetic() && c != 'e' && c != 'E') {
        return None;
    } else {
        t.parse::<f64>().ok()?
    };
    n.is_finite().then_some(n)
}

pub const MAX_TEXT: usize = 4096;
const MAX_CHORD: usize = 32;

/// `chordShapeError`: "" or a short string without control characters.
pub fn chord_shape_error(chord: &Value) -> Option<String> {
    let Some(s) = chord.as_str() else { return Some("A shortcut must be text".into()) };
    if jlen(s) > MAX_CHORD || has_control(s) {
        return Some("That is not a key combination".into());
    }
    None
}

/// A value in the user's words (`formatValue`).
pub fn format_value(def: Option<&SettingDef>, value: &Value) -> String {
    if value.is_null() {
        return "—".into();
    }
    let Some(def) = def else { return js_string(value) };
    match def.kind {
        SettingType::Toggle => if value.as_bool() == Some(true) { "On".into() } else { "Off".into() },
        SettingType::Choice => def.options.iter().find(|o| js_eq(&o.value, value)).map(|o| o.label.clone()).unwrap_or_else(|| js_string(value)),
        SettingType::Number => match &def.unit {
            Some(u) if !u.is_empty() => format!("{} {u}", js_string(value)),
            _ => js_string(value),
        },
        SettingType::Chord => if value.as_str() == Some("") { "None".into() } else { js_string(value) },
        _ => js_string(value),
    }
}
