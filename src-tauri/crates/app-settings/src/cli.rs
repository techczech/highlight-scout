//! The agent CLI (port of node/cli.ts): `list | get | set | reset | changes | export | import | undo-import`, `--json`, `--agent`.
//! Writes through the store, so every change is locked, validated and logged `who: agent`. Exit codes: 0 done, 1 the file cannot be used
//! or saved, 2 bad input, 3 needs confirmation, 4 locked.

use crate::changes::{limits, Actor, ChangeEntry, SkippedItem, Who};
use crate::file::{read_bounded, Bounded, MAX_FILE_BYTES};
use crate::load::MigrateFn;
use crate::schema::{format_value, Schema, SettingType};
use crate::state::{changed_from_default, last_import_batch, value_of, Clock, OpError, SystemClock, Target};
use crate::store::{SettingsStore, StoreOptions};
use crate::transfer::ImportPreview;
use crate::value::{bound, has_control, is_control, iso, jlen, js_string, pretty, Obj};
use crate::commands::CommandsAdapter;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

pub const EXIT_OK: i32 = 0;
pub const EXIT_FAILED: i32 = 1;
pub const EXIT_BAD_INPUT: i32 = 2;
pub const EXIT_CONFIRM: i32 = 3;
pub const EXIT_LOCKED: i32 = 4;

const USAGE: &str = "Usage:\n  list [--section <id>]\n  get <key>\n  set <key> <value>\n  reset <key> | --section <id> | --all --yes\n  changes [--limit <n>]\n  export <file>\n  import <file> [--yes]\n  undo-import --last | <batch>\nOptions for every command: --json, --agent <name> (or BRIDLE_AGENT)";

pub struct CliOptions {
    pub dir: PathBuf,
    pub schema: Arc<Schema>,
    pub commands: Arc<dyn CommandsAdapter>,
    pub version: String,
    pub app_name: String,
    pub migrate: Option<MigrateFn>,
    pub root: Option<PathBuf>,
}

pub struct CliIo<'a> {
    pub stdout: &'a mut dyn FnMut(&str),
    pub stderr: &'a mut dyn FnMut(&str),
    pub env: HashMap<String, String>,
    pub cwd: PathBuf,
    pub host_name: String,
    pub clock: Option<Arc<dyn Clock>>,
}

struct Exit {
    code: i32,
    message: String,
    json: Obj,
}

fn exit(code: i32, message: impl Into<String>) -> Exit {
    Exit { code, message: message.into(), json: Obj::new() }
}

fn exit_with(code: i32, message: impl Into<String>, extra: Value) -> Exit {
    Exit { code, message: message.into(), json: extra.as_object().cloned().unwrap_or_default() }
}

fn esc_char(c: char) -> String {
    format!("\\u{:04x}", c as u32)
}

/// Control characters of text from a file as `\uXXXX`.
fn esc(s: &str) -> String {
    s.chars().map(|c| if is_control(c) { esc_char(c) } else { c.to_string() }).collect()
}

/// As `esc`, keeping newlines.
fn safe(s: &str) -> String {
    s.chars().map(|c| if c != '\n' && is_control(c) { esc_char(c) } else { c.to_string() }).collect()
}

struct Parsed {
    verb: String,
    args: Vec<String>,
    flags: HashMap<String, Option<String>>,
}

