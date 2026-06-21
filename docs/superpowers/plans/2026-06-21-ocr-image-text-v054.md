# OCR Image Text (v0.5.4) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** OCR highlight images with Apple Vision (macOS) so the text inside becomes full-text searchable, with a "Copy text from image" command. No reading-pane display. Windows build stays green.

**Architecture:** A mac-only Swift sidecar (`ocr-helper`) runs `VNRecognizeTextRequest`; `build.rs` compiles it (universal) and a `tauri.macos.conf.json` bundles it as a resource. A `cfg(macos)` Rust `ocr` module downloads remote tweet images (or reads Zotero local files), invokes the helper, and writes `highlights.ocr_text`. OCR text lives in a new `ocr_text` column + a new FTS `ocr` column (searchable). `ocr_text` is **not** added to the `Highlight` struct — the OCR driver is its sole writer and `upsert_highlight` preserves it across re-imports. Triggers: auto-at-import (toggleable) + a manual batch. A "Text from image" copy command surfaces it.

**Tech Stack:** Rust/Tauri 2, Swift + Vision (macOS), SQLite FTS5, TypeScript/React, Vitest.

**Spec:** `docs/superpowers/specs/2026-06-21-ocr-image-text-v054-design.md`

---

## File Structure

**New:**
- `src-tauri/ocr-helper/main.swift` — Vision OCR CLI.
- `src-tauri/tauri.macos.conf.json` — macOS-only `bundle.resources` for the helper.
- `src-tauri/src/ocr.rs` — OCR module (real on macOS, stubs elsewhere).

**Modified:**
- `src-tauri/build.rs` — compile `ocr-helper` (universal) on macOS.
- `src-tauri/src/lib.rs` — `mod ocr;`, register `ocr_images` command, `is_ocring` flag, auto-OCR after import.
- `src-tauri/src/index/sqlite.rs` — `ocr_text` column, FTS `ocr` column + migration, `upsert_highlight` preserve/read-back, `HAYSTACK`, `RESULT_COLS`, `map_row`, a `reindex_highlight_fts` helper, an `ocr_pending`-support query.
- `src-tauri/src/models.rs` — `SearchResult.ocr_text: Option<String>`.
- `src-tauri/src/config.rs` — `ocr_on_import: bool` (default true).
- `src-tauri/src/commands/settings.rs` — expose `ocr_on_import` in `Settings`.
- `src-tauri/src/commands/search.rs` (or a new `commands/ocr.rs`) — `ocr_images` command.
- `src-tauri/src/commands/import.rs` — call auto-OCR after each import persists.
- `src/lib/api.ts`, `src/types.ts`, `src/lib/copyFormats.ts`, `src/components/CopyMenu.tsx`, `src/lib/keybindings.ts`, `src/App.tsx`, `src/components/SettingsPanel.tsx`, `src/components/Toolbar.tsx` (import action) — frontend wiring.
- `src/version.ts`, `src-tauri/tauri.conf.json` — bump to 0.5.4.

---

### Task 1: Swift OCR helper + build + bundling (macOS)

API/toolchain-uncertain (build.rs + swiftc + tauri resource bundling + runtime resolution). Iterate against the real toolchain. Requires Xcode Command Line Tools (`swiftc`, `lipo`) — present on dev macs and CI `macos-latest`.

**Files:**
- Create: `src-tauri/ocr-helper/main.swift`, `src-tauri/tauri.macos.conf.json`
- Modify: `src-tauri/build.rs`

- [ ] **Step 1: Write the Swift helper** — `src-tauri/ocr-helper/main.swift`:

```swift
import Foundation
import Vision
import AppKit

func ocr(_ path: String) -> String {
    guard let img = NSImage(contentsOfFile: path),
          let cg = img.cgImage(forProposedRect: nil, context: nil, hints: nil) else { return "" }
    let request = VNRecognizeTextRequest()
    request.recognitionLevel = .accurate
    request.usesLanguageCorrection = true
    let handler = VNImageRequestHandler(cgImage: cg, options: [:])
    do { try handler.perform([request]) } catch { return "" }
    guard let obs = request.results else { return "" }
    return obs.compactMap { $0.topCandidates(1).first?.string }.joined(separator: "\n")
}

let paths = Array(CommandLine.arguments.dropFirst())
var out: [String] = []
for p in paths {
    let t = ocr(p)
    if !t.isEmpty { out.append(t) }
}
print(out.joined(separator: "\n\n"))
```

