//! While the Settings window records a new shortcut, the app menu's accelerators are suspended, so the key press reaches the recorder
//! and never runs a menu command. Only the Settings window on its own page may suspend (the same check as the Keychain commands); the
//! accelerators come back when recording ends, on blur, or when the Settings window closes. Menu clicks are never held back.
//!
//! The menu is reached through `AccelMenu`, so the state machine is tested with a fake menu (no window, no main thread).

use std::sync::Mutex;

/// The menu items whose id is a registry command id, and a way to set each one's accelerator.
pub trait AccelMenu {
    fn command_items(&self) -> Vec<String>;
    fn set_accelerator(&self, id: &str, accelerator: Option<&str>) -> Result<(), String>;
}

#[derive(Default)]
pub struct MenuSuspend {
    suspended: Mutex<bool>,
}

impl MenuSuspend {
    pub fn is_suspended(&self) -> bool {
        *self.suspended.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Takes every command item's accelerator away (the items stay, and still run on a click).
    pub fn suspend(&self, menu: &dyn AccelMenu) -> Result<(), String> {
        let mut s = self.suspended.lock().unwrap_or_else(|e| e.into_inner());
        *s = true;
        for id in menu.command_items() {
            menu.set_accelerator(&id, None)?;
        }
        Ok(())
    }

    /// Puts back each item's accelerator from the effective bindings as they are now (a shortcut recorded meanwhile counts). Does
    /// nothing when nothing is suspended, so a menu the window rebuilt is left alone.
    pub fn restore(&self, menu: &dyn AccelMenu, accelerator_of: &dyn Fn(&str) -> Option<String>) -> Result<(), String> {
        let mut s = self.suspended.lock().unwrap_or_else(|e| e.into_inner());
        if !*s {
            return Ok(());
        }
        for id in menu.command_items() {
            menu.set_accelerator(&id, accelerator_of(&id).as_deref())?;
        }
        *s = false;
        Ok(())
    }
}

impl MenuSuspend {
    /// A menu was just installed or rebuilt: while the flag is set it comes up with its accelerators suspended as well. Called by the
    /// host at the point it sets the app menu (`menu_install`), so no front-end path can skip it.
    pub fn after_install(&self, menu: &dyn AccelMenu) -> Result<(), String> {
        let s = self.suspended.lock().unwrap_or_else(|e| e.into_inner());
        if *s {
            for id in menu.command_items() {
                menu.set_accelerator(&id, None)?;
            }
        }
        Ok(())
    }
}

pub static MENU: std::sync::LazyLock<MenuSuspend> = std::sync::LazyLock::new(MenuSuspend::default);

/// The accelerator each command shows now: the registry's effective bindings over the stored overrides.
pub fn accelerators(state: &app_settings::SettingsState, registry: &app_settings::Registry) -> std::collections::HashMap<String, String> {
    use app_settings::CommandsAdapter;
    let overrides: Vec<(String, serde_json::Value)> = state.keys.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    registry
        .resolve(&overrides)
        .bindings
        .into_iter()
        .filter_map(|(id, chord)| app_settings::commands::to_accelerator(&chord).map(|a| (id, a)))
        .collect()
}

/// The app menu as Tauri holds it (built by the main window with item ids = command ids).
pub struct TauriMenu<R: tauri::Runtime> {
    pub menu: tauri::menu::Menu<R>,
    pub registry: std::sync::Arc<app_settings::Registry>,
}

impl<R: tauri::Runtime> TauriMenu<R> {
    fn items(&self) -> Vec<tauri::menu::MenuItem<R>> {
        fn walk<R: tauri::Runtime>(kinds: Vec<tauri::menu::MenuItemKind<R>>, out: &mut Vec<tauri::menu::MenuItem<R>>) {
            for k in kinds {
                if let Some(i) = k.as_menuitem() {
                    out.push(i.clone());
                } else if let Some(s) = k.as_submenu() {
                    walk(s.items().unwrap_or_default(), out);
                }
            }
        }
        let mut out = Vec::new();
        walk(self.menu.items().unwrap_or_default(), &mut out);
        out.retain(|i| self.registry.by_id(&i.id().0).is_some());
        out
    }
}

impl<R: tauri::Runtime> AccelMenu for TauriMenu<R> {
    fn command_items(&self) -> Vec<String> {
        self.items().iter().map(|i| i.id().0.clone()).collect()
    }
    fn set_accelerator(&self, id: &str, accelerator: Option<&str>) -> Result<(), String> {
        for i in self.items().iter().filter(|i| i.id().0 == id) {
            i.set_accelerator(accelerator).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

/// Suspends the accelerators when the caller may (the Settings window on its own page); anyone else is refused and nothing changes.
pub fn suspend_if(may: bool, menu: &dyn AccelMenu, s: &MenuSuspend) -> Result<(), String> {
    if !may {
        return Err("refused".into());
    }
    s.suspend(menu)
}

/// A window closed: when it was the Settings window, the accelerators come back (it may have closed mid-recording).
pub fn on_closed(label: &str, menu: &dyn AccelMenu, s: &MenuSuspend, accelerator_of: &dyn Fn(&str) -> Option<String>) -> Result<(), String> {
    if label != crate::commands::app_settings::SETTINGS_LABEL {
        return Ok(());
    }
    s.restore(menu, accelerator_of)
}

/// The live app menu and the effective accelerators, for the commands and the close hook.
pub fn restore_app_menu<R: tauri::Runtime>(app: &tauri::AppHandle<R>, label: Option<&str>) {
    use tauri::Manager;
    let Some(menu) = app.menu() else { return };
    let registry = crate::settings_host::registry();
    let acc = match app.try_state::<crate::AppState>() {
        Some(st) => accelerators(&st.settings.view(), &registry),
        None => Default::default(),
    };
    let m = TauriMenu { menu, registry };
    let of = |id: &str| acc.get(id).cloned();
    let r = match label {
        Some(l) => on_closed(l, &m, &MENU, &of),
        None => MENU.restore(&m, &of),
    };
    if let Err(e) = r {
        eprintln!("Menu: the shortcuts could not be restored: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// A menu of command items and their accelerators.
    pub struct Fake(pub Mutex<HashMap<String, Option<String>>>);
    impl AccelMenu for Fake {
        fn command_items(&self) -> Vec<String> {
            let mut v: Vec<String> = self.0.lock().unwrap().keys().cloned().collect();
            v.sort();
            v
        }
        fn set_accelerator(&self, id: &str, a: Option<&str>) -> Result<(), String> {
            self.0.lock().unwrap().insert(id.into(), a.map(str::to_string));
            Ok(())
        }
    }
    pub fn fake() -> Fake {
        Fake(Mutex::new(HashMap::from([
            ("cycle-sort".to_string(), Some("Cmd+Shift+S".to_string())),
            ("copy-image".to_string(), None),
            ("settings".to_string(), Some("Cmd+,".to_string())),
        ])))
    }

    #[test]
    fn suspend_then_restore_round_trips_the_menu() {
        let m = fake();
        let before = m.0.lock().unwrap().clone();
        let s = MenuSuspend::default();
        s.suspend(&m).unwrap();
        assert!(s.is_suspended());
        assert!(m.0.lock().unwrap().values().all(Option::is_none), "every accelerator is taken away");
        let acc = |id: &str| before.get(id).cloned().flatten();
        s.restore(&m, &acc).unwrap();
        assert!(!s.is_suspended());
        assert_eq!(*m.0.lock().unwrap(), before);
        // a second restore (nothing suspended) leaves a rebuilt menu alone
        m.0.lock().unwrap().insert("cycle-sort".into(), Some("Cmd+Alt+S".into()));
        s.restore(&m, &acc).unwrap();
        assert_eq!(m.0.lock().unwrap()["cycle-sort"].as_deref(), Some("Cmd+Alt+S"));
    }

    #[test]
    fn a_menu_rebuilt_while_recording_comes_up_suspended() {
        let s = MenuSuspend::default();
        let rebuilt = fake();
        s.after_install(&rebuilt).unwrap();
        assert_eq!(rebuilt.0.lock().unwrap()["cycle-sort"].as_deref(), Some("Cmd+Shift+S"), "not recording: left as built");
        s.suspend(&fake()).unwrap();
        let rebuilt = fake(); // the window rebuilt the menu (a chord changed) with accelerators
        s.after_install(&rebuilt).unwrap();
        assert!(rebuilt.0.lock().unwrap().values().all(Option::is_none));
        assert!(s.is_suspended());
        let acc = |id: &str| fake().0.lock().unwrap().get(id).cloned().flatten();
        s.restore(&rebuilt, &acc).unwrap();
        assert_eq!(rebuilt.0.lock().unwrap()["cycle-sort"].as_deref(), Some("Cmd+Shift+S"));
    }

    #[test]
    fn a_window_other_than_settings_cannot_suspend() {
        let u = |s: &str| tauri::Url::parse(s).unwrap();
        let m = fake();
        let s = MenuSuspend::default();
        for (label, url) in [("main", "tauri://localhost/index.html"), ("work-1", "tauri://localhost/settings.html"), ("settings", "https://evil.example/settings.html")] {
            let may = crate::commands::app_settings::is_settings_page(label, &u(url));
            assert!(suspend_if(may, &m, &s).is_err(), "{label} {url}");
            assert!(!s.is_suspended());
            assert_eq!(m.0.lock().unwrap()["cycle-sort"].as_deref(), Some("Cmd+Shift+S"));
        }
        let may = crate::commands::app_settings::is_settings_page("settings", &u("tauri://localhost/settings.html"));
        suspend_if(may, &m, &s).unwrap();
        assert!(s.is_suspended());
    }

    #[test]
    fn closing_the_settings_window_mid_recording_restores_the_shortcuts() {
        let m = fake();
        let before = m.0.lock().unwrap().clone();
        let acc = |id: &str| before.get(id).cloned().flatten();
        let s = MenuSuspend::default();
        s.suspend(&m).unwrap();
        on_closed("work-1", &m, &s, &acc).unwrap();
        assert!(s.is_suspended(), "another window closing changes nothing");
        on_closed("settings", &m, &s, &acc).unwrap();
        assert!(!s.is_suspended());
        assert_eq!(*m.0.lock().unwrap(), before);
    }

    #[test]
    fn effective_accelerators_follow_the_stored_overrides() {
        let reg = crate::settings_host::registry();
        let mut st = app_settings::SettingsState::empty("highlight-scout");
        assert_eq!(accelerators(&st, &reg).get("cycle-sort").map(String::as_str), Some("Cmd+Shift+S"));
        st.keys.insert("cycle-sort".into(), serde_json::json!("⌘⌥S"));
        let a = accelerators(&st, &reg);
        assert_eq!(a.get("cycle-sort").map(String::as_str), Some("Cmd+Alt+S"));
        assert_eq!(a.get("settings").map(String::as_str), Some("Cmd+,"));
        assert!(!a.contains_key("copy-image"));
    }
}
