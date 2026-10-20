//! The write path and the agent CLI, at the crate seam: concurrent writers lose nothing, a stale lock is recovered, a damaged file is
//! set aside, linked folders and files are refused, outside changes are seen within a second, and every CLI verb answers with its
//! documented exit code and `--json` shape.

use app_settings::cli::{run_settings_cli, CliIo, CliOptions};
use app_settings::{Actor, Registry, Schema, SettingsStore, StoreOptions};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn schema() -> Arc<Schema> {
    Arc::new(
        Schema::from_json(&json!({
            "app": "testapp",
            "sections": [{ "id": "sync", "label": "Sync", "icon": "refresh-cw" }],
            "settings": [
                { "key": "sync.hours", "label": "Sync every", "section": "sync", "type": "number", "default": 6, "min": 0, "max": 10000 },
                { "key": "sync.onLaunch", "label": "Sync when the app opens", "section": "sync", "type": "toggle", "default": true },
                { "key": "sync.folder", "label": "Archive folder", "section": "sync", "type": "path", "default": "~/Archive" },
                { "key": "accounts.readwise", "label": "Readwise key", "section": "accounts", "type": "secret", "default": null },
            ]
        }))
        .unwrap(),
    )
}

fn registry() -> Arc<Registry> {
    Arc::new(Registry::from_json(&json!([{ "id": "open", "label": "Open", "key": "⌘O" }, { "id": "pin", "label": "Pin", "key": "⌘D" }])).unwrap())
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("app-settings-{name}-{}-{}", std::process::id(), app_settings::value::rand_hex(4)));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d.canonicalize().unwrap()
}

fn store(dir: &Path, who: Actor) -> Arc<SettingsStore> {
    let mut o = StoreOptions::new(dir.to_path_buf(), schema(), registry(), "1.0.0");
    o.who = who;
    o.log = Arc::new(|_| {});
    SettingsStore::open(o)
}

