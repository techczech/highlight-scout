//! Shortcut judging (port of the parts of `@bridle/commands` the settings core uses: chords, `resolveBindings`, `checkRebind`,
//! `rebindMessage`, and `commandsAdapter`). The command list is read from the JSON form of a TS registry's `defs`
//! (`{ id, label, key, fixed? }[]`), so the TS registry stays the one list.

use crate::value::{has_control, jlen};
use serde_json::Value;
use std::collections::HashMap;

const MODS: [char; 4] = ['⌘', '⌃', '⌥', '⇧'];

fn supported(key: &str) -> bool {
    if key.chars().count() == 1 {
        let c = key.chars().next().unwrap_or(' ');
        if c.is_ascii_uppercase() || c.is_ascii_digit() {
            return true;
        }
    }
    if let Some(n) = key.strip_prefix('F') {
        if let Ok(i) = n.parse::<u32>() {
            if (1..=24).contains(&i) && !n.starts_with('0') {
                return true;
            }
        }
    }
    [",", ".", "/", ";", "'", "\\", "[", "]", "`", "=", "-", "↵", "⌫", "⌦", "⇥", "Space", "↑", "↓", "←", "→", "Esc"].contains(&key)
}

/// `isValidChord`.
pub fn is_valid_chord(chord: &str) -> bool {
    if chord.is_empty() || jlen(chord) > 12 || has_control(chord) {
        return false;
    }
    let mut rest = chord;
    for m in MODS {
        if let Some(r) = rest.strip_prefix(m) {
            if r.is_empty() {
                return false;
            }
            rest = r;
        }
    }
    supported(rest)
}

/// `splitChord`: modifiers present (in ⌘⌃⌥⇧ order) and the key with every modifier removed.
pub fn split_chord(chord: &str) -> (Vec<char>, String) {
    let mods = MODS.iter().copied().filter(|m| chord.contains(*m)).collect();
    let key = chord.chars().filter(|c| !MODS.contains(c)).collect();
    (mods, key)
}

/// `canonChord`.
pub fn canon_chord(chord: &str) -> String {
    let (mods, key) = split_chord(chord.trim());
    if key.is_empty() {
        return String::new();
    }
    let key = if jlen(&key) == 1 { key.to_uppercase() } else { key };
    mods.into_iter().collect::<String>() + &key
}

fn is_fkey(key: &str) -> bool {
    key.strip_prefix('F').is_some_and(|n| (1..=2).contains(&n.len()) && n.chars().all(|c| c.is_ascii_digit()))
}

/// `isTypingSafe`.
pub fn is_typing_safe(chord: &str) -> bool {
    let (mods, key) = split_chord(chord);
    mods.iter().any(|&m| m != '⇧') || is_fkey(&key)
}

/// `toAccelerator` (Tauri and Electron accelerators share the shape).
pub fn to_accelerator(chord: &str) -> Option<String> {
    let (mods, key) = split_chord(chord);
    if key.is_empty() || (mods.is_empty() && !is_fkey(&key)) {
        return None;
    }
    let mut parts: Vec<String> = mods
        .iter()
        .map(|m| match m {
            '⌘' => "Cmd",
            '⌃' => "Ctrl",
            '⌥' => "Alt",
            _ => "Shift",
        })
        .map(str::to_string)
        .collect();
    let k = match key.as_str() {
        "↑" => "Up",
        "↓" => "Down",
        "←" => "Left",
        "→" => "Right",
        "↵" => "Return",
        "⌫" => "Backspace",
        "⌦" => "Delete",
        "⇥" => "Tab",
        other => other,
    };
    parts.push(k.to_string());
    Some(parts.join("+"))
}

pub const RESERVED_SYSTEM: [(&str, &str); 10] = [
    ("⌘Q", "Quit"),
    ("⌘W", "Close the window"),
    ("⌘H", "Hide the app"),
    ("⌘M", "Minimise"),
    ("⌘C", "Copy"),
    ("⌘X", "Cut"),
    ("⌘V", "Paste"),
    ("⌘A", "Select all"),
    ("⌘Z", "Undo"),
    ("⌘⇧Z", "Redo"),
];
pub const RESERVED_ESTATE: [(&str, &str); 8] = [
    ("⌘⇧P", "Command palette"),
    ("⌘⇧K", "Jump to"),
    ("⌘K", "Actions on the selection"),
    ("⌘P", "Show or hide the side pane"),
    ("⌘F", "Find"),
    ("⌘⇧F", "Search everything"),
    ("⌘,", "Settings"),
    ("⌘/", "Keyboard cheat sheet"),
];

#[derive(Debug, Clone)]
pub struct CommandInfo {
    pub id: String,
    pub label: String,
    /// Default chord, "" = none.
    pub key: String,
    pub fixed: bool,
}

/// An override that cannot stand in a set, with why (in the registry's words).
#[derive(Debug, Clone, PartialEq)]
pub struct Rejected {
    pub command_id: String,
    pub chord: String,
    pub reason: String,
}