- [ ] **Step 2: Compile it in `build.rs`** — replace `src-tauri/build.rs` with:

```rust
fn main() {
    #[cfg(target_os = "macos")]
    build_ocr_helper();
    tauri_build::build();
}

#[cfg(target_os = "macos")]
fn build_ocr_helper() {
    use std::process::Command;
    println!("cargo:rerun-if-changed=ocr-helper/main.swift");
    let _ = std::fs::create_dir_all("binaries");
    let src = "ocr-helper/main.swift";
    let arm = "binaries/ocr-helper-arm64";
    let x86 = "binaries/ocr-helper-x86_64";
    let out = "binaries/ocr-helper";
    let swift = |target: &str, dst: &str| {
        Command::new("swiftc")
            .args(["-O", "-target", target, "-o", dst, src])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    };
    let a = swift("arm64-apple-macosx12.0", arm);
    let x = swift("x86_64-apple-macosx12.0", x86);
    if a && x {
        let _ = Command::new("lipo").args(["-create", "-output", out, arm, x86]).status();
    } else if a {
        let _ = std::fs::copy(arm, out);
    } else if x {
        let _ = std::fs::copy(x86, out);
    } else {
        println!("cargo:warning=swiftc not available; ocr-helper not built (OCR will be disabled)");
    }
}
```

- [ ] **Step 3: Bundle it on macOS only** — create `src-tauri/tauri.macos.conf.json`:

```json
{
  "bundle": {
    "resources": ["binaries/ocr-helper"]
  }
}
```

(Tauri 2 merges `tauri.<platform>.conf.json` over `tauri.conf.json`; Windows never sees this resource.) Add `src-tauri/binaries/` to `.gitignore` if not already ignored (the binary is a build artifact).

- [ ] **Step 4: Build + verify the helper works**

Run: `cd src-tauri && cargo build` (triggers build.rs). Then verify the helper OCRs a real image:
```bash
# create a fixture image with known text, then:
src-tauri/binaries/ocr-helper <path-to-an-image-with-text.png>
```
Expected: prints the recognized text. (If you have no fixture handy, screenshot some text to a PNG and pass it.)

- [ ] **Step 5: Commit**

```bash
git add src-tauri/ocr-helper/main.swift src-tauri/build.rs src-tauri/tauri.macos.conf.json src-tauri/.gitignore
git commit -m "feat: Swift Vision OCR helper compiled + bundled on macOS"
```

---

### Task 2: Schema — ocr_text column, FTS ocr column, migration, wiring

**Files:**
- Modify: `src-tauri/src/index/sqlite.rs`, `src-tauri/src/models.rs`
- Test: `src-tauri/src/index/sqlite.rs` (tests)

- [ ] **Step 1: Write failing tests** — add to the `#[cfg(test)] mod tests` in `sqlite.rs`:

```rust
    #[test]
    fn migrates_old_search_index_to_include_ocr() {
        let conn = Connection::open_in_memory().unwrap();
        // simulate an OLD index: search_index without the `ocr` column
        conn.execute_batch(
            "CREATE VIRTUAL TABLE search_index USING fts5(
                highlight_id UNINDEXED, work_id UNINDEXED, text, note, title, author, tags,
                tokenize='porter unicode61');",
        ).unwrap();
        init_schema(&conn).unwrap(); // must detect missing `ocr` and recreate
        let has_ocr: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('search_index') WHERE name='ocr'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(has_ocr, 1, "search_index should have an ocr column after migration");
    }

    #[test]
    fn search_matches_text_found_only_in_ocr() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        // a work + highlight whose body has none of the term; OCR supplies it
        let w = sample_work("w1");
        upsert_work(&conn, &w).unwrap();
        let h = super::super::import::archive::tests_sample_highlight_unused(); // see note
        // simpler: insert directly
        conn.execute(
            "INSERT INTO highlights (id, work_id, text, tags, format, source_data)
             VALUES ('h1','w1','a plain body', '[]', 'plain', '{}')",
            [],
        ).unwrap();
        conn.execute(
            "UPDATE highlights SET ocr_text='ZEBRACODE inside the screenshot' WHERE id='h1'",
            [],
        ).unwrap();
        reindex_highlight_fts(&conn, "h1").unwrap();
        let q = keyword_query("ZEBRACODE", None);
        let page = search_query(&conn, &q, "/tmp").unwrap();
        assert_eq!(page.rows.len(), 1, "term only in OCR text should be found");
        assert_eq!(page.rows[0].highlight_id, "h1");
    }
```

