//! `app-settings`: a Rust port of `@bridle/settings` (pure core + node layer) for Tauri apps, reading and writing the same files.
//!
//! - Settings file `<dir>/settings.json`, format v2 (`{ format: "bridle-settings", version: 2, app, settings, keys, changes }`).
//! - Export file, format v1 (`{ format: "bridle-settings-export", version: 1, app, appVersion, exportedFrom, exportedAt, settings, keys }`).
//! - The schema and command list are read from JSON emitted by the app's TS schema and registry, so one declaration serves the window
//!   (TS, `@bridle/settings-react`) and the store and CLI (here).
//!
//! Seams: [`SettingsStore`] (every operation, locked and logged), [`run_settings_cli`] (the agent CLI), [`SecretStore`] (account keys).

pub mod changes;
pub mod cli;
pub mod commands;
pub mod file;
pub mod load;
pub mod lock;
pub mod schema;
pub mod secrets;
pub mod state;
pub mod store;
pub mod transfer;
pub mod value;

pub use changes::{Actor, ChangeEntry, SkippedItem, Subject, Who};
pub use cli::{run_process, run_settings_cli, CliIo, CliOptions};
pub use commands::{CommandsAdapter, Registry};
pub use load::{Migrated, MigrateFn};
pub use schema::{Schema, SettingDef, SettingType};
pub use secrets::{slot_statuses, MemoryStore, SecretStore, SlotStatus};
#[cfg(feature = "keychain")]
pub use secrets::{KeychainStore, MappedKeychain};
pub use state::{Clock, Committed, OpError, OpResult, SettingsState, SystemClock, Target};
pub use store::{Problem, SettingsStore, StoreOptions};
pub use transfer::ImportPreview;
