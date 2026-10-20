//! Byte compatibility with the TS package: replays the script `fixtures/gen-ts-fixture.mjs` ran through the real `@bridle/settings`
//! and `@bridle/commands`, and requires the same settings-file text after every step, the same export text, the same import preview
//! and the same loader output. Also reads every TS-written file back unchanged.

use app_settings::changes::Actor;
use app_settings::load::{load_state, LoadCtx, Migrated, MigrateFn};
use app_settings::state::{self, Clock, Ctx, SettingsState, Target};
use app_settings::transfer::{apply_import, export_file, preview_import};
use app_settings::value::pretty;
use app_settings::{Registry, Schema};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

struct Fixed {
    t: AtomicU64,
    n: AtomicU64,
}
impl Clock for Fixed {
    fn now(&self) -> f64 {
        self.t.fetch_add(1, Ordering::SeqCst) as f64
    }
    fn new_id(&self) -> String {
        format!("id{}", self.n.fetch_add(1, Ordering::SeqCst) + 1)
    }
}

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/ts-script.json")).unwrap()
}

fn who(v: &Value) -> Actor {
    if v == "agent" {
        Actor::Agent("Claude Code".into())
    } else {
        Actor::You
    }
}

#[test]
fn replaying_the_ts_script_writes_the_same_bytes() {
    let f = fixture();
    let schema = Schema::from_json(&f["schema"]).unwrap();
    let reg = Registry::from_json(&f["commands"]).unwrap();
    let clock = Fixed { t: AtomicU64::new(f["start"]["t"].as_u64().unwrap()), n: AtomicU64::new(0) };
    let mut st = SettingsState::empty("writeflex");
    for (i, step) in f["steps"].as_array().unwrap().iter().enumerate() {
        let a = &step["args"];
        let ctx = Ctx { schema: &schema, commands: &reg, clock: &clock, who: who(&a["who"]) };
        let r = match step["op"].as_str().unwrap() {
            "set" => state::set(&st, a["key"].as_str().unwrap(), &a["value"], &ctx),
            "rebind" => state::rebind(&st, a["id"].as_str().unwrap(), a["chord"].as_str().unwrap(), &ctx),
            "rebindMany" => {
                let pairs: Vec<(String, Value)> = a["pairs"].as_array().unwrap().iter().map(|p| (p[0].as_str().unwrap().to_string(), p[1].clone())).collect();
                state::rebind_many(&st, &pairs, &ctx)
            }
            "recordSecret" => state::record_secret(&st, a["key"].as_str().unwrap(), a["from"].as_str().unwrap(), a["to"].as_str().unwrap(), &ctx),
            "reset" => state::reset(&st, &Target::Key(a["key"].as_str().unwrap().into()), &ctx),
            "resetEntry" => state::reset_entry(&st, a["id"].as_str().unwrap(), &ctx),
            "applyImport" => {
                let p = preview_import(&st, a["text"].as_str().unwrap(), &schema, &reg);
                assert_eq!(p.to_json(), a["preview"], "preview");
                apply_import(&st, &p, a["file"].as_str().unwrap(), &ctx)
            }
            "undoBatch" => state::undo_batch(&st, a["batch"].as_str().unwrap(), &ctx),
            "resetSection" => state::reset_section(&st, a["section"].as_str().unwrap(), &ctx),
            "resetAll" => state::reset_all(&st, &ctx),
            other => panic!("unknown op {other}"),
        };
        assert_eq!(r.is_ok(), step["ok"].as_bool().unwrap(), "step {i} {}: {:?}", step["op"], r.as_ref().err());
        match r {
            Ok(c) => {
                let entries: Vec<Value> = c.entries.iter().map(|e| e.to_json()).collect();
                assert_eq!(Value::Array(entries), step["entries"], "step {i} entries");
                if let Some(s) = step.get("skipped") {
                    let ours: Vec<Value> = c.skipped.iter().map(|x| x.to_json()).collect();
                    assert_eq!(Value::Array(ours), *s, "step {i} skipped");
                }
                st = c.state;
            }
            Err(e) => {
                assert_eq!(e.code, step["code"].as_str().unwrap(), "step {i}");
                assert_eq!(e.error, step["error"].as_str().unwrap(), "step {i}");
            }
        }
        assert_eq!(pretty(&st.to_json()), step["file"].as_str().unwrap(), "step {i} file text");
    }
}