#[test]
fn two_writers_at_once_lose_no_change() {
    let dir = tmp("race");
    let n = 40;
    let handles: Vec<_> = (0..2)
        .map(|w| {
            let dir = dir.clone();
            std::thread::spawn(move || {
                let s = store(&dir, Actor::Agent(format!("w{w}")));
                for i in 0..n {
                    s.set("sync.hours", &json!(100 * (w + 1) + i)).unwrap();
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    let s = store(&dir, Actor::You);
    assert_eq!(s.view().changes.len(), 2 * n as usize, "every change logged once");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_dead_writers_lock_is_broken() {
    let dir = tmp("stale");
    std::fs::write(dir.join("settings.json.lock"), r#"{"pid":999999,"at":1,"nonce":"x"}"#).unwrap();
    let s = store(&dir, Actor::You);
    s.set("sync.onLaunch", &json!(false)).unwrap();
    assert!(!dir.join("settings.json.lock").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_damaged_file_is_set_aside_and_defaults_load() {
    let dir = tmp("damaged");
    std::fs::write(dir.join("settings.json"), "{ not json").unwrap();
    let s = store(&dir, Actor::You);
    let names: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
    assert!(names.iter().any(|n| n.starts_with("settings.json.damaged-")), "{names:?}");
    let v = s.view();
    assert!(v.settings.is_empty());
    assert_eq!(v.changes[0].note.as_deref(), Some("INVALID_SAVED_VALUE"));
    let text = std::fs::read_to_string(dir.join("settings.json")).unwrap();
    assert!(text.contains("\"format\": \"bridle-settings\""));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_linked_folder_or_file_is_refused() {
    let base = tmp("linked");
    let real = base.join("real");
    std::fs::create_dir_all(&real).unwrap();
    let link = base.join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let s = store(&link, Actor::You);
    let e = s.set("sync.hours", &json!(3)).unwrap_err();
    assert_eq!(e.code, "refused");
    assert!(!real.join("settings.json").exists());

    let dir = base.join("plain");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(base.join("elsewhere.json"), "{}").unwrap();
    std::os::unix::fs::symlink(base.join("elsewhere.json"), dir.join("settings.json")).unwrap();
    let s = store(&dir, Actor::You);
    assert_eq!(s.problem().map(|p| p.code), Some("refused"));
    assert_eq!(s.set("sync.hours", &json!(3)).unwrap_err().code, "refused");
    assert_eq!(std::fs::read_to_string(base.join("elsewhere.json")).unwrap(), "{}");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn an_outside_change_is_seen_within_a_second() {
    let dir = tmp("watch");
    let seen = Arc::new(AtomicUsize::new(0));
    let mut o = StoreOptions::new(dir.clone(), schema(), registry(), "1.0.0");
    let s2 = seen.clone();
    o.on_change = Some(Arc::new(move |_| {
        s2.fetch_add(1, Ordering::SeqCst);
    }));
    o.log = Arc::new(|_| {});
    let app = SettingsStore::open(o);
    app.watch();
    std::thread::sleep(Duration::from_millis(250));
    let other = store(&dir, Actor::Agent("cli".into()));
    other.set("sync.hours", &json!(12)).unwrap();
    let t0 = Instant::now();
    while seen.load(Ordering::SeqCst) == 0 && t0.elapsed() < Duration::from_secs(2) {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(t0.elapsed() < Duration::from_secs(1), "took {:?}", t0.elapsed());
    assert_eq!(app.view().settings.get("sync.hours"), Some(&json!(12)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_file_is_private() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tmp("mode");
    store(&dir, Actor::You).set("sync.hours", &json!(1)).unwrap();
    let mode = std::fs::metadata(dir.join("settings.json")).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- CLI --------------------------------------------------------------------------------------------------------------------------

fn cli(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let opts = CliOptions { dir: dir.to_path_buf(), schema: schema(), commands: registry(), version: "1.0.0".into(), app_name: "Test App".into(), migrate: None, root: None };
    let (mut out, mut err) = (String::new(), String::new());
    let code = {
        let mut so = |t: &str| out.push_str(t);
        let mut se = |t: &str| err.push_str(t);
        let mut io = CliIo { stdout: &mut so, stderr: &mut se, env: Default::default(), cwd: dir.to_path_buf(), host_name: "Test Mac".into(), clock: None };
        let argv: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        run_settings_cli(&argv, &opts, &mut io)
    };
    (code, out, err)
}

fn j(s: &str) -> Value {
    serde_json::from_str(s.trim()).unwrap_or_else(|e| panic!("{e}: {s}"))
}

#[test]
fn every_cli_verb_answers_with_its_exit_code_and_json() {
    let dir = tmp("cli");
    let (c, out, _) = cli(&dir, &["list", "--json"]);
    assert_eq!(c, 0);
    let rows = j(&out)["settings"].as_array().unwrap().clone();
    assert_eq!(rows.len(), 4);
    assert_eq!(rows[3], json!({ "key": "accounts.readwise", "label": "Readwise key", "section": "accounts", "type": "secret", "value": null, "default": null, "changed": false, "secret": true }));

    let (c, out, _) = cli(&dir, &["set", "sync.hours", "24", "--json", "--agent", "Claude Code"]);
    assert_eq!(c, 0);
    let v = j(&out);
    assert_eq!(v["changed"], true);
    assert_eq!(v["entries"][0]["who"], json!({ "kind": "agent", "name": "Claude Code" }));
    assert_eq!(v["entries"][0]["to"], json!(24));

    let (c, out, _) = cli(&dir, &["set", "sync.hours", "24", "--json"]);
    assert_eq!((c, j(&out)["changed"].clone()), (0, json!(false)), "a value already in place logs nothing");

    let (c, out, _) = cli(&dir, &["get", "sync.hours"]);
    assert_eq!((c, out.as_str()), (0, "24\n"));

    let (c, out, _) = cli(&dir, &["set", "accounts.readwise", "abc", "--json"]);
    assert_eq!(c, 2);
    assert_eq!(j(&out)["code"], "secret");
    let (c, _, err) = cli(&dir, &["set", "sync.hours", "lots"]);
    assert_eq!(c, 2);
    assert!(err.contains("Must be a number"));
    let (c, _, _) = cli(&dir, &["frobnicate"]);
    assert_eq!(c, 2);

    let (c, out, _) = cli(&dir, &["reset", "--all", "--json"]);
    assert_eq!(c, 3);
    assert_eq!(j(&out)["needsConfirmation"], true);

    let (c, out, _) = cli(&dir, &["export", "out.json", "--json"]);
    assert_eq!(c, 0);
    assert_eq!(j(&out)["settings"], 1);
    let exported = std::fs::read_to_string(dir.join("out.json")).unwrap();
    assert!(!exported.contains("readwise"), "no key in an export");
    let (c, _, _) = cli(&dir, &["export", "out.json"]);
    assert_eq!(c, 2, "never over an existing file");

    let (c, out, _) = cli(&dir, &["changes", "--json", "--limit", "5"]);
    assert_eq!(c, 0);
    assert_eq!(j(&out)["total"], 1);

    let (c, _, _) = cli(&dir, &["reset", "sync.hours"]);
    assert_eq!(c, 0);
    let (c, out, _) = cli(&dir, &["import", "out.json", "--json"]);
    assert_eq!(c, 3);
    assert_eq!(j(&out)["preview"]["changes"][0]["to"], 24);
    let (c, out, _) = cli(&dir, &["import", "out.json", "--yes", "--json"]);
    assert_eq!(c, 0);
    let v = j(&out);
    assert_eq!(v["applied"], true);
    assert_eq!(v["entries"][0]["who"]["kind"], "import");
    assert_eq!(v["entries"][0]["who"]["by"], json!({ "kind": "agent", "name": "agent" }));
    let (c, out, _) = cli(&dir, &["undo-import", "--last", "--json"]);
    assert_eq!(c, 0, "{out}");
    let (_, out, _) = cli(&dir, &["get", "sync.hours", "--json"]);
    assert_eq!(j(&out)["value"], 6);

    std::fs::write(dir.join("wrong.json"), r#"{"format":"bridle-settings-export","version":1,"app":"talkweaver","settings":{}}"#).unwrap();
    let (c, out, _) = cli(&dir, &["import", "wrong.json", "--json"]);
    assert_eq!(c, 2);
    assert_eq!(j(&out)["refused"]["code"], "other-app");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_cli_refuses_another_apps_file_and_prints_no_value() {
    let dir = tmp("cli-other");
    std::fs::write(dir.join("settings.json"), r#"{"format":"bridle-settings","version":2,"app":"other","settings":{"sync.hours":7},"keys":{},"changes":[]}"#).unwrap();
    let (c, out, _) = cli(&dir, &["get", "sync.hours", "--json"]);
    assert_eq!(c, 1);
    assert_eq!(j(&out)["problem"], "other-app");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_secret_left_in_the_file_is_purged_at_once() {
    let dir = tmp("purge");
    std::fs::write(dir.join("settings.json"), r#"{"format":"bridle-settings","version":2,"app":"testapp","settings":{"accounts.readwise":"FAKE-not-a-key"},"keys":{},"changes":[]}"#).unwrap();
    let _s = store(&dir, Actor::You);
    let text = std::fs::read_to_string(dir.join("settings.json")).unwrap();
    assert!(!text.contains("FAKE-not-a-key"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(feature = "keychain")]
#[test]
#[ignore = "touches the real login Keychain; run by hand only with the owner's go"]
fn keychain_slots_write_read_back_and_remove_under_a_throwaway_service() {
    use app_settings::SecretStore;
    let service = format!("app-settings test {}", app_settings::value::rand_hex(6));
    let k = app_settings::KeychainStore::new(&service);
    assert!(!k.has("slot").unwrap());
    k.set("slot", "fake-value-for-test").unwrap();
    assert!(k.has("slot").unwrap());
    k.remove("slot").unwrap();
    assert!(!k.has("slot").unwrap());
}