#[derive(Debug, Clone, Default)]
pub struct Resolution {
    /// Every command's effective chord, in registry order.
    pub bindings: Vec<(String, String)>,
    pub rejected: Vec<Rejected>,
}

/// What the core needs from a command registry (`CommandsAdapter`).
pub trait CommandsAdapter: Send + Sync {
    fn is_command(&self, id: &str) -> bool;
    /// "" = none; only asked for ids where `is_command` is true.
    fn default_chord(&self, id: &str) -> String;
    /// Judges a whole override set (`overrides` in insertion order; values as stored, which may be any JSON).
    fn resolve(&self, overrides: &[(String, Value)]) -> Resolution;
}

enum Check {
    Ok,
    Same,
    Fixed,
    Reserved(String),
    Bare,
    Taken(String),
    Invalid,
}

/// A registry read from the JSON form of a TS registry's `defs` (frame commands already merged in).
#[derive(Debug, Clone)]
pub struct Registry {
    pub defs: Vec<CommandInfo>,
    reserved: HashMap<String, String>,
}

fn describe_raw(v: &Value) -> String {
    match v {
        Value::String(s) => crate::value::bound(s, 80),
        other => crate::value::bound(&other.to_string(), 80),
    }
}

impl Registry {
    pub fn from_json(defs: &Value) -> Result<Registry, String> {
        let arr = defs.as_array().ok_or("commands must be an array")?;
        let mut out = Vec::new();
        for d in arr {
            let o = d.as_object().ok_or("command must be an object")?;
            let s = |k: &str| o.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            out.push(CommandInfo { id: s("id"), label: s("label"), key: s("key"), fixed: o.get("fixed").and_then(Value::as_bool) == Some(true) });
        }
        Ok(Self::new(out))
    }

    pub fn new(defs: Vec<CommandInfo>) -> Registry {
        let mut reserved: HashMap<String, String> = HashMap::new();
        for (c, l) in RESERVED_SYSTEM.iter().chain(RESERVED_ESTATE.iter()) {
            reserved.insert(c.to_string(), l.to_string());
        }
        for d in &defs {
            if d.fixed && !d.key.is_empty() {
                reserved.insert(canon_chord(&d.key), d.label.clone());
            }
        }
        Registry { defs, reserved }
    }

    pub fn by_id(&self, id: &str) -> Option<&CommandInfo> {
        self.defs.iter().find(|d| d.id == id)
    }

    fn command_for_chord(&self, bindings: &HashMap<String, String>, chord: &str) -> Option<String> {
        let want = canon_chord(chord);
        if want.is_empty() {
            return None;
        }
        self.defs
            .iter()
            .find(|d| bindings.get(&d.id).is_some_and(|h| !h.is_empty() && canon_chord(h) == want))
            .map(|d| d.id.clone())
    }

    fn check_rebind(&self, bindings: &HashMap<String, String>, id: &str, chord: &str) -> Check {
        let Some(def) = self.by_id(id) else { return Check::Fixed };
        if def.fixed {
            return Check::Fixed;
        }
        let want = canon_chord(chord);
        let current = bindings.get(id).cloned().unwrap_or_default();
        if want.is_empty() {
            return if current.is_empty() { Check::Same } else { Check::Ok };
        }
        if !is_valid_chord(&want) {
            return Check::Invalid;
        }
        if canon_chord(&current) == want {
            return Check::Same;
        }
        if !is_typing_safe(&want) {
            return Check::Bare;
        }
        if let Some(by) = self.reserved.get(&want) {
            return Check::Reserved(by.clone());
        }
        if let Some(other) = self.command_for_chord(bindings, &want) {
            if other != id {
                return Check::Taken(other);
            }
        }
        Check::Ok
    }

    fn rebind_message(&self, c: &Check, chord: &str) -> String {
        match c {
            Check::Fixed => "This shortcut is the same in all the apps, so it cannot be changed".into(),
            Check::Invalid => "That is not a key combination this app can use".into(),
            Check::Reserved(by) => format!("{chord} is kept for “{by}”, so it cannot be used here"),
            Check::Bare => format!("{chord} alone is for typing; add ⌘, ⌥ or ⌃"),
            Check::Taken(by) => format!("{chord} is already used by “{}”", self.by_id(by).map(|d| d.label.clone()).unwrap_or(by.clone())),
            _ => String::new(),
        }
    }