> Note: use whatever `sample_work` helper exists (or inline an INSERT into `works`). The test only needs a `works` row `w1` and the highlight `h1`. Drop the `tests_sample_highlight_unused` line — insert the highlight via raw SQL as shown. Adjust to the existing test helpers in the file.

- [ ] **Step 2: Run to verify failure**

Run: `cd src-tauri && cargo test --lib sqlite`
Expected: FAIL — `reindex_highlight_fts` undefined; migration not present; `ocr` column absent.

- [ ] **Step 3: Schema — add columns + migration**

In `init_schema`, after creating the tables, add the `ocr_text` column (idempotent) and the FTS migration. Replace the `CREATE VIRTUAL TABLE ... search_index ...` block so new installs include `ocr`, and add migration logic after the `execute_batch`:

```rust
    // highlights.ocr_text (idempotent add)
    let has_ocr_text: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('highlights') WHERE name='ocr_text'",
        [], |r| r.get(0),
    ).unwrap_or(0);
    if has_ocr_text == 0 {
        conn.execute("ALTER TABLE highlights ADD COLUMN ocr_text TEXT", [])?;
    }

    // search_index must have an `ocr` column; FTS5 can't ALTER ADD, so recreate.
    let fts_has_ocr: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('search_index') WHERE name='ocr'",
        [], |r| r.get(0),
    ).unwrap_or(0);
    if fts_has_ocr == 0 {
        conn.execute_batch(
            "DROP TABLE IF EXISTS search_index;
             CREATE VIRTUAL TABLE search_index USING fts5(
                highlight_id UNINDEXED, work_id UNINDEXED,
                text, note, title, author, tags, ocr,
                tokenize='porter unicode61');",
        )?;
        // repopulate from highlights joined to works
        let mut stmt = conn.prepare(
            "SELECT h.id, h.work_id, h.text, h.note, w.title, w.author, h.tags, h.ocr_text
             FROM highlights h JOIN works w ON w.id = h.work_id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?, r.get::<_, String>(4)?,
                r.get::<_, Option<String>>(5)?, r.get::<_, String>(6)?,
                r.get::<_, Option<String>>(7)?,
            ))
        })?.collect::<Result<Vec<_>, _>>()?;
        for (id, wid, text, note, title, author, tags_json, ocr) in rows {
            let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
            conn.execute(
                "INSERT INTO search_index (highlight_id, work_id, text, note, title, author, tags, ocr)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                params![id, wid, text, note.unwrap_or_default(), title,
                    author.unwrap_or_default(), tags.join(" "), ocr.unwrap_or_default()],
            )?;
        }
    }
```

Also update the base `CREATE VIRTUAL TABLE IF NOT EXISTS search_index` in the initial `execute_batch` to include `ocr` (so fresh installs match; the migration only fires for pre-existing indexes without it).

- [ ] **Step 4: `upsert_highlight` — preserve + index ocr_text**