fn parse(argv: &[String]) -> (bool, Result<Parsed, Exit>) {
    let verbs: HashMap<&str, ((usize, usize), Vec<&str>)> = [
        ("list", ((0, 0), vec!["section"])),
        ("get", ((1, 1), vec![])),
        ("set", ((2, 2), vec![])),
        ("reset", ((0, 1), vec!["section", "all", "yes"])),
        ("changes", ((0, 0), vec!["limit"])),
        ("export", ((1, 1), vec![])),
        ("import", ((1, 1), vec!["yes"])),
        ("undo-import", ((0, 1), vec!["last"])),
    ]
    .into_iter()
    .collect();
    let mut args = Vec::new();
    let mut flags: HashMap<String, Option<String>> = HashMap::new();
    let mut rest = false;
    let mut error: Option<Exit> = None;
    let mut i = 0;
    while i < argv.len() {
        let a = &argv[i];
        if rest || !a.starts_with("--") {
            args.push(a.clone());
            i += 1;
            continue;
        }
        if a == "--" {
            rest = true;
            i += 1;
            continue;
        }
        let (name, val) = match a[2..].split_once('=') {
            Some((n, v)) => (n.to_string(), Some(v.to_string())),
            None => (a[2..].to_string(), None),
        };
        if ["section", "limit", "agent"].contains(&name.as_str()) {
            let v = val.or_else(|| {
                i += 1;
                argv.get(i).cloned()
            });
            match v {
                Some(v) => {
                    flags.insert(name, Some(v));
                }
                None => {
                    error.get_or_insert(exit(EXIT_BAD_INPUT, format!("--{name} needs a value")));
                }
            }
        } else if ["all", "yes", "last", "json"].contains(&name.as_str()) && val.is_none() {
            flags.insert(name, None);
        } else {
            let ok = a.len() <= 22 && a[2..].chars().all(|c| c.is_ascii_lowercase() || c == '-') && a.len() > 2;
            error.get_or_insert(exit(EXIT_BAD_INPUT, if ok { format!("Unknown option {a}\n{USAGE}") } else { format!("Unknown option\n{USAGE}") }));
        }
        i += 1;
    }
    let json = flags.contains_key("json");
    if let Some(e) = error {
        return (json, Err(e));
    }
    if args.is_empty() {
        return (json, Err(exit(EXIT_BAD_INPUT, USAGE)));
    }
    let verb = args.remove(0);
    let Some((range, allowed)) = verbs.get(verb.as_str()) else {
        let ok = !verb.is_empty() && jlen(&verb) <= 20 && verb.chars().all(|c| c.is_ascii_lowercase() || c == '-');
        return (json, Err(exit(EXIT_BAD_INPUT, format!("{}\n{USAGE}", if ok { format!("Unknown command \"{verb}\"") } else { "Unknown command".into() }))));
    };
    if args.len() < range.0 || args.len() > range.1 {
        return (json, Err(exit(EXIT_BAD_INPUT, USAGE)));
    }
    for f in flags.keys() {
        if f != "json" && f != "agent" && !allowed.contains(&f.as_str()) {
            return (json, Err(exit(EXIT_BAD_INPUT, format!("--{f} does not go with {verb}\n{USAGE}"))));
        }
    }
    (json, Ok(Parsed { verb, args, flags }))
}

fn agent_name(flag: Option<&String>, env: &HashMap<String, String>) -> Result<String, Exit> {
    let raw = flag.cloned().or_else(|| env.get("BRIDLE_AGENT").cloned()).unwrap_or_default();
    let name = raw.trim();
    if name.is_empty() {
        return Ok("agent".into());
    }
    if has_control(name) {
        return Err(exit(EXIT_BAD_INPUT, "The agent name cannot hold control characters"));
    }
    if jlen(name) > limits::NAME {
        return Err(exit(EXIT_BAD_INPUT, format!("The agent name can be at most {} characters", limits::NAME)));
    }
    Ok(name.into())
}

fn exit_for(code: &str) -> i32 {
    match code {
        "locked" => EXIT_LOCKED,
        "unwritten" | "closed" | "too-large" | "refused" => EXIT_FAILED,
        _ => EXIT_BAD_INPUT,
    }
}

