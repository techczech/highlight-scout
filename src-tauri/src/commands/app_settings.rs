//! The Settings window's host contract over Tauri commands (what `@bridle/settings-electron` gives an Electron app over IPC): the view,
//! every store operation, export and import through native panels opened here (the renderer never names a file), and the Keychain
//! slots. A key's value comes in once (`settings_set_secret`) and never goes out.
//!
//! Caller rule (checked here, in Rust): changing or removing a key, and previewing or applying an import, are answered only for the
//! webview labelled `settings` showing the app's own `settings.html`; any other window gets a refusal, never a result.

use crate::settings_host::{self, SLOTS};
use crate::AppState;
use app_settings::{slot_statuses, ImportPreview, OpResult, Target};
use serde_json::{json, Value};
use tauri::Manager;

fn outcome(r: OpResult) -> Value {
    match r {
        Ok(c) => {
            let mut v = json!({ "ok": true });
            if let Some(b) = c.batch {
                v["batch"] = json!(b);
            }
            if !c.skipped.is_empty() {
                v["skipped"] = Value::Array(c.skipped.iter().map(|s| s.to_json()).collect());
            }
            v
        }
        Err(e) => json!({ "ok": false, "code": e.code, "error": e.error }),
    }
}

/// What the window draws: the state (format v2, as the file), the key slots (status only), the running build.
pub fn view_json(state: &AppState) -> Value {
    let st = state.settings.view();
    let sec = settings_host::secrets();
    let slots: Vec<Value> = slot_statuses(sec.as_ref(), &SLOTS)
        .into_iter()
        .map(|s| {
            let mut v = json!({ "id": s.id, "label": s.label, "status": s.status, "group": if s.id == settings_host::READWISE_SLOT { "Readwise" } else { "Cloudflare R2 backup" } });
            if let Some(d) = s.detail {
                v["detail"] = json!(d);
            }
            v
        })
        .collect();
    let mut v = json!({
        "state": st.to_json(),
        "slots": slots,
        "appInfo": { "name": settings_host::APP_NAME, "version": settings_host::VERSION, "icon": "highlighter", "cli": "highlight-scout" },
    });
    if let Some(p) = state.settings.problem() {
        v["problem"] = json!({ "code": p.code, "error": p.error });
    }
    v
}

#[tauri::command]
pub fn settings_view(state: tauri::State<'_, AppState>) -> Value {
    view_json(&state)
}

#[tauri::command]
pub fn settings_set(key: String, value: Value, state: tauri::State<'_, AppState>) -> Value {
    outcome(state.settings.set(&key, &value))
}

#[tauri::command]
pub fn settings_rebind(command_id: String, chord: String, state: tauri::State<'_, AppState>) -> Value {
    outcome(state.settings.rebind(&command_id, &chord))
}

#[tauri::command]
pub fn settings_rebind_many(pairs: Vec<(String, String)>, state: tauri::State<'_, AppState>) -> Value {
    let p: Vec<(String, Value)> = pairs.into_iter().map(|(a, b)| (a, Value::String(b))).collect();
    outcome(state.settings.rebind_many(&p))
}

/// `target` is a setting key (string) or `{ commandId }`.
#[tauri::command]
pub fn settings_reset(target: Value, state: tauri::State<'_, AppState>) -> Value {
    let t = match (&target, target.get("commandId").and_then(Value::as_str)) {
        (Value::String(k), _) => Target::Key(k.clone()),
        (_, Some(c)) => Target::Command(c.to_string()),
        _ => return json!({ "ok": false, "code": "unknown-key", "error": "Nothing to reset" }),
    };
    outcome(state.settings.reset(&t))
}

#[tauri::command]
pub fn settings_reset_section(section: String, state: tauri::State<'_, AppState>) -> Value {
    outcome(state.settings.reset_section(&section))
}

#[tauri::command]
pub fn settings_reset_all(state: tauri::State<'_, AppState>) -> Value {
    outcome(state.settings.reset_all())
}

#[tauri::command]
pub fn settings_reset_entry(entry_id: String, state: tauri::State<'_, AppState>) -> Value {
    outcome(state.settings.reset_entry(&entry_id))
}

#[tauri::command]
pub fn settings_undo_batch(batch_id: String, state: tauri::State<'_, AppState>) -> Value {
    outcome(state.settings.undo_batch(&batch_id))
}

/// The Settings window's label and page.
pub const SETTINGS_LABEL: &str = "settings";
pub const SETTINGS_PAGE: &str = "/settings.html";
pub const MSG_NOT_SETTINGS: &str = "This can only be done in the Settings window";
pub const MSG_IMPORT_AGAIN: &str = "Choose the file again before importing it";
pub const MSG_EXPORT_UNWRITTEN: &str = "The settings file could not be written there";

