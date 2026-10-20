//! Highlight Scout on the shared settings design: the schema and command registry (one JSON declaration each, read by the window and
//! here), the store in the app's config folder, the first-launch migration from `config.toml`, the Readwise key in the Keychain, the
//! runtime `Config` the rest of the app reads, and the `highlight-scout settings …` CLI.

use crate::config::{self, Config};
use app_settings::state::value_of;
use app_settings::file::FolderGuard;
use app_settings::value::Obj;
use app_settings::{Actor, Migrated, Registry, Schema, SecretStore, SettingsState, SettingsStore, StoreOptions};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub const SCHEMA_JSON: &str = include_str!("../../src/settings/schema.json");
pub const COMMANDS_JSON: &str = include_str!("../../src/settings/commands.json");
pub const APP_NAME: &str = "Highlight Scout";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Keychain service of the Readwise token (the R2 keys keep their own service, see `r2.rs`).
pub const KEYCHAIN_SERVICE: &str = "Highlight Scout";
pub const READWISE_SLOT: &str = "accounts.readwise";
pub const R2_ACCESS_SLOT: &str = "accounts.r2AccessKeyId";
pub const R2_SECRET_SLOT: &str = "accounts.r2SecretAccessKey";

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_default()
}

/// `~/x` → `$HOME/x`.
pub fn expand(p: &str) -> String {
    match p.strip_prefix("~/") {
        Some(rest) => home().join(rest).to_string_lossy().to_string(),
        None if p == "~" => home().to_string_lossy().to_string(),
        None => p.to_string(),
    }
}

/// Settings that describe this Mac's own Cloudflare account: an import never sets them (the prefix and the on/off switch do import).
pub const THIS_MAC_ONLY: [&str; 3] = ["backup.r2Endpoint", "backup.r2AccountId", "backup.r2Bucket"];
pub const MSG_THIS_MAC_ONLY: &str = "Set on this Mac only";

/// Why `chord` cannot be the summon shortcut (None when it can, or when it is "" for none): a key combination the global shortcut
/// can register, with ⌘, ⌃ or ⌥ (a Shift-only or bare key would take a key from typing in every app).
pub fn summon_error(chord: &str) -> Option<String> {
    use std::str::FromStr;
    if chord.is_empty() {
        return None;
    }
    let bad = || Some("That is not a key combination Highlight Scout can listen for from any app".to_string());
    if !app_settings::commands::is_valid_chord(chord) || app_settings::commands::canon_chord(chord) != chord || !app_settings::commands::is_typing_safe(chord) {
        return bad();
    }
    let Some(acc) = app_settings::commands::to_accelerator(chord) else { return bad() };
    if tauri_plugin_global_shortcut::Shortcut::from_str(&acc).is_err() {
        return bad();
    }
    None
}

/// `backup.r2Endpoint`: empty, or an https address on Cloudflare R2 (`https://<account>.r2.cloudflarestorage.com`), with no user name.
pub fn r2_endpoint_error(v: &str) -> Option<String> {
    if v.is_empty() {
        return None;
    }
    let bad = || Some("Must be empty or an https address ending in .r2.cloudflarestorage.com".to_string());
    let Ok(u) = tauri::Url::parse(v) else { return bad() };
    let host = u.host_str().unwrap_or("").to_ascii_lowercase();
    let label = host.strip_suffix(".r2.cloudflarestorage.com").unwrap_or("");
    if u.scheme() != "https" || label.is_empty() || !u.username().is_empty() || u.password().is_some() {
        return bad();
    }
    None
}

/// `backup.r2AccountId`: empty, or letters and digits only.
pub fn r2_account_error(v: &str) -> Option<String> {
    (!v.chars().all(|c| c.is_ascii_alphanumeric())).then(|| "Must be empty or letters and digits only".to_string())
}

pub fn schema() -> Arc<Schema> {
    let v: Value = serde_json::from_str(SCHEMA_JSON).expect("schema.json parses");
    let s = Schema::from_json(&v).expect("schema.json is a valid schema");
    Arc::new(
        s.with_validate(Arc::new(|d, v| {
            let t = v.as_str().unwrap_or("");
            match d.key.as_str() {
                "general.summonShortcut" => summon_error(t),
                "backup.r2Endpoint" => r2_endpoint_error(t),
                "backup.r2AccountId" => r2_account_error(t),
                _ => None,
            }
        }))
        .with_can_apply(Arc::new(|d, v| {
            if THIS_MAC_ONLY.contains(&d.key.as_str()) {
                return Some(MSG_THIS_MAC_ONLY.to_string());
            }
            // An imported folder that is not on this Mac is skipped, not imported broken.
            if d.kind != app_settings::SettingType::Path {
                return None;
            }
            let p = expand(v.as_str().unwrap_or(""));
            (!Path::new(&p).exists()).then(|| "This folder or file is not on this Mac".to_string())
        })),
    )
}

pub fn registry() -> Arc<Registry> {
    let v: Value = serde_json::from_str(COMMANDS_JSON).expect("commands.json parses");
    Arc::new(Registry::from_json(&v).expect("commands.json is a command list"))
}

/// The Keychain slots of Accounts and keys: (slot id, label).
pub const SLOTS: [(&str, &str); 3] = [
    (READWISE_SLOT, "Readwise access token"),
    (R2_ACCESS_SLOT, "R2 access key id"),
    (R2_SECRET_SLOT, "R2 secret access key"),
];