fn row(store: &SettingsStore, schema: &Schema, key: &str) -> Value {
    let d = schema.def(key).expect("row of a known key");
    if d.kind == SettingType::Secret {
        return json!({ "key": d.key, "label": d.label, "section": d.section, "type": "secret", "value": null, "default": null, "changed": false, "secret": true });
    }
    let st = store.view();
    json!({
        "key": d.key, "label": d.label, "section": d.section, "type": d.kind.as_str(),
        "value": value_of(&st, schema, key), "default": d.default, "changed": st.settings.contains_key(key),
    })
}

fn who_text(w: &Who) -> String {
    match w {
        Who::You => "you".into(),
        Who::Agent(n) => format!("agent {}", esc(n)),
        Who::AppUpdate(v) => format!("app update {}", esc(v)),
        Who::Import(i) => format!("import of {}{}", esc(&i.file), match &i.by { Some(Actor::Agent(n)) => format!(" by agent {}", esc(n)), _ => String::new() }),
        Who::Reset { by, .. } => format!("reset{}", match by { Some(Actor::Agent(n)) => format!(" by agent {}", esc(n)), _ => String::new() }),
    }
}

fn entry_line(e: &ChangeEntry, schema: &Schema) -> String {
    let def = e.key().and_then(|k| schema.def(k));
    let what = esc(&e.key().map(str::to_string).unwrap_or_else(|| format!("shortcut {}", e.command_id().unwrap_or(""))));
    let val = |v: &Value| {
        esc(&if e.command_id().is_some() {
            match v {
                Value::Null => "—".into(),
                Value::String(s) if s.is_empty() => "None".into(),
                other => js_string(other),
            }
        } else {
            format_value(def, v)
        })
    };
    let note = e.note.as_ref().map(|n| format!(" ({})", esc(n))).unwrap_or_default();
    let at = if e.at.abs() <= 8.64e15 { iso(e.at) } else { "?".into() };
    let batch = e.batch.as_ref().map(|b| format!("  batch {}", esc(b))).unwrap_or_default();
    format!("{at}  {what}  {} → {}  by {}{batch}{note}", val(&e.from), val(&e.to), who_text(&e.who))
}

fn preview_text(p: &ImportPreview, schema: &Schema) -> String {
    let mut lines = Vec::new();
    if let Some(s) = &p.source {
        let parts: Vec<String> = [s.exported_from.clone(), s.app_version.as_ref().map(|v| format!("version {v}")), s.exported_at.clone()]
            .into_iter()
            .flatten()
            .filter(|x| !x.is_empty())
            .map(|x| esc(&x))
            .collect();
        lines.push(format!("From {}", if parts.is_empty() { "an export file".into() } else { parts.join(", ") }));
    }
    lines.push(format!("{} to change, {} skipped, {} already match", p.changes.len(), p.skipped.len(), p.already_match));
    for c in &p.changes {
        match &c.subject {
            crate::changes::Subject::Key(k) => {
                let def = schema.def(k);
                lines.push(format!("  {}: {} → {}", esc(k), esc(&format_value(def, &c.from)), esc(&format_value(def, &c.to))));
            }
            crate::changes::Subject::Command(id) => {
                let none = |v: &Value| match v.as_str() {
                    Some("") | None => "None".to_string(),
                    Some(s) => s.to_string(),
                };
                lines.push(format!("  shortcut {}: {} → {}", esc(id), esc(&none(&c.from)), esc(&none(&c.to))));
            }
        }
    }
    for s in &p.skipped {
        let what = s.key.clone().unwrap_or_else(|| format!("shortcut {}", s.command_id.clone().unwrap_or_default()));
        lines.push(format!("  skipped {}: {}", esc(&what), esc(&s.reason)));
    }
    lines.join("\n")
}

fn entries_json(es: &[ChangeEntry]) -> Value {
    Value::Array(es.iter().map(ChangeEntry::to_json).collect())
}

fn skipped_json(s: &[SkippedItem]) -> Value {
    Value::Array(s.iter().map(SkippedItem::to_json).collect())
}