/// True for the Settings window showing the app's own Settings page (the bundled `tauri://localhost`, or the dev server in a debug
/// build). Whatever the label, any other page, origin or window is refused.
pub fn is_settings_page(label: &str, url: &tauri::Url) -> bool {
    if label != SETTINGS_LABEL || url.path() != SETTINGS_PAGE {
        return false;
    }
    let host = url.host_str().unwrap_or("");
    match url.scheme() {
        "tauri" => host == "localhost",
        "http" | "https" if host == "tauri.localhost" => true,
        "http" => cfg!(debug_assertions) && (host == "localhost" || host == "127.0.0.1") && url.port() == Some(1420),
        _ => false,
    }
}

fn from_settings<R: tauri::Runtime>(w: &tauri::Webview<R>) -> bool {
    w.url().is_ok_and(|u| is_settings_page(w.label(), &u))
}

fn refused() -> Value {
    json!({ "ok": false, "error": MSG_NOT_SETTINGS })
}

/// Writes the export text to `path` (a file the user chose in the save panel, which asked before replacing one): a link is refused,
/// never followed; anything but a plain file is refused; the file is private (0600).
pub fn write_export(path: &std::path::Path, text: &str) -> Result<(), String> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| MSG_EXPORT_UNWRITTEN.to_string())?;
    if !f.metadata().is_ok_and(|m| m.is_file()) {
        return Err(MSG_EXPORT_UNWRITTEN.into());
    }
    f.set_len(0).and_then(|_| f.write_all(text.as_bytes())).and_then(|_| f.sync_all()).map_err(|_| MSG_EXPORT_UNWRITTEN.to_string())
}

/// Export Settings…: the native save panel (opened here; the destination never comes from a renderer), then the export file. Never a
/// key. `{ ok: true }` also when the panel was cancelled.
#[tauri::command]
pub async fn settings_export(app: tauri::AppHandle, state: tauri::State<'_, AppState>) -> Result<Value, String> {
    use tauri_plugin_dialog::DialogExt;
    let name = format!("Highlight Scout settings {}.json", chrono::Local::now().format("%Y-%m-%d"));
    let mut panel = app.dialog().file().set_title("Export Settings").set_file_name(&name).add_filter("Settings", &["json"]);
    if let Some(d) = dirs::download_dir() {
        panel = panel.set_directory(d);
    }
    let Some(path) = panel.blocking_save_file().and_then(|p| p.into_path().ok()) else { return Ok(json!({ "ok": true })) };
    let file = state.settings.export_file(&computer_name());
    Ok(match write_export(&path, &app_settings::value::pretty(&file)) {
        Ok(()) => json!({ "ok": true }),
        Err(e) => json!({ "ok": false, "error": e }),
    })
}

/// The file the window last previewed, kept here: apply reads it again and previews it again; the renderer only names the preview.
#[derive(Debug, Clone)]
pub struct PendingImport {
    pub id: String,
    pub path: std::path::PathBuf,
    pub file_name: String,
    pub preview: Value,
}

static PENDING: std::sync::Mutex<Option<PendingImport>> = std::sync::Mutex::new(None);

/// The preview of the file at `path` against the store as it is now (a file that cannot be read previews as refused).
pub fn preview_path(store: &app_settings::SettingsStore, path: &std::path::Path) -> ImportPreview {
    match app_settings::file::read_bounded(path, app_settings::file::MAX_FILE_BYTES) {
        app_settings::file::Bounded::Ok(text) => store.preview_import(&text),
        _ => app_settings::transfer::preview_import(&store.view(), "", store.schema(), store.commands()),
    }
}

/// Starts an import from `path`: previews it, keeps it as the one pending import, and returns what the window shows.
pub fn begin_import(store: &app_settings::SettingsStore, path: &std::path::Path) -> Value {
    let file_name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let preview = preview_path(store, path).to_json();
    let id = app_settings::value::uuid();
    *PENDING.lock().unwrap_or_else(|e| e.into_inner()) = Some(PendingImport { id: id.clone(), path: path.to_path_buf(), file_name: file_name.clone(), preview: preview.clone() });
    json!({ "previewId": id, "fileName": file_name, "preview": preview })
}