pub fn secrets() -> Arc<dyn SecretStore> {
    Arc::new(app_settings::MappedKeychain {
        slots: vec![
            (READWISE_SLOT.into(), KEYCHAIN_SERVICE.into(), "readwise_api_key".into()),
            (R2_ACCESS_SLOT.into(), crate::r2::KEYCHAIN_SERVICE.into(), crate::r2::ACCESS_KEY_ACCOUNT.into()),
            (R2_SECRET_SLOT.into(), crate::r2::KEYCHAIN_SERVICE.into(), crate::r2::SECRET_KEY_ACCOUNT.into()),
        ],
    })
}

pub fn open_store(dir: &Path, who: Actor, on_change: Option<app_settings::store::ChangeFn>) -> Arc<SettingsStore> {
    let mut o = StoreOptions::new(dir.to_path_buf(), schema(), registry(), VERSION);
    o.who = who;
    o.on_change = on_change;
    SettingsStore::open(o)
}

// ---- the old config.toml -----------------------------------------------------------------------------------------------------------

/// `CmdOrCtrl+Alt+Shift+H` → `⌘⌥⇧H` (None when it is not one key with modifiers).
pub fn chord_from_accelerator(acc: &str) -> Option<String> {
    let mut mods = [false; 4]; // ⌘ ⌃ ⌥ ⇧
    let mut key = None;
    for part in acc.split('+').map(str::trim) {
        match part.to_ascii_lowercase().as_str() {
            "cmdorctrl" | "commandorcontrol" | "cmd" | "command" | "super" | "meta" => mods[0] = true,
            "ctrl" | "control" => mods[1] = true,
            "alt" | "option" => mods[2] = true,
            "shift" => mods[3] = true,
            _ => key = Some(part.to_string()),
        }
    }
    let key = key?;
    let k = match key.to_ascii_lowercase().as_str() {
        "space" => "Space".to_string(),
        "up" => "↑".into(),
        "down" => "↓".into(),
        "left" => "←".into(),
        "right" => "→".into(),
        "enter" | "return" => "↵".into(),
        _ if key.chars().count() == 1 => key.to_uppercase(),
        _ => key,
    };
    let chord: String = ['⌘', '⌃', '⌥', '⇧'].iter().zip(mods).filter(|(_, on)| *on).map(|(c, _)| *c).collect::<String>() + &k;
    app_settings::commands::is_valid_chord(&chord).then_some(chord)
}

/// The settings a `config.toml` holds, by setting key: only schema-known, non-secret settings (never the token, never the sync cursors).
pub fn migrated_from(cfg: &Config) -> Migrated {
    let d = Config::default();
    let mut s = Obj::new();
    let mut put = |k: &str, v: Value| {
        s.insert(k.into(), v);
    };
    put("general.launchAtLogin", json!(cfg.autostart_enabled));
    if let Some(c) = chord_from_accelerator(&cfg.shortcut) {
        put("general.summonShortcut", json!(c));
    }
    // the old defaults were absolute paths; the schema's are `~/…`
    put("sources.archiveFolder", json!(if cfg.archive_path == d.archive_path { "~/Documents/Highlight Scout".to_string() } else { cfg.archive_path.clone() }));
    put("sources.zoteroDatabase", json!(if cfg.zotero_db_path == d.zotero_db_path { "~/Zotero/zotero.sqlite".to_string() } else { cfg.zotero_db_path.clone() }));
    put("sync.onLaunch", json!(cfg.sync_on_launch));
    put("sync.intervalHours", json!(cfg.sync_interval_hours));
    put("sync.reminderDays", json!(cfg.import_reminder_days));
    put("sync.ocrOnImport", json!(cfg.ocr_on_import));
    put("search.resultLimit", json!(cfg.result_limit));
    put("backup.r2Enabled", json!(cfg.r2_enabled));
    put("backup.r2AccountId", json!(cfg.r2_account_id));
    put("backup.r2Bucket", json!(cfg.r2_bucket));
    put("backup.r2Prefix", json!(cfg.r2_prefix));
    put("backup.r2Endpoint", json!(cfg.r2_endpoint));
    Migrated { settings: s, keys: Obj::new() }
}

/// Records what was taken from `config.toml` and when, in the settings folder: per setting the legacy value that was handled (applied,
/// already equal, or kept out because the new store changed it), per sync cursor the legacy cursor that was handled, and the time.
pub const MIGRATION_STAMP: &str = "settings-migration.json";
const LEGACY_FILE: &str = "config.toml";
const CURSOR_NAMES: [&str; 3] = ["readwise", "readwiseTweets", "zotero"];

fn digest(text: &str) -> String {
    use sha2::Digest;
    hex::encode(sha2::Sha256::digest(text.as_bytes()))
}

/// The stamp as read: the legacy values already handled and when the last migration ran (ms; 0 when there was none).
#[derive(Debug, Default, Clone, PartialEq)]
struct Stamp {
    at: f64,
    settings: Obj,
    cursors: Obj,
}

fn read_stamp(g: &FolderGuard, cfg_now: &Config, text_now: &str) -> Stamp {
    let Ok(Some(r)) = g.read(MIGRATION_STAMP) else { return Stamp::default() };
    let Ok(v) = serde_json::from_str::<Value>(&r.text) else { return Stamp::default() };
    let at = v.get("migratedAtMs").and_then(Value::as_f64).or_else(|| {
        let iso = v.get("migratedAt")?.as_str()?;
        chrono::DateTime::parse_from_rfc3339(iso).ok().map(|d| d.timestamp_millis() as f64)
    });
    let obj = |k: &str| v.get(k).and_then(Value::as_object).cloned();
    match (obj("settings"), obj("cursors")) {
        (Some(settings), cursors) => Stamp { at: at.unwrap_or(0.0), settings, cursors: cursors.unwrap_or_default() },
        // a stamp from 0.6.0-preview.9 (the file's hash only): when the file is still that one, every value in it was handled
        _ if v.get("legacySha256").and_then(Value::as_str) == Some(digest(text_now).as_str()) => Stamp {
            at: at.unwrap_or(0.0),
            settings: migrated_from(cfg_now).settings,
            cursors: cursors_obj(&config::Cursors::from(cfg_now)),
        },
        _ => Stamp { at: at.unwrap_or(0.0), ..Default::default() },
    }
}

