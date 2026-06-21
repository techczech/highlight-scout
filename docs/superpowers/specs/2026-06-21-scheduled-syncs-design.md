# Scheduled syncs + built-in Readwise tweet import — design (v0.5.0)

## Goal

Make Highlight Scout a universal app for recurring imports, not just one-off file
picks. Two features:

1. A **built-in "Readwise saved tweets" importer** — any user with a Readwise
   account pulls their saved tweets (full text) from Readwise Reader, in-app. No
   local scripts.
2. A **scheduled-sync system** — users schedule recurring syncs per source
   (Readwise highlights, Readwise tweets, Zotero), with a registry so more
   sources can be added later.

Non-goals: OS-level scheduling, syncing one-off file imports (CSV/Kindle/JSON),
the birdclaw/X-API path (Dominik-local), new secret storage.

## Source-of-truth facts (current code)

- Importers conform to `parse → (works, highlights)` and persist via the shared
  `persist()` in `commands/import.rs` (ADR-0010). Existing: csv, kindle, json,
  readwise (v2 export + Reader v3 article fulltext), zotero, x (file).
- Async source commands already exist: `run_import` (Readwise, incremental via
  `readwise_last_sync`), `run_zotero_import`.
- App is a resident daemon: `tauri-plugin-autostart` (currently **force-enabled**
  in `lib.rs`), hide-on-close, global hotkey.
- `config.toml` holds `readwise_api_key`, `zotero_db_path`, `readwise_last_sync`,
  `import_reminder_days`, `result_limit`, etc. Settings flow: `commands/settings.rs`
  `get_settings`/`save_settings` ↔ `SettingsPanel.tsx`.
- A headless `--import-x` flag exists in `lib.rs`.

## 1. Syncable-source registry

A small `sync` module (`src-tauri/src/sync/mod.rs`):

```
enum SyncSourceId { ReadwiseHighlights, ReadwiseTweets, Zotero }
const SCHEDULABLE: [SyncSourceId; 3] = [...];   // registry the UI + scheduler iterate
fn label(id) -> &str
async fn run_source(id, &AppState, &Window) -> Result<ImportStatus, String>
fn last_sync(id, &Config) -> Option<DateTime>   // reads the per-source cursor
fn config_keys(id) -> (enabled_key, interval_key, cursor_key)
```

`run_source` dispatches to each source's headless sync (reusing existing importer
internals — `ReadwiseClient::import_export`, the new tweets importer, the Zotero
importer). Deliberately a small enum + dispatcher, not a trait framework. Adding a
future source = add an enum variant + a `run_source` branch + register in
`SCHEDULABLE`.

## 2. Readwise saved-tweets importer (new source)

New `import/readwise_tweets.rs`. Uses the Readwise **Reader v3** API:
`GET /api/v3/list/?category=tweet&withHtmlContent=true[&updatedAfter=…][&pageCursor=…]`,
paginated, honoring `Retry-After` on 429 (same retry pattern as `readwise.rs`).

Per document:
- `tweet_id` from `source_url` (`/status/(\d+)`); `author_handle` from `source_url`,
  `author_name` from `author`.
- Text from `html_content`: strip tags → clean text (preserve paragraph breaks).
- Images: `<img src>`/`source_url`s containing `/media/` only (exclude avatars).
- Article links: `<a href>` excluding twitter/x/t.co/pbs.
- Embed article links (🔗) and images (`![]`) into the highlight body, matching
  `import/x.rs` `body_with_context`.

Output: the **same tweet shape as `import/x.rs`** — `Work { work_type: "tweet",
source_system: "x", id: "x-w-{tweet_id}" }`, `Highlight { id: "x-{tweet_id}" }`.
This guarantees a tweet from Readwise and the same tweet from a birdclaw/X import
**upsert to one highlight** (dedupe by native tweet ID), per ADR-0013/0011.

Incremental: new `readwise_tweets_last_sync` cursor (set to sync start time on
success; passed as `updatedAfter` next run).

Manual entry point: a `import_readwise_tweets` Tauri command + an Import-menu item
("Readwise saved tweets") so it's usable on demand, not only scheduled.

## 3. In-app scheduler

Spawned in `lib.rs` `setup()` as a tokio task with an `AppHandle`:

- Ticks every 5 minutes (`tokio::time::interval`).
- Each tick, for each `SCHEDULABLE` source: if `enabled` and
  `now − last_sync ≥ interval_hours`, run it via `run_source`.
- **Sequential** (one source at a time) and guarded by a shared `is_syncing`
  flag/mutex in `AppState` so a scheduled run never overlaps a manual import or
  another scheduled run.
- Overdue-on-launch is handled naturally: the first tick after startup runs
  anything due. (No separate catch-up path needed.)
- Emits the existing `import:progress`/`import:complete` events (UI refreshes) and
  records each run via `log_outcome` → `import_log`.

## 4. Config + credentials

New `config.toml` fields (all `#[serde(default)]`, parsed in `config.rs`,
round-tripped in `serialize`):

- Per source: `readwise_sync_enabled`/`readwise_sync_interval_hours`,
  `readwise_tweets_sync_enabled`/`readwise_tweets_sync_interval_hours` +
  `readwise_tweets_last_sync`, `zotero_sync_enabled`/`zotero_sync_interval_hours` +
  `zotero_last_sync`.
- `autostart_enabled` (bool, default **false** for new installs).

`interval_hours`: 0 = off; UI presets map to 1 / 6 / 24. Credentials reuse existing
config fields — no new secret handling. Existing `readwise_last_sync` is reused for
Readwise highlights.

**Autostart change:** `lib.rs` `setup()` currently force-enables autostart. Replace
with: read `autostart_enabled` and enable/disable the launch-agent to match. New
installs default off (don't force launch-at-login on a universal app).

## 5. Settings UI — new "Sync" tab

`SettingsPanel.tsx` gains a "Sync" tab:

- One row per schedulable source: **enable toggle + interval `<select>`** (Off /
  Hourly / Every 6 hours / Daily), plus last-sync time and last status (from the
  import log).
- A **"Launch at login (enables background syncs)"** toggle bound to
  `autostart_enabled`.
- A short note that scheduling runs while the app is open/resident; launch-at-login
  keeps it running.

`Settings` struct (`commands/settings.rs`) + `types.ts` `Settings` gain the new
fields; `get_settings`/`save_settings` round-trip them. The existing import-reminder
control stays.

## 6. Failure handling

Scheduled runs use the same `persist()` + `log_outcome()`. Failures are logged
(visible in the Import Log panel), non-fatal, and retried on the next tick. No
toasts/nags for scheduled failures. A source whose credentials are missing (e.g. no
Readwise key) is skipped silently.

## 7. Version

Bump `0.4.7 → 0.5.0` in `src/version.ts` (APP_VERSION + a RELEASE_NOTES entry) and
`src-tauri/tauri.conf.json`.

## Testing

- Rust unit tests: `readwise_tweets` html→text/image/link parsing + tweet-id keying
  (mirror `import/x.rs` tests); scheduler "is it due?" logic (pure function over
  `last_sync`, `interval`, `now`); config round-trip of new fields.
- Frontend: `tsc` build green; the Sync tab renders and round-trips settings.
- Manual: enable a source on Hourly, confirm a run fires and logs; toggle autostart
  and confirm the launch-agent state changes.

## Out of scope (future)

- OS-level scheduling; syncing file-based sources; per-source notifications; secret
  encryption; additional sources beyond the three (the registry makes them easy to
  add later).