/// Applies the pending import named `preview_id` (once): the file is read and previewed again here, and applied only when that preview
/// is the one the window showed (the file and the settings are as they were).
pub fn finish_import(store: &app_settings::SettingsStore, preview_id: &str, file_name: &str) -> Value {
    let p = PENDING.lock().unwrap_or_else(|e| e.into_inner()).take();
    let Some(p) = p.filter(|p| p.id == preview_id && p.file_name == file_name) else { return json!({ "ok": false, "error": MSG_IMPORT_AGAIN }) };
    // Accepted residual (Sol r2 New 3): the preview is compared with the store's cached view; a write by another store between this
    // check and the locked apply (the same user, at the same moment) can be replaced by the imported value. Same-user race, not fixed.
    let fresh = preview_path(store, &p.path);
    if fresh.refused.is_some() || fresh.to_json() != p.preview {
        return json!({ "ok": false, "error": MSG_IMPORT_AGAIN });
    }
    outcome(store.apply_import(&fresh, &p.file_name))
}

/// Import Settings…: the native open panel (opened here), then `{ previewId, fileName, preview }` (the TS `ImportPreview` shape), or
/// null when the panel was cancelled.
#[tauri::command]
pub async fn settings_import_preview(webview: tauri::Webview, app: tauri::AppHandle, state: tauri::State<'_, AppState>) -> Result<Value, String> {
    use tauri_plugin_dialog::DialogExt;
    if !from_settings(&webview) {
        return Ok(refused());
    }
    let picked = app.dialog().file().set_title("Import Settings").add_filter("Settings", &["json"]).blocking_pick_file();
    let Some(path) = picked.and_then(|p| p.into_path().ok()) else { return Ok(Value::Null) };
    Ok(begin_import(&state.settings, &path))
}

#[tauri::command]
pub fn settings_import_apply(webview: tauri::Webview, preview_id: String, file_name: String, state: tauri::State<'_, AppState>) -> Value {
    if !from_settings(&webview) {
        return refused();
    }
    finish_import(&state.settings, &preview_id, &file_name)
}

/// Saves a key in the Keychain (read back before it counts) and logs connected/missing, never the value. Settings window only.
#[tauri::command]
pub fn settings_set_secret(webview: tauri::Webview, slot_id: String, value: String, state: tauri::State<'_, AppState>) -> Value {
    if !from_settings(&webview) {
        return refused();
    }
    if !SLOTS.iter().any(|(id, _)| *id == slot_id) {
        return json!({ "ok": false, "error": "There is no such key" });
    }
    let sec = settings_host::secrets();
    let before = if sec.has(&slot_id).unwrap_or(false) { "connected" } else { "missing" };
    let value = value.trim().to_string();
    if value.is_empty() {
        return json!({ "ok": false, "error": "Paste the key first" });
    }
    if let Err(e) = sec.set(&slot_id, &value) {
        return json!({ "ok": false, "error": e });
    }
    drop(value);
    let r = state.settings.record_secret(&slot_id, before, "connected");
    crate::refresh_runtime_config(&state);
    outcome(r)
}

/// Suspends the app menu's accelerators while the Settings window records a shortcut (the key press is for the recorder). Settings
/// window on its own page only; no event is broadcast.
#[tauri::command]
pub fn menu_shortcuts_suspend(webview: tauri::Webview) -> Value {
    let may = from_settings(&webview);
    if !may {
        return refused();
    }
    let Some(menu) = webview.app_handle().menu() else { return json!({ "ok": true }) };
    let m = crate::menu_suspend::TauriMenu { menu, registry: settings_host::registry() };
    match crate::menu_suspend::suspend_if(may, &m, &crate::menu_suspend::MENU) {
        Ok(()) => json!({ "ok": true }),
        Err(e) => json!({ "ok": false, "error": e }),
    }
}

/// Sets the menu the main window built (`rid` in its resource table) as the app menu, then re-applies a running suspension, so a
/// menu rebuilt while a shortcut is recorded never brings the accelerators back. The webviews may not call setAsAppMenu themselves
/// (denied in the capabilities); the main window only.
#[tauri::command]
pub fn menu_install(webview: tauri::Webview, rid: tauri::ResourceId) -> Result<(), String> {
    if webview.label() != "main" {
        return Err("Only the main window sets the app menu".into());
    }
    let menu = webview.resources_table().get::<tauri::menu::Menu<tauri::Wry>>(rid).map_err(|e| e.to_string())?;
    menu.set_as_app_menu().map_err(|e| e.to_string())?;
    let m = crate::menu_suspend::TauriMenu { menu: (*menu).clone(), registry: settings_host::registry() };
    crate::menu_suspend::MENU.after_install(&m)
}

/// Gives the menu its accelerators back (recording ended or the recorder lost focus). Settings window on its own page only.
#[tauri::command]
pub fn menu_shortcuts_restore(webview: tauri::Webview) -> Value {
    if !from_settings(&webview) {
        return refused();
    }
    crate::menu_suspend::restore_app_menu(webview.app_handle(), None);
    json!({ "ok": true })
}