`upsert_highlight` must NOT clobber `ocr_text` (it isn't on the `Highlight` struct, so the highlights UPDATE already omits it — good). For the `search_index` INSERT, read the current `ocr_text` back and include it. After the highlights INSERT/UPDATE and the `DELETE FROM search_index`, change the `search_index` INSERT to also set `ocr`:

```rust
    let ocr_text: String = conn.query_row(
        "SELECT COALESCE(ocr_text,'') FROM highlights WHERE id=?1",
        params![h.id], |r| r.get(0),
    ).unwrap_or_default();
    conn.execute(
        "INSERT INTO search_index
         (highlight_id, work_id, text, note, title, author, tags, ocr)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![h.id, h.work_id, h.text, h.note.as_deref().unwrap_or(""),
            work_title, work_author.unwrap_or(""), h.tags.join(" "), ocr_text],
    )?;
```

- [ ] **Step 5: `reindex_highlight_fts` helper** (used by the OCR driver)

Add a public helper that rebuilds one highlight's FTS row from the DB (so the OCR driver can refresh it after writing `ocr_text`):

```rust
/// Rebuild a single highlight's search_index row from the DB (used after OCR
/// writes ocr_text). Joins works for title/author.
pub fn reindex_highlight_fts(conn: &Connection, highlight_id: &str) -> Result<()> {
    conn.execute("DELETE FROM search_index WHERE highlight_id = ?1", params![highlight_id])?;
    conn.execute(
        "INSERT INTO search_index (highlight_id, work_id, text, note, title, author, tags, ocr)
         SELECT h.id, h.work_id, h.text, COALESCE(h.note,''), w.title, COALESCE(w.author,''),
                h.tags, COALESCE(h.ocr_text,'')
         FROM highlights h JOIN works w ON w.id = h.work_id WHERE h.id = ?1",
        params![highlight_id],
    )?;
    Ok(())
}
```

(Note: `tags` here is the JSON array string, whereas `upsert_highlight` stores space-joined tags in FTS. For FTS matching this difference is cosmetic — tags still tokenize. If you want exact parity, parse + join; acceptable either way. Prefer parity: do the join in Rust like `upsert_highlight` if simple, else leave the JSON — FTS tokenizes punctuation out.)

- [ ] **Step 6: HAYSTACK, RESULT_COLS, map_row, SearchResult**

- `HAYSTACK`: append `||' '||COALESCE(h.ocr_text,'')` inside the parentheses.
- `RESULT_COLS`: append `, h.ocr_text`.
- `map_row`: read the new trailing column into `ocr_text` (use the next index after the current last; confirm the index).
- `models.rs` `SearchResult`: add `pub ocr_text: Option<String>,` (place near `snippet`); update any `SearchResult { ... }` literal in tests to include it.

- [ ] **Step 7: Run tests**

Run: `cd src-tauri && cargo test --lib`
Expected: the two new tests pass; all existing pass (update any `SearchResult`/`keyword_query` literals for the new field/columns as the compiler directs).

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/index/sqlite.rs src-tauri/src/models.rs
git commit -m "feat: ocr_text column + FTS ocr column + migration, searchable"
```

---

### Task 3: Rust `ocr` module (cfg-gated) + driver

**Files:**
- Create: `src-tauri/src/ocr.rs`
- Modify: `src-tauri/src/lib.rs` (`mod ocr;`, `is_ocring` flag on `AppState`)

- [ ] **Step 1: Image-source extraction helper**

Add to `sqlite.rs` (or `ocr.rs`) a function returning a highlight's OCR-able sources given its row fields. Reuse the `![image](url)` convention + Zotero `asset_path`:

```rust
/// Image sources to OCR for a highlight: Zotero image annotations → the local
/// asset PNG; otherwise the ![image](url) URLs embedded in the text.
pub fn ocr_sources(format: &str, asset_path: Option<&str>, text: &str) -> Vec<String> {
    if format == "image" {
        return asset_path.map(|p| vec![p.to_string()]).unwrap_or_default();
    }
    let mut out = Vec::new();
    let re = regex::Regex::new(r"!\[[^\]]*\]\((https?://[^)\s]+)\)").unwrap();
    for c in re.captures_iter(text) {
        out.push(c[1].to_string());
    }
    out
}
```

(`regex` is already a dependency.)

- [ ] **Step 2: The OCR module with a test seam**

Create `src-tauri/src/ocr.rs`. The core driver `ocr_pending` is platform-independent and testable via an injected OCR function; the real `ocr_source` (download/helper) is macOS-only.

```rust
use crate::index::sqlite;
use rusqlite::Connection;

/// Find highlight ids that have images but no ocr_text yet (optionally limited
/// to a set of ids, e.g. just-imported ones).
pub fn pending_ids(conn: &Connection, only: Option<&[String]>) -> Vec<(String, String, Option<String>, String)> {
    // returns (id, format, asset_path, text) for rows needing OCR
    let base = "SELECT id, format, NULL, text FROM highlights
                WHERE (ocr_text IS NULL OR ocr_text='')
                AND (format='image' OR text LIKE '%![image](%')";
    let mut stmt = conn.prepare(base).expect("prepare pending");
    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_,String>(0)?, r.get::<_,String>(1)?, r.get::<_,Option<String>>(2)?, r.get::<_,String>(3)?))
    }).expect("query pending").filter_map(|x| x.ok());
    let all: Vec<_> = rows.collect();
    match only {
        Some(ids) => all.into_iter().filter(|(id,_,_,_)| ids.contains(id)).collect(),
        None => all,
    }
}

