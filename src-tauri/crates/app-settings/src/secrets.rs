//! Account keys (`secret` settings) live outside the settings file. `SecretSlots` is the host's view of them: status only, never a
//! value toward the window or the CLI. The Keychain implementation stores each slot as a generic password under one service name
//! (tests pass a throwaway service so they never touch a real item).

#[derive(Debug, Clone, PartialEq)]
pub struct SlotStatus {
    pub id: String,
    pub label: String,
    /// `connected`, `missing` or `problem`.
    pub status: &'static str,
    pub detail: Option<String>,
}

pub trait SecretStore: Send + Sync {
    /// Whether a value is saved (reads it back to answer; the value never leaves this call).
    fn has(&self, id: &str) -> Result<bool, String>;
    /// Saves a value and reads it back; Err unless the read-back equals what was written.
    fn set(&self, id: &str, value: &str) -> Result<(), String>;
    fn remove(&self, id: &str) -> Result<(), String>;
    /// The value, for the app's own use (a sync request); never for the window, the CLI, a log or an export.
    fn get(&self, id: &str) -> Result<Option<String>, String>;
}

/// The macOS Keychain (Security framework through the keyring crate): one generic password per slot, `service` / `id`.
#[cfg(feature = "keychain")]
pub struct KeychainStore {
    pub service: String,
}

#[cfg(feature = "keychain")]
impl KeychainStore {
    pub fn new(service: &str) -> Self {
        KeychainStore { service: service.into() }
    }
    fn entry(&self, id: &str) -> Result<keyring::Entry, String> {
        keyring::Entry::new(&self.service, id).map_err(|e| format!("Keychain: {}", short(&e)))
    }
}

#[cfg(feature = "keychain")]
fn short(e: &keyring::Error) -> &'static str {
    match e {
        keyring::Error::NoEntry => "no entry",
        keyring::Error::NoStorageAccess(_) => "no access to the Keychain",
        keyring::Error::PlatformFailure(_) => "the Keychain refused",
        keyring::Error::BadEncoding(_) => "the saved key is not text",
        _ => "the Keychain refused",
    }
}

#[cfg(feature = "keychain")]
impl SecretStore for KeychainStore {
    fn has(&self, id: &str) -> Result<bool, String> {
        Ok(self.get(id)?.is_some_and(|v| !v.is_empty()))
    }
    fn set(&self, id: &str, value: &str) -> Result<(), String> {
        let e = self.entry(id)?;
        e.set_password(value).map_err(|x| format!("Keychain: {}", short(&x)))?;
        match e.get_password() {
            Ok(back) if back == value => Ok(()),
            _ => Err("Keychain: the key did not read back after saving".into()),
        }
    }
    fn remove(&self, id: &str) -> Result<(), String> {
        match self.entry(id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(x) => Err(format!("Keychain: {}", short(&x))),
        }
    }
    fn get(&self, id: &str) -> Result<Option<String>, String> {
        match self.entry(id)?.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(x) => Err(format!("Keychain: {}", short(&x))),
        }
    }
}

/// Several Keychain services behind one `SecretStore`: each slot id names its own service and account (so an app's existing items,
/// written by other code under their own service, are slots too).
#[cfg(feature = "keychain")]
pub struct MappedKeychain {
    /// (slot id, service, account)
    pub slots: Vec<(String, String, String)>,
}

#[cfg(feature = "keychain")]
impl MappedKeychain {
    fn find(&self, id: &str) -> Result<(KeychainStore, String), String> {
        let (_, svc, acct) = self.slots.iter().find(|(s, _, _)| s == id).ok_or_else(|| format!("There is no key slot \"{id}\""))?;
        Ok((KeychainStore::new(svc), acct.clone()))
    }
}

#[cfg(feature = "keychain")]
impl SecretStore for MappedKeychain {
    fn has(&self, id: &str) -> Result<bool, String> {
        let (k, a) = self.find(id)?;
        k.has(&a)
    }
    fn set(&self, id: &str, value: &str) -> Result<(), String> {
        let (k, a) = self.find(id)?;
        k.set(&a, value)
    }
    fn remove(&self, id: &str) -> Result<(), String> {
        let (k, a) = self.find(id)?;
        k.remove(&a)
    }
    fn get(&self, id: &str) -> Result<Option<String>, String> {
        let (k, a) = self.find(id)?;
        k.get(&a)
    }
}

/// An in-memory store for tests.
#[derive(Default)]
pub struct MemoryStore(pub std::sync::Mutex<std::collections::HashMap<String, String>>);

impl SecretStore for MemoryStore {
    fn has(&self, id: &str) -> Result<bool, String> {
        Ok(self.0.lock().map_err(|_| "poisoned")?.get(id).is_some_and(|v| !v.is_empty()))
    }
    fn set(&self, id: &str, value: &str) -> Result<(), String> {
        self.0.lock().map_err(|_| "poisoned")?.insert(id.into(), value.into());
        Ok(())
    }
    fn remove(&self, id: &str) -> Result<(), String> {
        self.0.lock().map_err(|_| "poisoned")?.remove(id);
        Ok(())
    }
    fn get(&self, id: &str) -> Result<Option<String>, String> {
        Ok(self.0.lock().map_err(|_| "poisoned")?.get(id).cloned())
    }
}

/// Status of each slot for the window: connected / missing / problem, never a value.
pub fn slot_statuses(store: &dyn SecretStore, slots: &[(&str, &str)]) -> Vec<SlotStatus> {
    slots
        .iter()
        .map(|(id, label)| match store.has(id) {
            Ok(true) => SlotStatus { id: id.to_string(), label: label.to_string(), status: "connected", detail: None },
            Ok(false) => SlotStatus { id: id.to_string(), label: label.to_string(), status: "missing", detail: None },
            Err(e) => SlotStatus { id: id.to_string(), label: label.to_string(), status: "problem", detail: Some(e) },
        })
        .collect()
}