/// Whether the old version's cursor is later than this version's: both are RFC 3339 instants (compared as instants, so "Z" and
/// "+00:00" agree). No cursor here (or an unreadable one) takes the old one; an unreadable old cursor is never taken.
fn later_cursor(legacy: &str, current: &str) -> bool {
    let at = |s: &str| chrono::DateTime::parse_from_rfc3339(s.trim()).ok();
    match (at(legacy), at(current)) {
        (None, _) => false,
        (Some(l), Some(c)) => l > c,
        (Some(_), None) => true,
    }
}

fn cursors_obj(c: &config::Cursors) -> Obj {
    let mut o = Obj::new();
    for (n, v) in CURSOR_NAMES.iter().zip([&c.readwise, &c.readwise_tweets, &c.zotero]) {
        o.insert((*n).into(), json!(v));
    }
    o
}

fn write_stamp(g: &FolderGuard, st: &Stamp, legacy_text: &str) -> Result<(), String> {
    let v = json!({
        "legacyFile": LEGACY_FILE, "legacySha256": digest(legacy_text), "migratedBy": VERSION,
        "migratedAt": app_settings::value::iso(st.at), "migratedAtMs": st.at, "settings": st.settings, "cursors": st.cursors,
    });
    g.write(MIGRATION_STAMP, &app_settings::value::pretty(&v), None)
}

/// The old file's text with the Readwise token's value emptied (that one field; every other line kept as it is).
fn without_token(text: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for l in text.split_inclusive('\n') {
        let body = l.trim_end_matches('\n');
        let is_key = body.split_once('=').is_some_and(|(k, _)| k.trim() == "readwise_api_key");
        if is_key {
            out.push(format!("readwise_api_key = \"\"{}", if l.ends_with('\n') { "\n" } else { "" }));
        } else {
            out.push(l.to_string());
        }
    }
    out.concat()
}

#[derive(Debug, Default, PartialEq)]
pub struct MigrationReport {
    /// At least one setting from `config.toml` was written to the store.
    pub settings_migrated: bool,
    /// Settings the old version changed that were left as they are, because they were changed here after the last migration.
    pub kept: Vec<String>,
    /// Settings whose write did not land (not stamped; tried again at the next start).
    pub failed: Vec<String>,
    /// The old version's sync cursors were taken over (only when the caller may write the app's sync state).
    pub cursors_migrated: bool,
    /// Some(true) moved to Keychain and emptied in the old file, Some(false) not (left where it was), None no token.
    pub key_moved: Option<bool>,
}

/// Whether the migration may write `sync-state.json` now: returns a guard held for the write (the app lock, or a token when the
/// caller already holds it), or None when another process may be writing the app's state (the cursors wait for the next start).
pub type CursorLock<'a> = &'a dyn Fn() -> Option<Box<dyn std::any::Any>>;