/// Removes a key from the Keychain and logs it. Settings window only.
#[tauri::command]
pub fn settings_remove_secret(webview: tauri::Webview, slot_id: String, state: tauri::State<'_, AppState>) -> Value {
    if !from_settings(&webview) {
        return refused();
    }
    if !SLOTS.iter().any(|(id, _)| *id == slot_id) {
        return json!({ "ok": false, "error": "There is no such key" });
    }
    let sec = settings_host::secrets();
    let before = if sec.has(&slot_id).unwrap_or(false) { "connected" } else { "missing" };
    if let Err(e) = sec.remove(&slot_id) {
        return json!({ "ok": false, "error": e });
    }
    let r = state.settings.record_secret(&slot_id, before, "missing");
    crate::refresh_runtime_config(&state);
    outcome(r)
}

/// Values the window found in its browser storage from before shared settings (text size, shortcut overrides): logged as an app
/// update, once; values already set are left.
#[tauri::command]
pub fn settings_migrate_local(settings: serde_json::Map<String, Value>, keys: serde_json::Map<String, Value>, state: tauri::State<'_, AppState>) -> Value {
    outcome(state.settings.migrate_values(&app_settings::Migrated { settings, keys }, false))
}

/// The Mac's name as the user set it ("Dominik's MacBook Air"), for the export file; the host name when it cannot be read.
fn computer_name() -> String {
    std::process::Command::new("scutil")
        .args(["--get", "ComputerName"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(app_settings::lock::host_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use app_settings::Actor;

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("hs-app-settings-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d.canonicalize().unwrap()
    }

    #[test]
    fn only_the_settings_window_on_its_own_page_may_change_keys() {
        let u = |s: &str| tauri::Url::parse(s).unwrap();
        assert!(is_settings_page("settings", &u("tauri://localhost/settings.html")));
        assert!(is_settings_page("settings", &u("tauri://localhost/settings.html?section=accounts")));
        assert!(!is_settings_page("main", &u("tauri://localhost/settings.html")));
        assert!(!is_settings_page("settings", &u("tauri://localhost/index.html")));
        assert!(!is_settings_page("settings", &u("https://evil.example/settings.html")));
        assert!(!is_settings_page("settings", &u("http://localhost:8080/settings.html")));
        assert!(!is_settings_page("work-1", &u("tauri://localhost/settings.html")));
    }

    #[test]
    fn export_refuses_a_link_and_writes_a_private_file() {
        use std::os::unix::fs::PermissionsExt;
        let d = tmp("export");
        let target = d.join("victim.txt");
        std::fs::write(&target, "keep me").unwrap();
        std::os::unix::fs::symlink(&target, d.join("link.json")).unwrap();
        assert!(write_export(&d.join("link.json"), "{}\n").is_err());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "keep me");
        write_export(&d.join("out.json"), "{}\n").unwrap();
        assert_eq!(std::fs::read_to_string(d.join("out.json")).unwrap(), "{}\n");
        assert_eq!(std::fs::metadata(d.join("out.json")).unwrap().permissions().mode() & 0o777, 0o600);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// One test (the pending import is one per process).
    #[test]
    fn import_applies_only_the_preview_made_here_from_the_file_as_it_still_is() {
        let d = tmp("import");
        let store = settings_host::open_store(&d.join("cfg"), Actor::You, None);
        let file = d.join("hs.json");
        let text = |n: u32| json!({ "format": "bridle-settings-export", "version": 1, "app": "highlight-scout", "settings": { "search.resultLimit": n }, "keys": {} }).to_string();
        std::fs::write(&file, text(77)).unwrap();
        // a made-up id or another file name: refused, and the pending import is gone
        let shown = begin_import(&store, &file);
        assert_eq!(finish_import(&store, "made-up", "hs.json")["ok"], json!(false));
        assert_eq!(finish_import(&store, shown["previewId"].as_str().unwrap(), "hs.json")["error"], json!(MSG_IMPORT_AGAIN));
        // the file changed after the preview: refused
        let shown = begin_import(&store, &file);
        std::fs::write(&file, text(250)).unwrap();
        assert_eq!(finish_import(&store, shown["previewId"].as_str().unwrap(), "hs.json")["error"], json!(MSG_IMPORT_AGAIN));
        assert!(store.view().changes.is_empty());
        // the genuine one applies, once
        let shown = begin_import(&store, &file);
        let id = shown["previewId"].as_str().unwrap().to_string();
        assert_eq!(finish_import(&store, &id, "hs.json")["ok"], json!(true));
        assert_eq!(store.view().settings.get("search.resultLimit"), Some(&json!(250)));
        assert_eq!(finish_import(&store, &id, "hs.json")["ok"], json!(false));
        let _ = std::fs::remove_dir_all(&d);
    }
}