#[test]
fn ts_written_files_read_back_unchanged() {
    let f = fixture();
    let schema = Schema::from_json(&f["schema"]).unwrap();
    let reg = Registry::from_json(&f["commands"]).unwrap();
    let clock = Fixed { t: AtomicU64::new(0), n: AtomicU64::new(0) };
    let ctx = LoadCtx { schema: &schema, commands: &reg, clock: &clock, app_version: "0.31.0", migrate: None };
    for step in f["steps"].as_array().unwrap() {
        let text = step["file"].as_str().unwrap();
        let raw: Value = serde_json::from_str(text).unwrap();
        let r = load_state(Some(&raw), &ctx);
        assert!(r.problem.is_none() && r.entries.is_empty() && r.dropped.is_empty(), "{:?}", r.dropped);
        assert_eq!(pretty(&r.state.to_json()), text);
    }
}

#[test]
fn export_and_loader_match_the_ts_package() {
    let f = fixture();
    let schema = Schema::from_json(&f["schema"]).unwrap();
    let reg = Registry::from_json(&f["commands"]).unwrap();
    // the export was taken after the resetEntry step
    let steps = f["steps"].as_array().unwrap();
    let at = steps.iter().position(|s| s["op"] == "resetEntry").unwrap();
    let raw: Value = serde_json::from_str(steps[at]["file"].as_str().unwrap()).unwrap();
    let clock = Fixed { t: AtomicU64::new(0), n: AtomicU64::new(0) };
    let st = load_state(Some(&raw), &LoadCtx { schema: &schema, commands: &reg, clock: &clock, app_version: "x", migrate: None }).state;
    let ex = export_file(&st, &schema, &reg, "0.31.0", "Test Mac", f["export"]["now"].as_f64().unwrap());
    assert_eq!(pretty(&ex), f["export"]["text"].as_str().unwrap());

    let migrate: MigrateFn = Arc::new(|old: &Value| {
        let size = old.get("legacy")?.get("size")?.clone();
        let mut m = Migrated::default();
        m.settings.insert("editor.proseSize".into(), size);
        m.settings.insert("appearance.theme".into(), json!("light"));
        m.keys.insert("rename".into(), json!("⌘E"));
        Some(m)
    });
    for l in f["loads"].as_array().unwrap() {
        let clock = Fixed { t: AtomicU64::new(l["clock"]["t"].as_u64().unwrap()), n: AtomicU64::new(l["clock"]["n"].as_u64().unwrap()) };
        let ctx = LoadCtx { schema: &schema, commands: &reg, clock: &clock, app_version: "0.32.0", migrate: Some(&migrate) };
        let r = load_state(Some(&l["raw"]), &ctx);
        assert_eq!(pretty(&r.state.to_json()), l["file"].as_str().unwrap());
        assert_eq!(r.entries.len() as u64, l["entries"].as_u64().unwrap());
        assert_eq!(r.migrated, l["migrated"].as_bool().unwrap());
        let dropped: Vec<Value> = r.dropped.iter().map(|d| json!(d)).collect();
        assert_eq!(Value::Array(dropped), l["dropped"]);
    }
}

#[test]
fn json_text_matches_json_stringify() {
    let f = fixture();
    for case in f["json"].as_array().unwrap() {
        let src = case["src"].as_str().unwrap();
        let v: Value = serde_json::from_str(src).unwrap();
        assert_eq!(app_settings::value::compact(&v), case["compact"].as_str().unwrap(), "compact {src}");
        assert_eq!(pretty(&v), case["pretty"].as_str().unwrap(), "pretty {src}");
    }
    for case in f["numberStrings"].as_array().unwrap() {
        assert_eq!(app_settings::value::js_string(&case["n"]), case["s"].as_str().unwrap(), "String({})", case["n"]);
    }
}