/// Moves `config.toml` onto shared settings. The old file is never copied, renamed or deleted: it stays as the old version's file (it
/// is the backup). It is read on every start; each setting in it is handled once per value (the stamp records it):
/// - a value equal to the store's, or one the schema refuses, is only recorded;
/// - a value the old version changed is applied, unless the new store's changelog changed that setting at or after the last migration
///   (anything but an app update): then the new store's value is kept;
/// - a write that does not read back is not recorded and is tried again at the next start.
///
/// A sync cursor is taken only under `cursor_lock`, only when its old value changed since the stamp, and only when that value is later
/// than the one in `sync-state.json` (the values decide, never the files' mtimes).
/// The one edit to the old file: the Readwise token is emptied, after the Keychain write reads back, by compare and swap on the file as
/// it is then (an edit in between is never overwritten). Every file is read and written through a guard pinned on the folder.
pub fn migrate_legacy(dir: &Path, store: &SettingsStore, secrets: &dyn SecretStore, cursor_lock: CursorLock) -> Result<MigrationReport, String> {
    let mut report = MigrationReport::default();
    if store.problem().is_some() {
        return Ok(report);
    }
    let ensure_file = || -> Result<(), String> {
        if !dir.join("settings.json").exists() {
            store.touch().map_err(|e| e.error)?;
        }
        Ok(())
    };
    let legacy_there = std::fs::symlink_metadata(dir.join(LEGACY_FILE)).is_ok_and(|m| m.is_file());
    if !legacy_there {
        ensure_file()?;
        return Ok(report);
    }
    let g = FolderGuard::pin(dir)?;
    let Some(first) = g.read(LEGACY_FILE).map_err(|e| format!("read config.toml: {e}"))? else {
        ensure_file()?;
        return Ok(report);
    };
    let cfg = config::parse_config_text(&first.text);
    let before = read_stamp(&g, &cfg, &first.text);
    let mut stamp = before.clone();
    let now = app_settings::value::now_ms();

    // settings
    let view = store.view();
    let schema = store.schema();
    let mut to_apply = Obj::new();
    for (k, v) in &migrated_from(&cfg).settings {
        if stamp.settings.get(k).is_some_and(|s| app_settings::value::js_eq(s, v)) {
            continue;
        }
        let valid = schema.def(k).is_some_and(|d| schema.check_value(d, v).is_none());
        let same = value_of(&view, schema, k).is_some_and(|c| app_settings::value::js_eq(&c, v));
        if !valid || same {
            stamp.settings.insert(k.clone(), v.clone());
            continue;
        }
        // Accepted residual (Sol r2 New 1): `view` is this store's snapshot; a second store writing this key during start-up migration
        // (the same user, at the same moment) is not seen here and the old version's value wins. Same-user race, not fixed.
        let changed_here = view.changes.iter().any(|e| e.key() == Some(k.as_str()) && e.at >= before.at && !matches!(e.who, app_settings::Who::AppUpdate(_)));
        if changed_here {
            stamp.settings.insert(k.clone(), v.clone());
            report.kept.push(k.clone());
            continue;
        }
        to_apply.insert(k.clone(), v.clone());
    }
    if !to_apply.is_empty() {
        let wrote = store.migrate_values(&Migrated { settings: to_apply.clone(), keys: Obj::new() }, true);
        let back = open_store(dir, Actor::You, None).view();
        for (k, v) in &to_apply {
            if wrote.is_ok() && value_of(&back, schema, k).is_some_and(|g| app_settings::value::js_eq(&g, v)) {
                stamp.settings.insert(k.clone(), v.clone());
                report.settings_migrated = true;
            } else {
                report.failed.push(k.clone());
            }
        }
    }
    ensure_file()?;

    // sync cursors: the old version's, per cursor, when its value changed since the stamp and is later than this version's own. Freshness
    // comes from the cursor values, never the file's mtime (this migration's own token-emptying write would make an old cursor look new).
    let legacy_cur = cursors_obj(&config::Cursors::from(&cfg));
    let advanced: Vec<&str> = CURSOR_NAMES
        .iter()
        .copied()
        .filter(|n| {
            let lv = legacy_cur.get(*n).and_then(Value::as_str).unwrap_or("");
            !lv.is_empty() && stamp.cursors.get(*n).and_then(Value::as_str) != Some(lv)
        })
        .collect();
    if !advanced.is_empty() {
        if let Some(_held) = cursor_lock() {
            let state = g.read(config::CURSORS_FILE).map_err(|e| format!("sync-state.json: {e}"))?;
            let mut cur = config::load_cursors(dir);
            let mut newer = false;
            for n in &advanced {
                let lv = legacy_cur.get(*n).and_then(Value::as_str).unwrap_or("").to_string();
                let slot = match *n {
                    "readwise" => &mut cur.readwise,
                    "readwiseTweets" => &mut cur.readwise_tweets,
                    _ => &mut cur.zotero,
                };
                if later_cursor(&lv, slot) {
                    *slot = lv;
                    newer = true;
                }
            }
            if newer {
                g.write(config::CURSORS_FILE, &config::cursors_text(&cur), state.as_ref()).map_err(|e| format!("sync-state.json: {e}"))?;
                report.cursors_migrated = true;
            }
            for n in &advanced {
                stamp.cursors.insert((*n).into(), legacy_cur[*n].clone());
            }
        }
    }

    // the Readwise token, into the Keychain, then out of the old file
    let mut stamped_text = first.text.clone();
    if !cfg.readwise_api_key.is_empty() {
        let token = cfg.readwise_api_key.as_str();
        let moved = secrets.set(READWISE_SLOT, token).is_ok() && secrets.get(READWISE_SLOT).ok().flatten().as_deref() == Some(token);
        if moved {
            let mut emptied = false;
            for _ in 0..3 {
                let Some(fresh) = g.read(LEGACY_FILE).map_err(|e| format!("read config.toml: {e}"))? else { break };
                if config::parse_config_text(&fresh.text).readwise_api_key != token {
                    break; // the old version saved another token meanwhile: it is moved at the next start
                }
                let edited = without_token(&fresh.text);
                match g.write(LEGACY_FILE, &edited, Some(&fresh)) {
                    Ok(()) => {
                        if fresh.text == first.text {
                            stamped_text = edited;
                        }
                        emptied = true;
                        break;
                    }
                    Err(e) if e == app_settings::file::MSG_CHANGED => continue,
                    Err(e) => return Err(format!("empty the token in config.toml: {e}")),
                }
            }
            if !emptied {
                eprintln!("Settings: the Readwise token is in Keychain; config.toml changed meanwhile and still holds it until the next start");
            }
        }
        report.key_moved = Some(moved);
    }
    if stamp.settings != before.settings || stamp.cursors != before.cursors || before.at == 0.0 {
        stamp.at = now;
        write_stamp(&g, &stamp, &stamped_text)?;
    }
    Ok(report)
}

// ---- the runtime view --------------------------------------------------------------------------------------------------------------

/// The `Config` the rest of the app reads, from the settings state, the Keychain and the sync cursors.
pub fn runtime_config(state: &SettingsState, schema: &Schema, secrets: &dyn SecretStore, dir: &Path) -> Config {
    let v = |k: &str| value_of(state, schema, k).unwrap_or(Value::Null);
    let s = |k: &str| v(k).as_str().unwrap_or("").to_string();
    let n = |k: &str| v(k).as_f64().unwrap_or(0.0).max(0.0) as u32;
    let b = |k: &str| v(k).as_bool().unwrap_or(false);
    let cursors = config::load_cursors(dir);
    let mut key = secrets.get(READWISE_SLOT).ok().flatten().unwrap_or_default();
    if key.is_empty() {
        // not moved yet (the Keychain refused): the old file still holds it
        if let Ok(text) = std::fs::read_to_string(dir.join("config.toml")) {
            key = config::parse_config_text(&text).readwise_api_key;
        }
    }
    if key.is_empty() {
        key = std::env::var("READWISE_API_KEY").unwrap_or_default();
    }
    let summon = s("general.summonShortcut");
    Config {
        readwise_api_key: key,
        archive_path: expand(&s("sources.archiveFolder")),
        shortcut: app_settings::commands::to_accelerator(&summon).unwrap_or_default(),
        zotero_db_path: expand(&s("sources.zoteroDatabase")),
        result_limit: n("search.resultLimit").clamp(1, 300),
        readwise_archive_path: Config::default().readwise_archive_path,
        readwise_last_sync: cursors.readwise,
        import_reminder_days: n("sync.reminderDays"),
        sync_on_launch: b("sync.onLaunch"),
        sync_interval_hours: n("sync.intervalHours"),
        readwise_tweets_last_sync: cursors.readwise_tweets,
        zotero_last_sync: cursors.zotero,
        autostart_enabled: b("general.launchAtLogin"),
        ocr_on_import: b("sync.ocrOnImport"),
        r2_enabled: b("backup.r2Enabled"),
        r2_account_id: s("backup.r2AccountId"),
        r2_endpoint: s("backup.r2Endpoint"),
        r2_bucket: s("backup.r2Bucket"),
        r2_prefix: s("backup.r2Prefix"),
    }
}