/// Drive OCR over pending highlights using an injected per-source OCR fn.
/// Returns the number of highlights updated. The `ocr_fn` takes (format, asset_path, text)
/// and returns the concatenated recognized text (empty = nothing recognised).
pub async fn run_ocr<F, Fut>(conn: &Connection, archive: &str, only: Option<&[String]>, mut ocr_fn: F) -> usize
where
    F: FnMut(Vec<String>) -> Fut,
    Fut: std::future::Future<Output = String>,
{
    let pending = pending_ids(conn, only);
    let mut updated = 0usize;
    for (id, format, _asset, text) in pending {
        // Zotero asset path is derived from archive + id (see map_row convention).
        let asset = if format == "image" {
            Some(format!("{}/readings/assets/{}.png", archive.trim_end_matches('/'), id))
        } else { None };
        let sources = sqlite::ocr_sources(&format, asset.as_deref(), &text);
        if sources.is_empty() { continue; }
        let ocr_text = ocr_fn(sources).await;
        // Always write (even empty) so we don't retry forever? No — only write non-empty,
        // leave empty as retry-able. Write empty as '' to mark "tried, nothing"? Decision:
        // write whatever we got; mark tried with non-NULL. Use empty string to mean "tried".
        conn.execute("UPDATE highlights SET ocr_text=?2 WHERE id=?1",
            rusqlite::params![id, ocr_text]).ok();
        sqlite::reindex_highlight_fts(conn, &id).ok();
        updated += 1;
    }
    updated
}
```

> Decision baked in: write `ocr_text` (even empty string) once attempted, so a textless image isn't retried every batch. The empty string is falsy for the "Copy text from image" gate. If you'd rather retry failures, only write on non-empty and leave true failures NULL — but then every batch re-downloads textless images. Go with "write once attempted" (empty = tried).

```rust
#[cfg(target_os = "macos")]
pub mod platform {
    use tauri::{AppHandle, Manager};
    use std::path::PathBuf;

    pub fn helper_path(app: &AppHandle) -> Option<PathBuf> {
        app.path().resolve("binaries/ocr-helper", tauri::path::BaseDirectory::Resource).ok()
            .filter(|p| p.exists())
    }

    /// Download (http/https) or locate (local path) each source, OCR via the
    /// helper, return concatenated text.
    pub async fn ocr_sources(app: &AppHandle, sources: Vec<String>) -> String {
        let Some(helper) = helper_path(app) else { return String::new() };
        // ensure executable
        #[cfg(unix)]
        { use std::os::unix::fs::PermissionsExt;
          if let Ok(m) = std::fs::metadata(&helper) { let mut p = m.permissions(); p.set_mode(0o755); let _ = std::fs::set_permissions(&helper, p); } }
        let mut files: Vec<(PathBuf, bool)> = Vec::new(); // (path, is_temp)
        let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(20)).build().ok();
        for s in &sources {
            if s.starts_with("http://") || s.starts_with("https://") {
                if let Some(c) = &client {
                    if let Ok(resp) = c.get(s).send().await {
                        if let Ok(resp) = resp.error_for_status() {
                            if let Ok(bytes) = resp.bytes().await {
                                let tmp = std::env::temp_dir().join(format!("hs-ocr-{}.img", uuid::Uuid::new_v4()));
                                if std::fs::write(&tmp, &bytes).is_ok() { files.push((tmp, true)); }
                            }
                        }
                    }
                }
            } else {
                let p = PathBuf::from(s);
                if p.exists() { files.push((p, false)); }
            }
        }
        if files.is_empty() { return String::new(); }
        let args: Vec<String> = files.iter().map(|(p,_)| p.to_string_lossy().to_string()).collect();
        let out = tokio::process::Command::new(&helper).args(&args).output().await.ok();
        for (p, is_temp) in &files { if *is_temp { let _ = std::fs::remove_file(p); } }
        out.filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default()
    }
}