    /// `resolveBindings`, with the reasons worded as `commandsAdapter` words them.
    fn resolve_bindings(&self, overrides: &[(String, Value)]) -> Resolution {
        let has = |id: &str| overrides.iter().any(|(k, _)| k == id);
        let get = |id: &str| overrides.iter().find(|(k, _)| k == id).map(|(_, v)| v.clone());
        // reason codes first, worded at the end against the resolved set
        let mut raw: Vec<(String, String, &'static str)> = Vec::new();
        for (id, v) in overrides {
            match self.by_id(id) {
                None => raw.push((id.clone(), describe_raw(v), "unknown")),
                Some(d) if d.fixed => raw.push((id.clone(), describe_raw(v), "fixed")),
                _ => {}
            }
        }
        let mut out: HashMap<String, String> = HashMap::new();
        for d in &self.defs {
            out.insert(d.id.clone(), if !d.fixed && has(&d.id) { String::new() } else { d.key.clone() });
        }
        let mut refused: Vec<&CommandInfo> = Vec::new();
        for d in &self.defs {
            if d.fixed || !has(&d.id) {
                continue;
            }
            let rawv = get(&d.id).unwrap_or(Value::Null);
            if rawv.as_str() == Some("") {
                continue;
            }
            let chord = rawv.as_str().map(canon_chord).unwrap_or_default();
            if rawv.as_str().is_none() || chord.is_empty() {
                raw.push((d.id.clone(), describe_raw(&rawv), "invalid"));
                refused.push(d);
                continue;
            }
            match self.check_rebind(&out, &d.id, &chord) {
                Check::Ok | Check::Same => {
                    out.insert(d.id.clone(), chord);
                }
                c => {
                    let code = match c {
                        Check::Fixed => "fixed",
                        Check::Reserved(_) => "reserved",
                        Check::Bare => "bare",
                        Check::Taken(_) => "taken",
                        _ => "invalid",
                    };
                    raw.push((d.id.clone(), rawv.as_str().unwrap_or("").to_string(), code));
                    refused.push(d);
                }
            }
        }
        for d in refused {
            if !d.key.is_empty() && self.command_for_chord(&out, &d.key).is_none() {
                out.insert(d.id.clone(), d.key.clone());
            }
        }
        let rejected = raw
            .into_iter()
            .map(|(id, chord, code)| {
                let reason = if code == "unknown" {
                    format!("There is no command \"{id}\"")
                } else {
                    let asked = if code != "fixed" && code != "invalid" {
                        let m = self.rebind_message(&self.check_rebind(&out, &id, &chord), &chord);
                        (!m.is_empty()).then_some(m)
                    } else {
                        None
                    };
                    asked.unwrap_or_else(|| {
                        match code {
                            "fixed" => "This shortcut is the same in all the apps, so it cannot be changed",
                            "invalid" => "That is not a key combination this app can use",
                            "reserved" => "That shortcut is kept for the system or all the apps",
                            "bare" => "A key alone is for typing; add ⌘, ⌥ or ⌃",
                            "taken" => "That shortcut is already used by another command",
                            _ => "Conflicts with another shortcut",
                        }
                        .to_string()
                    })
                };
                Rejected { command_id: id, chord, reason }
            })
            .collect();
        let bindings = self.defs.iter().map(|d| (d.id.clone(), out.get(&d.id).cloned().unwrap_or_default())).collect();
        Resolution { bindings, rejected }
    }
}

impl CommandsAdapter for Registry {
    fn is_command(&self, id: &str) -> bool {
        self.by_id(id).is_some()
    }
    fn default_chord(&self, id: &str) -> String {
        self.by_id(id).map(|d| d.key.clone()).unwrap_or_default()
    }
    fn resolve(&self, overrides: &[(String, Value)]) -> Resolution {
        self.resolve_bindings(overrides)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn reg() -> Registry {
        Registry::from_json(&json!([
            { "id": "open", "label": "Open", "key": "⌘O" },
            { "id": "pin", "label": "Pin or unpin", "key": "⌘D" },
            { "id": "next", "label": "Next", "key": "↓" },
            { "id": "settings", "label": "Settings…", "key": "⌘,", "fixed": true },
        ]))
        .unwrap()
    }

    #[test]
    fn chords() {
        assert!(is_valid_chord("⌘⇧K"));
        assert!(!is_valid_chord("⇧⌘K"));
        assert!(is_valid_chord("↓"));
        assert!(!is_valid_chord("⌘"));
        assert_eq!(canon_chord("⇧⌘k"), "⌘⇧K");
        assert!(!is_typing_safe("⇧A"));
        assert!(is_typing_safe("F5"));
        assert_eq!(to_accelerator("⌘⌥⇧H").as_deref(), Some("Cmd+Alt+Shift+H"));
        assert_eq!(to_accelerator("↓"), None);
    }

    #[test]
    fn swap_resolves_and_collisions_are_named() {
        let r = reg();
        let swap = r.resolve(&[("open".into(), json!("⌘D")), ("pin".into(), json!("⌘O"))]);
        assert!(swap.rejected.is_empty(), "{:?}", swap.rejected);
        let taken = r.resolve(&[("open".into(), json!("⌘D"))]);
        assert_eq!(taken.rejected[0].reason, "⌘D is already used by “Pin or unpin”");
        let reserved = r.resolve(&[("open".into(), json!("⌘,"))]);
        assert_eq!(reserved.rejected[0].reason, "⌘, is kept for “Settings…”, so it cannot be used here");
        let fixed = r.resolve(&[("settings".into(), json!("⌘J"))]);
        assert_eq!(fixed.rejected[0].reason, "This shortcut is the same in all the apps, so it cannot be changed");
        let bare = r.resolve(&[("open".into(), json!("A"))]);
        assert_eq!(bare.rejected[0].reason, "A alone is for typing; add ⌘, ⌥ or ⌃");
    }
}