/// Opens the store in the app's config folder, runs the migration, and returns the runtime config (headless paths and start-up).
pub fn load(on_change: Option<app_settings::store::ChangeFn>) -> (Arc<SettingsStore>, Config) {
    let dir = config::base_dir();
    let store = open_store(&dir, Actor::You, on_change);
    let sec = secrets();
    // the app holds the app lock for its whole run (taken before this is called)
    if let Err(e) = migrate_legacy(&dir, &store, sec.as_ref(), &|| Some(Box::new(()))) {
        eprintln!("Settings: moving config.toml to settings.json did not finish: {e}");
    }
    let cfg = runtime_config(&store.view(), store.schema(), sec.as_ref(), &dir);
    (store, cfg)
}

/// `highlight-scout settings …`: the agent CLI over the same store and changelog (runs the migration first; takes the app lock only for
/// the moment it writes migrated sync cursors, and skips that step while the app runs).
pub fn run_cli(args: &[String]) -> i32 {
    let dir = config::base_dir();
    {
        let store = open_store(&dir, Actor::You, None);
        // the sync cursors only while no app runs: the app lock, taken for the cursor write alone (never across the Keychain step)
        let lock = || crate::app_lock::AppLock::acquire(&config::lock_path()).ok().map(|l| Box::new(l) as Box<dyn std::any::Any>);
        if let Err(e) = migrate_legacy(&dir, &store, secrets().as_ref(), &lock) {
            eprintln!("Settings: moving config.toml to settings.json did not finish: {e}");
        }
    }
    let opts = app_settings::CliOptions { dir, schema: schema(), commands: registry(), version: VERSION.into(), app_name: APP_NAME.into(), migrate: None, root: None };
    app_settings::run_process(args, &opts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use app_settings::MemoryStore;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("hs-settings-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d.canonicalize().unwrap()
    }

    /// The app's case: it holds the app lock for its whole run.
    fn held() -> Option<Box<dyn std::any::Any>> {
        Some(Box::new(()))
    }
    /// The CLI's case while the app runs: the app lock is taken.
    fn busy() -> Option<Box<dyn std::any::Any>> {
        None
    }

    fn migrate(dir: &Path, store: &SettingsStore, keys: &dyn SecretStore) -> MigrationReport {
        migrate_legacy(dir, store, keys, &held).unwrap()
    }

    /// The old version saving its whole file, a moment later (mtime moves on).
    fn old_version_saves(dir: &Path, text: &str) {
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(dir.join("config.toml"), text).unwrap();
    }

    #[test]
    fn schema_and_registry_load_and_every_shortcut_stands() {
        let s = schema();
        assert_eq!(s.app, "highlight-scout");
        let r = registry();
        assert_eq!(r.defs.len(), 41);
        use app_settings::CommandsAdapter;
        assert!(r.resolve(&[]).rejected.is_empty());
    }

    #[test]
    fn accelerators_become_chords() {
        assert_eq!(chord_from_accelerator("CmdOrCtrl+Alt+Shift+H").as_deref(), Some("⌘⌥⇧H"));
        assert_eq!(chord_from_accelerator("Shift+Cmd+space").as_deref(), Some("⌘⇧Space"));
        assert_eq!(chord_from_accelerator("nonsense"), None);
        assert_eq!(app_settings::commands::to_accelerator("⌘⌥⇧H").as_deref(), Some("Cmd+Alt+Shift+H"));
    }

    #[test]
    fn a_malformed_summon_shortcut_is_refused_and_nothing_is_written() {
        let dir = tmp("summon");
        let store = open_store(&dir, Actor::You, None);
        for bad in ["nonsense", "H", "⇧H", "⌘", "⌘⌘H", "⌥⌘H"] {
            let r = store.set("general.summonShortcut", &json!(bad));
            assert!(r.is_err(), "{bad} was accepted");
        }
        assert!(store.view().changes.is_empty());
        for good in ["", "⌘⇧Space", "⌃⌥K", "⌘⌥⇧H"] {
            assert!(summon_error(good).is_none(), "{good} refused");
        }
        assert!(store.set("general.summonShortcut", &json!("⌃⌥K")).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn r2_account_settings_validate_and_never_import() {
        let dir = tmp("r2rule");
        let store = open_store(&dir, Actor::You, None);
        for bad in ["http://abc.r2.cloudflarestorage.com", "https://evil.example.com", "https://r2.cloudflarestorage.com", "https://u:p@abc.r2.cloudflarestorage.com", "abc"] {
            assert!(store.set("backup.r2Endpoint", &json!(bad)).is_err(), "{bad}");
        }
        assert!(store.set("backup.r2Endpoint", &json!("https://abc123.eu.r2.cloudflarestorage.com")).is_ok());
        assert!(store.set("backup.r2Endpoint", &json!("")).is_ok());
        assert!(store.set("backup.r2AccountId", &json!("abc-123")).is_err());
        assert!(store.set("backup.r2AccountId", &json!("abc123DEF")).is_ok());
        let file = json!({ "format": "bridle-settings-export", "version": 1, "app": "highlight-scout", "settings": {
            "backup.r2Endpoint": "https://other.r2.cloudflarestorage.com", "backup.r2AccountId": "other1", "backup.r2Bucket": "theirs",
            "backup.r2Prefix": "their-prefix", "backup.r2Enabled": true }, "keys": {} });
        let p = store.preview_import(&file.to_string());
        let changed: Vec<String> = p.changes.iter().filter_map(|c| match &c.subject { app_settings::Subject::Key(k) => Some(k.clone()), _ => None }).collect();
        assert_eq!(changed, vec!["backup.r2Prefix".to_string(), "backup.r2Enabled".to_string()]);
        for k in THIS_MAC_ONLY {
            assert!(p.skipped.iter().any(|s| s.key.as_deref() == Some(k) && s.reason == MSG_THIS_MAC_ONLY), "{k} not skipped");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_fabricated_preview_is_refused_on_apply() {
        let dir = tmp("fabricated");
        let store = open_store(&dir, Actor::You, None);
        let mut p = app_settings::ImportPreview::from_json(&json!({ "changes": [{ "key": "sources.archiveFolder", "from": "", "to": "/nowhere/at/all", "section": "sources" }], "skipped": [], "alreadyMatch": 0, "source": { "app": "other-app" } })).unwrap();
        assert!(store.apply_import(&p, "x.json").is_err(), "another app's preview applied");
        p.source.as_mut().unwrap().app = "highlight-scout".into();
        assert!(store.apply_import(&p, "x.json").is_err(), "a folder not on this Mac applied");
        p.changes[0].subject = app_settings::Subject::Key("backup.r2Bucket".into());
        p.changes[0].to = json!("theirs");
        assert!(store.apply_import(&p, "x.json").is_err(), "a this-Mac-only setting applied");
        assert!(store.view().changes.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    const OLD: &str = "readwise_api_key = \"FAKE-TOKEN-FOR-TEST\"\narchive_path = \"/tmp/hs-archive\"\nshortcut = \"CmdOrCtrl+Alt+Shift+H\"\nresult_limit = 120\nsync_on_launch = false\nsync_interval_hours = 6\nreadwise_last_sync = \"2026-10-01T00:00:00Z\"\nr2_bucket = \"highlight-scout\"\n";

    fn no_file_holds_the_token(dir: &Path) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let text = std::fs::read_to_string(e.path()).unwrap_or_default();
            assert!(!text.contains("FAKE-TOKEN-FOR-TEST"), "{} holds the token", e.path().display());
        }
    }

    #[test]
    fn config_toml_migrates_without_copying_or_deleting_it() {
        let dir = tmp("migrate");
        std::fs::write(dir.join("config.toml"), OLD).unwrap();
        let store = open_store(&dir, Actor::You, None);
        let keys = MemoryStore::default();
        let r = migrate(&dir, &store, &keys);
        assert_eq!(r, MigrationReport { settings_migrated: true, cursors_migrated: true, key_moved: Some(true), ..Default::default() });
        // the old file stays, only its token emptied; nothing else in it changed
        let old = std::fs::read_to_string(dir.join("config.toml")).unwrap();
        assert_eq!(old, OLD.replace("FAKE-TOKEN-FOR-TEST", ""));
        let names: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
        assert!(!names.iter().any(|n| n.contains("config.toml") && n != "config.toml"), "no copy of the old file: {names:?}");
        assert!(!names.iter().any(|n| n.ends_with(".tmp")), "no temporary file left: {names:?}");
        no_file_holds_the_token(&dir);
        assert_eq!(keys.get(READWISE_SLOT).unwrap().as_deref(), Some("FAKE-TOKEN-FOR-TEST"));
        let st = store.view();
        // only values that differ from the defaults are logged, as app updates, in one batch
        let logged: Vec<&str> = st.changes.iter().filter_map(|e| e.key()).collect();
        assert_eq!(logged.len(), 3, "{logged:?}");
        assert!(logged.contains(&"sources.archiveFolder") && logged.contains(&"search.resultLimit") && logged.contains(&"sync.onLaunch"));
        assert!(st.changes.iter().all(|e| e.who == app_settings::Who::AppUpdate(VERSION.into()) && e.batch.is_some()));
        let cfg = runtime_config(&st, store.schema(), &keys, &dir);
        assert_eq!(cfg.result_limit, 120);
        assert_eq!(cfg.readwise_last_sync, "2026-10-01T00:00:00Z");
        assert_eq!(cfg.shortcut, "Cmd+Alt+Shift+H");
        assert_eq!(cfg.readwise_api_key, "FAKE-TOKEN-FOR-TEST");
        // a second start changes nothing, and does not rewrite the stamp
        let stamp = std::fs::read_to_string(dir.join(MIGRATION_STAMP)).unwrap();
        assert_eq!(migrate(&dir, &store, &keys), MigrationReport::default());
        assert_eq!(store.view().changes.len(), 3);
        assert_eq!(std::fs::read_to_string(dir.join(MIGRATION_STAMP)).unwrap(), stamp);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_save_by_the_old_version_after_a_downgrade_migrates_again() {
        let dir = tmp("downgrade");
        std::fs::write(dir.join("config.toml"), OLD).unwrap();
        let store = open_store(&dir, Actor::You, None);
        let keys = MemoryStore::default();
        migrate(&dir, &store, &keys);
        // the old version runs, the user changes two settings there and it saves its whole file (with the token again)
        let saved = OLD.replace("result_limit = 120", "result_limit = 200").replace("sync_on_launch = false", "sync_on_launch = true").replace("2026-10-01T00:00:00Z", "2026-10-05T00:00:00Z");
        old_version_saves(&dir, &saved);
        let r = migrate(&dir, &store, &keys);
        assert!(r.settings_migrated && r.cursors_migrated && r.kept.is_empty());
        let st = store.view();
        let cfg = runtime_config(&st, store.schema(), &keys, &dir);
        assert_eq!((cfg.result_limit, cfg.sync_on_launch), (200, true));
        assert_eq!(cfg.readwise_last_sync, "2026-10-05T00:00:00Z");
        assert_eq!(st.changes.iter().filter(|e| e.batch == st.changes[0].batch).count(), 2, "one entry per setting the old version changed");
        no_file_holds_the_token(&dir);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A: first start of the new version. B: the user changes a setting in the new version. C: a downgrade; the old version saves
    /// its file (written after B) with that setting and another one changed. The new version keeps B's value and takes the other.
    #[test]
    fn a_change_made_in_the_new_version_wins_over_a_later_save_by_the_old_one() {
        let dir = tmp("abc");
        std::fs::write(dir.join("config.toml"), OLD).unwrap();
        let store = open_store(&dir, Actor::You, None);
        let keys = MemoryStore::default();
        migrate(&dir, &store, &keys); // A
        std::thread::sleep(std::time::Duration::from_millis(5));
        store.set("search.resultLimit", &json!(150)).unwrap(); // B
        let saved = OLD.replace("result_limit = 120", "result_limit = 200").replace("sync_on_launch = false", "sync_on_launch = true");
        old_version_saves(&dir, &saved); // C
        let r = migrate(&dir, &store, &keys);
        assert_eq!(r.kept, vec!["search.resultLimit".to_string()]);
        let cfg = runtime_config(&store.view(), store.schema(), &keys, &dir);
        assert_eq!((cfg.result_limit, cfg.sync_on_launch), (150, true));
        // decided once: the next start leaves both as they are
        assert_eq!(migrate(&dir, &store, &keys), MigrationReport::default());
        assert_eq!(runtime_config(&store.view(), store.schema(), &keys, &dir).result_limit, 150);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_write_that_does_not_land_is_not_stamped_and_is_tried_again() {
        let dir = tmp("unlanded");
        std::fs::write(dir.join("config.toml"), OLD).unwrap();
        let keys = MemoryStore::default();
        {
            let store = open_store(&dir, Actor::You, None);
            store.touch().unwrap();
            store.refuse_writes(Some("test: closed"));
            let r = migrate_legacy(&dir, &store, &keys, &held);
            let r = r.unwrap_or_else(|_| MigrationReport { failed: vec!["x".into()], ..Default::default() });
            assert!(!r.failed.is_empty() && !r.settings_migrated);
        }
        let stamped = std::fs::read_to_string(dir.join(MIGRATION_STAMP)).unwrap_or_default();
        assert!(!stamped.contains("search.resultLimit\": 120"), "a write that did not land was stamped");
        let store = open_store(&dir, Actor::You, None);
        let r = migrate(&dir, &store, &keys);
        assert!(r.settings_migrated);
        assert_eq!(runtime_config(&store.view(), store.schema(), &keys, &dir).result_limit, 120);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_cli_never_writes_sync_state_while_the_app_runs() {
        let dir = tmp("cursors");
        std::fs::write(dir.join("config.toml"), OLD).unwrap();
        let store = open_store(&dir, Actor::You, None);
        let keys = MemoryStore::default();
        let r = migrate_legacy(&dir, &store, &keys, &busy).unwrap();
        assert!(r.settings_migrated && !r.cursors_migrated);
        assert!(!dir.join(config::CURSORS_FILE).exists());
        // the app runs and syncs (its cursor is newer); then a CLI run with the app closed must not put the old cursor back
        std::thread::sleep(std::time::Duration::from_millis(20));
        config::save_cursors(&dir, &config::Cursors { readwise: "2026-10-09T00:00:00Z".into(), ..Default::default() }).unwrap();
        let r = migrate_legacy(&dir, &store, &keys, &held).unwrap();
        assert!(!r.cursors_migrated);
        assert_eq!(config::load_cursors(&dir).readwise, "2026-10-09T00:00:00Z");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Sol r2 New 2: the old cursor (OLD) in config.toml, a newer one (NEW) in sync-state.json, the lock taken (cursors wait) while the
    /// token is moved and emptied, which rewrites config.toml. That rewrite must not make OLD look newer: the next migration keeps NEW.
    #[test]
    fn emptying_the_token_never_makes_an_old_cursor_look_newer() {
        let dir = tmp("cursor-after-token");
        std::fs::write(dir.join("config.toml"), OLD).unwrap();
        config::save_cursors(&dir, &config::Cursors { readwise: "2026-10-09T00:00:00Z".into(), ..Default::default() }).unwrap();
        let store = open_store(&dir, Actor::You, None);
        let keys = MemoryStore::default();
        let r = migrate_legacy(&dir, &store, &keys, &busy).unwrap();
        assert!(!r.cursors_migrated && r.key_moved == Some(true));
        assert!(!std::fs::read_to_string(dir.join("config.toml")).unwrap().contains("FAKE-TOKEN-FOR-TEST"), "the token was emptied");
        std::thread::sleep(std::time::Duration::from_millis(20));
        let r = migrate_legacy(&dir, &store, &keys, &held).unwrap();
        assert!(!r.cursors_migrated);
        assert_eq!(config::load_cursors(&dir).readwise, "2026-10-09T00:00:00Z");
        // decided: a third start leaves it too
        migrate_legacy(&dir, &store, &keys, &held).unwrap();
        assert_eq!(config::load_cursors(&dir).readwise, "2026-10-09T00:00:00Z");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cursor_freshness_compares_instants() {
        assert!(later_cursor("2026-10-05T00:00:00Z", "2026-10-01T00:00:00+00:00"));
        assert!(!later_cursor("2026-10-01T00:00:00Z", "2026-10-01T00:00:00+00:00"));
        assert!(!later_cursor("2026-10-01T00:00:00Z", "2026-10-09T00:00:00Z"));
        assert!(later_cursor("2026-10-01T00:00:00Z", ""));
        assert!(!later_cursor("not a time", ""));
    }

    /// A Keychain whose save runs `during` first (what another process does while the Keychain is busy).
    struct During<F: Fn() + Send + Sync>(MemoryStore, F);
    impl<F: Fn() + Send + Sync> SecretStore for During<F> {
        fn has(&self, s: &str) -> Result<bool, String> {
            self.0.has(s)
        }
        fn set(&self, s: &str, v: &str) -> Result<(), String> {
            (self.1)();
            self.0.set(s, v)
        }
        fn remove(&self, s: &str) -> Result<(), String> {
            self.0.remove(s)
        }
        fn get(&self, s: &str) -> Result<Option<String>, String> {
            self.0.get(s)
        }
    }

    #[test]
    fn a_folder_swapped_for_a_link_during_the_keychain_step_is_never_written_through() {
        let base = tmp("swap");
        let dir = base.join("app");
        let elsewhere = base.join("elsewhere");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::create_dir_all(&elsewhere).unwrap();
        std::fs::write(dir.join("config.toml"), OLD).unwrap();
        let victim = "readwise_api_key = \"FAKE-TOKEN-FOR-TEST\"\nunrelated = 1\n";
        std::fs::write(elsewhere.join("config.toml"), victim).unwrap();
        let store = open_store(&dir, Actor::You, None);
        let (d, e, moved) = (dir.clone(), elsewhere.clone(), base.join("moved"));
        let keys = During(MemoryStore::default(), move || {
            if !moved.exists() {
                std::fs::rename(&d, &moved).unwrap();
                std::os::unix::fs::symlink(&e, &d).unwrap();
            }
        });
        let r = migrate_legacy(&dir, &store, &keys, &held);
        assert!(r.is_err(), "{r:?}");
        assert_eq!(std::fs::read_to_string(elsewhere.join("config.toml")).unwrap(), victim, "the link's target was written");
        assert!(!elsewhere.join(MIGRATION_STAMP).exists());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn emptying_the_token_keeps_an_edit_made_during_the_keychain_step() {
        let dir = tmp("intervening");
        std::fs::write(dir.join("config.toml"), OLD).unwrap();
        let store = open_store(&dir, Actor::You, None);
        let d = dir.clone();
        let keys = During(MemoryStore::default(), move || {
            let t = std::fs::read_to_string(d.join("config.toml")).unwrap();
            if t.contains("result_limit = 120") {
                std::thread::sleep(std::time::Duration::from_millis(20));
                std::fs::write(d.join("config.toml"), t.replace("result_limit = 120", "result_limit = 200")).unwrap();
            }
        });
        let r = migrate_legacy(&dir, &store, &keys, &held).unwrap();
        assert_eq!(r.key_moved, Some(true));
        let old = std::fs::read_to_string(dir.join("config.toml")).unwrap();
        assert!(old.contains("result_limit = 200"), "the edit was overwritten: {old}");
        assert!(!old.contains("FAKE-TOKEN-FOR-TEST"));
        // the edit is the old version's: the next start takes it
        assert!(migrate(&dir, &store, &keys.0).settings_migrated);
        assert_eq!(runtime_config(&store.view(), store.schema(), &keys.0, &dir).result_limit, 200);
        let _ = std::fs::remove_dir_all(&dir);
    }

    struct Refusing;
    impl SecretStore for Refusing {
        fn has(&self, _: &str) -> Result<bool, String> {
            Ok(false)
        }
        fn set(&self, _: &str, _: &str) -> Result<(), String> {
            Err("Keychain: the key did not read back after saving".into())
        }
        fn remove(&self, _: &str) -> Result<(), String> {
            Ok(())
        }
        fn get(&self, _: &str) -> Result<Option<String>, String> {
            Ok(None)
        }
    }

    #[test]
    fn the_old_file_stays_when_the_keychain_does_not_read_back() {
        let dir = tmp("keychain-refuses");
        std::fs::write(dir.join("config.toml"), OLD).unwrap();
        let store = open_store(&dir, Actor::You, None);
        let r = migrate(&dir, &store, &Refusing);
        assert_eq!(r.key_moved, Some(false));
        assert_eq!(std::fs::read_to_string(dir.join("config.toml")).unwrap(), OLD, "untouched: it still holds the token");
        assert!(!std::fs::read_to_string(dir.join("settings.json")).unwrap().contains("FAKE-TOKEN-FOR-TEST"));
        assert!(dir.join("settings.json").exists());
        assert_eq!(runtime_config(&store.view(), store.schema(), &Refusing, &dir).readwise_api_key, "FAKE-TOKEN-FOR-TEST");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_fresh_install_writes_defaults_with_no_entries() {
        let dir = tmp("fresh");
        let store = open_store(&dir, Actor::You, None);
        migrate(&dir, &store, &MemoryStore::default());
        assert!(dir.join("settings.json").exists());
        assert!(store.view().changes.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