/// Runs one CLI command; returns the exit code. Never panics on input.
pub fn run_settings_cli(argv: &[String], opts: &CliOptions, io: &mut CliIo) -> i32 {
    let (json, parsed) = parse(argv);
    let mut stdout_buf: Vec<String> = Vec::new();
    let mut stderr_buf: Vec<String> = Vec::new();
    let code = {
        let mut out = |text: &str, data: Value| {
            stdout_buf.push(if json { format!("{}\n", crate::value::compact(&data)) } else { format!("{}\n", safe(text)) });
        };
        let errlog: Arc<std::sync::Mutex<Vec<String>>> = Arc::new(std::sync::Mutex::new(Vec::new()));
        let result = run(json, parsed, opts, io, &mut out, errlog.clone());
        stderr_buf.extend(errlog.lock().map(|g| g.clone()).unwrap_or_default());
        match result {
            Ok(c) => c,
            Err(e) => {
                if json {
                    let mut o = Obj::new();
                    o.insert("ok".into(), json!(false));
                    o.insert("exit".into(), json!(e.code));
                    o.insert("error".into(), json!(e.message));
                    for (k, v) in e.json {
                        o.insert(k, v);
                    }
                    stdout_buf.push(format!("{}\n", crate::value::compact(&Value::Object(o))));
                } else if e.code == EXIT_CONFIRM {
                    stdout_buf.push(format!("{}\n", safe(&e.message)));
                } else {
                    stderr_buf.push(format!("{}\n", safe(&e.message)));
                }
                e.code
            }
        }
    };
    for l in stderr_buf {
        (io.stderr)(&l);
    }
    for l in stdout_buf {
        (io.stdout)(&l);
    }
    code
}

fn store_fail(e: OpError) -> Exit {
    let mut x = json!({ "code": e.code });
    if let Some(s) = &e.skipped {
        x["skipped"] = skipped_json(s);
    }
    exit_with(exit_for(e.code), e.error, x)
}