#[cfg(not(target_os = "macos"))]
pub mod platform {
    use tauri::AppHandle;
    pub async fn ocr_sources(_app: &AppHandle, _sources: Vec<String>) -> String { String::new() }
}

pub fn available() -> bool { cfg!(target_os = "macos") }
```

- [ ] **Step 3: Add `is_ocring` to AppState + `mod ocr`**

In `src-tauri/src/lib.rs`: add `mod ocr;`; add `pub is_ocring: std::sync::atomic::AtomicBool` to `AppState` and initialise `false` in the `.manage(AppState { ... })`.

- [ ] **Step 4: Unit test the driver (platform-independent)**

Add to `ocr.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::sqlite::{init_schema, reindex_highlight_fts};
    use rusqlite::Connection;

    #[tokio::test]
    async fn run_ocr_writes_text_only_for_image_highlights() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        conn.execute("INSERT INTO works (id,slug,title,source_system,imported_at,updated_at) VALUES ('w','w','W','x','t','t')", []).unwrap();
        conn.execute("INSERT INTO highlights (id,work_id,text,tags,format,source_data) VALUES ('img','w','see ![image](https://p/a.jpg)','[]','plain','{}')", []).unwrap();
        conn.execute("INSERT INTO highlights (id,work_id,text,tags,format,source_data) VALUES ('noimg','w','plain text','[]','plain','{}')", []).unwrap();
        let n = run_ocr(&conn, "/tmp", None, |_srcs| async { "RECOGNISED".to_string() }).await;
        assert_eq!(n, 1, "only the image highlight is processed");
        let got: Option<String> = conn.query_row("SELECT ocr_text FROM highlights WHERE id='img'", [], |r| r.get(0)).unwrap();
        assert_eq!(got.as_deref(), Some("RECOGNISED"));
        let none: Option<String> = conn.query_row("SELECT ocr_text FROM highlights WHERE id='noimg'", [], |r| r.get(0)).unwrap();
        assert_eq!(none, None);
        // and it is now searchable
        reindex_highlight_fts(&conn, "img").unwrap();
        let cnt: i64 = conn.query_row("SELECT COUNT(*) FROM search_index WHERE search_index MATCH 'RECOGNISED'", [], |r| r.get(0)).unwrap();
        assert_eq!(cnt, 1);
    }
}
```

Run: `cd src-tauri && cargo test --lib ocr` — expect PASS. `cargo build` — compiles on macOS (and the `not(macos)` stubs keep non-mac green; you can't test that here but the cfg structure ensures it).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/ocr.rs src-tauri/src/lib.rs src-tauri/src/index/sqlite.rs
git commit -m "feat: OCR driver (cfg-gated Vision helper + testable batch over pending images)"
```

---

### Task 4: Commands + triggers (manual batch, auto-at-import, setting)

**Files:**
- Modify: `src-tauri/src/config.rs`, `src-tauri/src/commands/settings.rs`, `src-tauri/src/commands/import.rs`, `src-tauri/src/lib.rs`, `src/lib/api.ts`

- [ ] **Step 1: Config + setting**

In `config.rs`: add `pub ocr_on_import: bool` to `Config` (default `true` — set in the default constructor and `parse_config_text`). In `commands/settings.rs`: add `ocr_on_import: bool` to `Settings`, populate in `get_settings`, persist in `save_settings`.

- [ ] **Step 2: `ocr_images` manual batch command**

Add to `commands/search.rs` (alongside `qmd_reindex`) — or a new `commands/ocr.rs` registered in `commands/mod.rs`:

```rust
#[tauri::command]
pub async fn ocr_images(app: tauri::AppHandle, window: tauri::WebviewWindow) -> Result<usize, String> {
    use std::sync::atomic::Ordering;
    let state = app.state::<crate::AppState>();
    if state.is_ocring.swap(true, Ordering::SeqCst) { return Err("OCR already running".into()); }
    let archive = state.config().archive_path.clone();
    let result = crate::ocr::run_ocr_app(&app, &window, &archive, None).await;
    app.state::<crate::AppState>().is_ocring.store(false, Ordering::SeqCst);
    Ok(result)
}
```

Add a thin `run_ocr_app` wrapper in `ocr.rs` that binds `run_ocr`'s `ocr_fn` to `platform::ocr_sources(app, ...)`, snapshots the DB connection from state per highlight (mind the `Mutex<Connection>` — do DB work synchronously between awaits, like the scheduler does), and emits `import:progress` events on `window` with current/total. Keep the State borrow off across `.await` (fetch sources synchronously, drop the lock, await the OCR, re-lock to write). Model the borrow discipline on the existing scheduler in `lib.rs`.

- [ ] **Step 3: Auto-OCR after import**

In `commands/import.rs`, in the shared `persist()` (or right after each import command persists works+highlights), if `cfg!(target_os="macos")` and `config().ocr_on_import`, collect the imported highlight ids and run OCR over just those: `crate::ocr::run_ocr_app(&app, &window, &archive, Some(&ids)).await` (guarded by `is_ocring`). Ensure this does not run during unit tests / when ids is empty.

- [ ] **Step 4: Register + api.ts**

`lib.rs`: add `commands::...::ocr_images` to `generate_handler!`. `src/lib/api.ts`: add `export function ocrImages() { return invoke<number>("ocr_images"); }`.

- [ ] **Step 5: Build + tests**