fn run(json: bool, parsed: Result<Parsed, Exit>, opts: &CliOptions, io: &CliIo, out: &mut dyn FnMut(&str, Value), errlog: Arc<std::sync::Mutex<Vec<String>>>) -> Result<i32, Exit> {
    let p = parsed?;
    let name = agent_name(p.flags.get("agent").and_then(|x| x.as_ref()), &io.env)?;
    let schema = opts.schema.clone();
    let mut so = StoreOptions::new(opts.dir.clone(), schema.clone(), opts.commands.clone(), &opts.version);
    so.who = Actor::Agent(name);
    so.migrate = opts.migrate.clone();
    so.root = opts.root.clone();
    if let Some(c) = &io.clock {
        so.clock = c.clone();
    } else {
        so.clock = Arc::new(SystemClock);
    }
    so.log = Arc::new(move |l| {
        if let Ok(mut g) = errlog.lock() {
            g.push(format!("{}\n", esc(l)));
        }
    });
    let s = SettingsStore::open(so);
    if let Some(pr) = s.problem() {
        return Err(exit_with(EXIT_FAILED, pr.error, json!({ "code": "refused", "problem": pr.code })));
    }
    let path_of = |f: &str| {
        let p = PathBuf::from(f);
        if p.is_absolute() {
            p
        } else {
            io.cwd.join(p)
        }
    };
    let need_def = |key: &str| schema.def(key).cloned().ok_or_else(|| exit(EXIT_BAD_INPUT, format!("There is no setting \"{}\". Run \"list\" to see them.", bound(key, limits::ID))));
    let changes_out = |out: &mut dyn FnMut(&str, Value), entries: &[ChangeEntry], extra: Option<String>| {
        let text = if entries.is_empty() { "Nothing changed; it already had that value".into() } else { entries.iter().map(|e| entry_line(e, &schema)).collect::<Vec<_>>().join("\n") };
        let mut data = json!({ "ok": true, "changed": !entries.is_empty(), "entries": entries_json(entries) });
        if let Some(b) = extra {
            data["batch"] = json!(b);
        }
        out(&text, data);
    };
    let flag = |k: &str| p.flags.get(k).cloned().flatten();
    let has = |k: &str| p.flags.contains_key(k);
    match p.verb.as_str() {
        "list" => {
            let section = flag("section");
            if let Some(sec) = &section {
                if schema.section(sec).is_none() {
                    let ids: Vec<&str> = schema.sections.iter().map(|x| x.id.as_str()).collect();
                    return Err(exit(EXIT_BAD_INPUT, format!("There is no section \"{}\". Sections: {}", bound(sec, limits::ID), ids.join(", "))));
                }
            }
            let rows: Vec<Value> = schema.settings.iter().filter(|d| section.as_ref().is_none_or(|x| d.section == *x)).map(|d| row(&s, &schema, &d.key)).collect();
            let text: Vec<String> = rows
                .iter()
                .map(|r| {
                    let key = r["key"].as_str().unwrap_or("");
                    if r["type"] == "secret" {
                        format!("{key} = (a key, kept in Keychain; see Accounts and keys)")
                    } else {
                        let d = schema.def(key);
                        let dflt = if r["changed"] == true { format!("  (default {})", esc(&format_value(d, &r["default"]))) } else { String::new() };
                        format!("{key} = {}{dflt}", esc(&format_value(d, &r["value"])))
                    }
                })
                .collect();
            out(&text.join("\n"), json!({ "settings": rows }));
            Ok(EXIT_OK)
        }
        "get" => {
            let d = need_def(&p.args[0])?;
            let mut r = row(&s, &schema, &d.key);
            if let Some(last) = s.view().changes.iter().find(|e| e.key() == Some(d.key.as_str())) {
                r["lastChange"] = last.to_json();
            }
            let text = if d.kind == SettingType::Secret { "(a key, kept in Keychain; see Accounts and keys)".into() } else { esc(&js_string(&r["value"])) };
            out(&text, r);
            Ok(EXIT_OK)
        }
        "set" => {
            let d = need_def(&p.args[0])?;
            if d.kind == SettingType::Secret {
                return Err(exit_with(
                    EXIT_BAD_INPUT,
                    format!("\"{}\" is a key. Keys are kept in Keychain, never in the settings file: set it in {} › Settings › Accounts and keys.", d.label, opts.app_name),
                    json!({ "code": "secret" }),
                ));
            }
            let v = schema.parse_value(&d, &p.args[1]).map_err(|e| exit_with(EXIT_BAD_INPUT, format!("{}: {e}", d.key), json!({ "code": "invalid-value" })))?;
            let r = s.set(&d.key, &v).map_err(store_fail)?;
            changes_out(out, &r.entries, None);
            Ok(EXIT_OK)
        }
        "reset" => {
            let key = p.args.first().cloned();
            let section = flag("section");
            let all = has("all");
            if [key.is_some(), section.is_some(), all].iter().filter(|x| **x).count() != 1 {
                return Err(exit(EXIT_BAD_INPUT, USAGE));
            }
            if let Some(key) = key {
                let d = need_def(&key)?;
                if d.kind == SettingType::Secret {
                    return Err(exit_with(EXIT_BAD_INPUT, format!("\"{}\" is a key: remove it in {} › Settings › Accounts and keys.", d.label, opts.app_name), json!({ "code": "secret" })));
                }
                let r = s.reset(&Target::Key(d.key.clone())).map_err(store_fail)?;
                changes_out(out, &r.entries, None);
                return Ok(EXIT_OK);
            }
            if let Some(sec) = section {
                if schema.section(&sec).is_none() {
                    return Err(exit(EXIT_BAD_INPUT, format!("There is no section \"{}\"", bound(&sec, limits::ID))));
                }
                let r = s.reset_section(&sec).map_err(store_fail)?;
                changes_out(out, &r.entries, r.batch);
                return Ok(EXIT_OK);
            }
            let count = changed_from_default(&s.view(), &schema, None).count;
            if !has("yes") {
                if count == 0 {
                    out("Nothing to reset: every setting and shortcut has its default", json!({ "ok": true, "changed": false, "entries": [], "count": 0 }));
                    return Ok(EXIT_OK);
                }
                return Err(exit_with(
                    EXIT_CONFIRM,
                    format!("This puts {count} setting{} and shortcuts back to their defaults. Run again with --yes to do it.", if count == 1 { "" } else { "s" }),
                    json!({ "needsConfirmation": true, "count": count }),
                ));
            }
            let r = s.reset_all().map_err(store_fail)?;
            changes_out(out, &r.entries, r.batch);
            Ok(EXIT_OK)
        }
        "changes" => {
            let limit = match flag("limit") {
                None => 20,
                Some(x) => x.trim().parse::<usize>().ok().filter(|n| *n >= 1).ok_or_else(|| exit(EXIT_BAD_INPUT, "--limit must be a whole number of at least 1"))?,
            };
            let all = s.view().changes;
            let entries: Vec<ChangeEntry> = all.iter().take(limit).cloned().collect();
            let text = if entries.is_empty() { "No changes yet".into() } else { entries.iter().map(|e| entry_line(e, &schema)).collect::<Vec<_>>().join("\n") };
            out(&text, json!({ "changes": entries_json(&entries), "total": all.len() }));
            Ok(EXIT_OK)
        }
        "export" => {
            let path = path_of(&p.args[0]);
            let file = s.export_file(&io.host_name);
            write_new(&path, &pretty(&file)).map_err(|already| {
                if already {
                    exit(EXIT_BAD_INPUT, format!("{} already exists; pick a new file name", path.display()))
                } else {
                    exit(EXIT_FAILED, format!("Could not write {}", path.display()))
                }
            })?;
            let n = file["settings"].as_object().map(|o| o.len()).unwrap_or(0);
            let k = file["keys"].as_object().map(|o| o.len()).unwrap_or(0);
            out(
                &format!("Exported {n} setting{} and {k} shortcut{} to {}", if n == 1 { "" } else { "s" }, if k == 1 { "" } else { "s" }, path.display()),
                json!({ "ok": true, "file": path.display().to_string(), "settings": n, "keys": k }),
            );
            Ok(EXIT_OK)
        }
        "import" => {
            let path = path_of(&p.args[0]);
            let text = match read_bounded(&path, MAX_FILE_BYTES) {
                Bounded::Ok(t) => t,
                Bounded::Missing => return Err(exit(EXIT_BAD_INPUT, format!("There is no file {}", path.display()))),
                Bounded::TooLarge => return Err(exit(EXIT_BAD_INPUT, format!("{} is too large to be a settings export", path.display()))),
                _ => return Err(exit(EXIT_BAD_INPUT, format!("{} cannot be read", path.display()))),
            };
            let preview = s.preview_import(&text);
            if let Some(r) = &preview.refused {
                let pj = preview.to_json();
                return Err(exit_with(EXIT_BAD_INPUT, r.message.clone(), json!({ "code": "refused", "refused": pj["refused"] })));
            }
            if preview.changes.is_empty() {
                out(&format!("{}\nNothing to import.", preview_text(&preview, &schema)), json!({ "ok": true, "applied": false, "preview": preview.to_json() }));
                return Ok(EXIT_OK);
            }
            if !has("yes") {
                if json {
                    out("", json!({ "ok": false, "exit": EXIT_CONFIRM, "needsConfirmation": true, "preview": preview.to_json() }));
                } else {
                    out(&format!("{}\nRun again with --yes to import.", preview_text(&preview, &schema)), Value::Null);
                }
                return Ok(EXIT_CONFIRM);
            }
            let file_name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let done = s.apply_import(&preview, &file_name).map_err(store_fail)?;
            let n = done.entries.len();
            let batch_text = done.batch.as_ref().map(|b| format!(" (batch {}; undo with \"undo-import --last\")", esc(b))).unwrap_or_default();
            let mut data = json!({ "ok": true, "applied": true, "entries": entries_json(&done.entries), "skipped": skipped_json(&preview.skipped) });
            if let Some(b) = &done.batch {
                let mut o = Obj::new();
                for (k, v) in data.as_object().cloned().unwrap_or_default() {
                    o.insert(k.clone(), v);
                    if k == "applied" {
                        o.insert("batch".into(), json!(b));
                    }
                }
                data = Value::Object(o);
            }
            out(&format!("Imported: {n} change{}{batch_text}", if n == 1 { "" } else { "s" }), data);
            Ok(EXIT_OK)
        }
        "undo-import" => {
            let last = has("last");
            let id = p.args.first().cloned();
            if last == id.is_some() {
                return Err(exit(EXIT_BAD_INPUT, USAGE));
            }
            let st = s.view();
            let batch = if last { last_import_batch(&st) } else { id };
            let Some(batch) = batch else { return Err(exit_with(EXIT_BAD_INPUT, "There is no import to undo", json!({ "code": "not-found" }))) };
            let Some(first) = st.changes.iter().find(|e| e.batch.as_deref() == Some(batch.as_str())) else {
                return Err(exit_with(EXIT_BAD_INPUT, "That batch is no longer in the list of changes", json!({ "code": "not-found" })));
            };
            if !matches!(first.who, Who::Import(_)) {
                return Err(exit_with(EXIT_BAD_INPUT, "That batch is not an import", json!({ "code": "not-found" })));
            }
            let r = s.undo_batch(&batch).map_err(store_fail)?;
            let skipped_text: Vec<String> = r
                .skipped
                .iter()
                .map(|x| format!("  kept {}: {}", esc(&x.key.clone().unwrap_or_else(|| format!("shortcut {}", x.command_id.clone().unwrap_or_default()))), esc(&x.reason)))
                .collect();
            let mut text = r.entries.iter().map(|e| entry_line(e, &schema)).collect::<Vec<_>>().join("\n");
            if !skipped_text.is_empty() {
                text = format!("{text}\n{}", skipped_text.join("\n"));
            }
            let mut data = Obj::new();
            data.insert("ok".into(), json!(true));
            if let Some(b) = &r.batch {
                data.insert("batch".into(), json!(b));
            }
            data.insert("entries".into(), entries_json(&r.entries));
            data.insert("skipped".into(), skipped_json(&r.skipped));
            out(&text, Value::Object(data));
            Ok(EXIT_OK)
        }
        _ => Err(exit(EXIT_BAD_INPUT, USAGE)),
    }
}

/// Writes a new file (never over an existing one), mode 0644. Err(true) when it already exists.
fn write_new(path: &std::path::Path, text: &str) -> Result<(), bool> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o644).open(path).map_err(|e| e.kind() == std::io::ErrorKind::AlreadyExists)?;
    f.write_all(text.as_bytes()).map_err(|_| false)
}

/// Runs the CLI against the real process (stdout, stderr, environment, working directory, host name).
pub fn run_process(argv: &[String], opts: &CliOptions) -> i32 {
    use std::io::Write;
    let mut so = |t: &str| {
        let _ = std::io::stdout().write_all(t.as_bytes());
    };
    let mut se = |t: &str| {
        let _ = std::io::stderr().write_all(t.as_bytes());
    };
    let mut io = CliIo {
        stdout: &mut so,
        stderr: &mut se,
        env: std::env::vars().collect(),
        cwd: std::env::current_dir().unwrap_or_default(),
        host_name: crate::lock::host_name(),
        clock: None,
    };
    run_settings_cli(argv, opts, &mut io)
}