Run: `cd src-tauri && cargo build && cargo test --lib` — compiles, all pass. (No new unit test required here beyond Task 3's driver test; the command is thin glue.)

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/config.rs src-tauri/src/commands/ src-tauri/src/lib.rs src-tauri/src/ocr.rs src/lib/api.ts
git commit -m "feat: ocr_images command, auto-at-import OCR, ocr_on_import setting"
```

---

### Task 5: Frontend — copy command + settings

**Files:**
- Modify: `src/types.ts`, `src/lib/copyFormats.ts` (+ test), `src/components/CopyMenu.tsx`, `src/lib/keybindings.ts`, `src/App.tsx`, `src/components/SettingsPanel.tsx`, `src/components/Toolbar.tsx`

- [ ] **Step 1: Type + serializer (TDD the serializer)**

`src/types.ts`: add `ocr_text: string | null;` to `SearchResult`. In `src/lib/copyFormats.test.ts`, add:

```ts
  test("imageText returns ocr_text when present, null when empty", () => {
    expect(imageText(tweet({ ocr_text: "TEXT IN PIC" }))).toBe("TEXT IN PIC");
    expect(imageText(tweet({ ocr_text: "" }))).toBeNull();
    expect(imageText(tweet({ ocr_text: null }))).toBeNull();
  });
```

(Add `ocr_text: null` to the `tweet()` fixture's defaults.) Then in `copyFormats.ts`:

```ts
/** OCR'd text from the highlight's image(s), or null when none. */
export function imageText(row: SearchResult): string | null {
  const t = (row.ocr_text || "").trim();
  return t === "" ? null : t;
}
```

Run `bun run test src/lib/copyFormats.test.ts` (fail → pass) and `bunx tsc --noEmit`.

- [ ] **Step 2: CopyMenu item**

In `src/components/CopyMenu.tsx`, import `imageText`; compute `const ocr = imageText(row);`; add an item after "Image", enabled only when `ocr` is non-null:

```tsx
          <Item
            disabled={!ocr}
            onClick={() => run(() => copyText(ocr ?? ""), "Copied image text", "Copy failed")}
          >
            Text from image
          </Item>
```

(`copyText` is already imported in CopyMenu.)

- [ ] **Step 3: Command-palette command**

`src/lib/keybindings.ts`: add `"copyImageText"` to `CommandId` and a `COMMANDS` entry `{ id: "copyImageText", label: "Copy text from image", group: "Actions", default: "" }`. `src/App.tsx`: add the impl and map entry:

```tsx
  const copyImageTextCmd = async () => {
    const t = activeRow ? imageText(activeRow) : null;
    if (t) { await copyText(t); showToast("Copied image text"); }
    else showToast("No image text");
  };
```
and `copyImageText: copyImageTextCmd,` in the `commands` map; import `imageText` from `./lib/copyFormats`.

- [ ] **Step 4: Settings — toggle + "OCR images" button (mac-only)**

Add a frontend `ocrAvailable()` (e.g. in `src/lib/api.ts`: `export const ocrAvailable = () => navigator.userAgent.includes("Mac");`). In `SettingsPanel.tsx` (Sync or Search&view tab), when `ocrAvailable()`: a checkbox bound to the `ocr_on_import` setting, and a button "OCR images" calling `ocrImages()` (show a toast with the count, e.g. `OCR'd N images`). The Toolbar/Import menu may also surface "OCR images" if that's where reindex lives — match the existing "Rebuild semantic index" placement. When not available, render nothing.

- [ ] **Step 5: Build**

Run: `bun run test && bunx tsc --noEmit && bun run build` — all pass.

- [ ] **Step 6: Commit**

```bash
git add src/
git commit -m "feat: 'Text from image' copy + OCR settings (mac-only)"
```

---

### Task 6: Version bump + manual QA

**Files:** `src/version.ts`, `src-tauri/tauri.conf.json`

- [ ] **Step 1: Bump** — `version.ts` to `0.5.4` + prepend:

```ts
  {
    version: "0.5.4",
    notes: [
      "Text inside images is now searchable: Highlight Scout reads images with on-device OCR (macOS) so you can find tweets and screenshots by the words in the picture.",
      "New \"Text from image\" copy option for image highlights.",
      "Runs automatically on import (toggle in Settings) — or use Settings → \"OCR images\" to process your existing library.",
    ],
  },
```
and `tauri.conf.json` `"version": "0.5.4"`.

- [ ] **Step 2: Full sweep** — `bun run test && cd src-tauri && cargo test && cd ..` — all pass.

- [ ] **Step 3: Build + install** — `cargo tauri build`; quit/replace `/Applications/Highlight Scout.app`; relaunch.

- [ ] **Step 4: Manual QA (macOS)**
- Settings → "OCR images" → progress runs; toast reports a count.
- Search a word that only appears inside a tweet's image → the tweet is found.
- On that highlight, Copy menu → "Text from image" → pastes the recognised text.
- Toggle "OCR on import" off, import, confirm no auto-OCR; on, confirm it runs.
- (CI) confirm the **Windows** build still compiles with OCR absent.

- [ ] **Step 5: Commit**

```bash
git add src/version.ts src-tauri/tauri.conf.json
git commit -m "chore: bump to 0.5.4 (OCR image text)"
```

---

## Self-Review

**1. Spec coverage:** Swift Vision helper + build + bundle (T1); ocr_text column + FTS ocr + migration + searchable + SearchResult (T2); cfg-gated OCR module + download/helper + batch driver + image-source extraction (T3); manual batch command + auto-at-import + setting (T4); copy command + settings UI + mac gating, no display (T5); version bump + Windows-compiles QA (T6). ✓

**2. Placeholder scan:** No TBD/TODO. T1 (toolchain) and parts of T3/T4 (borrow discipline around `Mutex<Connection>` across awaits) are flagged as iterate-against-reality, with the contract pinned by tests where testable and the existing scheduler cited as the borrow model — deliberate, not placeholder.

**3. Type/name consistency:** `ocr_text` (DB column + SearchResult + TS), `ocr` (FTS column), `reindex_highlight_fts`, `ocr_sources`/`pending_ids`/`run_ocr`/`run_ocr_app`, `platform::ocr_sources`, `ocr_images` command ↔ `ocrImages()` api ↔ `copyImageText` command ↔ `imageText()` serializer — names consistent across tasks. `ocr_on_import` consistent (config ↔ Settings ↔ checkbox).
