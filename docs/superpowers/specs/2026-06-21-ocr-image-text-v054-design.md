# Highlight Scout v0.5.4 — OCR Image Text (macOS)

**Goal:** Extract the text inside highlight images (mainly tweet screenshots/tables) with Apple Vision so it becomes full-text searchable, and add a "Copy text from image" command.

**Status:** Approved (2026-06-21). macOS-only feature; the Windows build must keep compiling (OCR absent there).

**Lineage:** item 2 of "Open follow-ups (v0.5.3)" in `2026-06-21-formatting-and-copy-v052-design.md`. Local webp image storage (item 1) remains deferred to a later release; OCR therefore downloads remote tweet images on demand.

---

## Background

Highlight images are currently remote `https://pbs.twimg.com/media/…` URLs embedded in highlight text as `![image](url)` (tweets), or local PNGs at `archive/readings/assets/<id>.png` (Zotero image annotations, `format = "image"`). ~8,054 tweets are already imported, many with images. The SQLite index (`src-tauri/src/index/sqlite.rs`) has a `highlights` table and an FTS5 `search_index` (columns: highlight_id, work_id, text, note, title, author, tags). Search runs FTS `MATCH` plus a concatenated `HAYSTACK` for coverage ranking.

The user's images are mostly tables / text screenshots they want to **find by searching**, and occasionally copy — they do **not** want OCR text displayed in the reading pane.

## Scope

**In scope:** Apple Vision OCR (mac-only) of highlight images; store text; make it searchable; "Copy text from image" command; auto-OCR at import (toggleable) + a manual batch for the backlog.

**Out of scope:** local webp image storage/migration (later release); OCR on Windows/Linux; displaying OCR text in the UI; translating or structuring tables (raw recognized text only).

---

## Architecture

### Swift sidecar (`ocr-helper`)

A small Swift command-line tool using the Vision framework:
- Input: one or more image file **paths** as arguments.
- For each, run `VNRecognizeTextRequest` with `recognitionLevel = .accurate`, `usesLanguageCorrection = true`, automatic languages.
- Output: the recognized text — observations joined by newline, images separated by a blank line — to stdout. Exit non-zero on a hard failure; empty output (exit 0) when an image has no detectable text.

Built **only on macOS**: compiled with `swiftc` during the build and bundled as a Tauri resource (a universal binary covering Apple Silicon + Intel, matching the release's `universal-apple-darwin` target). It is not built or bundled on Windows.

### Rust `ocr` module (`src-tauri/src/ocr.rs`)

`#[cfg(target_os = "macos")]` real implementation; `#[cfg(not(target_os = "macos"))]` stubs that return `None`/empty so the crate compiles everywhere.

- `fn helper_path(app: &AppHandle) -> Option<PathBuf>` — resolves the bundled `ocr-helper` resource.
- `async fn ocr_source(app, source: &str) -> Option<String>` — if `source` is an `http(s)` URL, download it (reuse the `copy_image` download path: a `reqwest` client with timeout + `error_for_status`) to a temp file; otherwise use the local path directly. Invoke `ocr-helper <file>`; return trimmed text (`None` if empty/failure). Temp files are cleaned up.
- `async fn ocr_highlight_sources(app, sources: &[String]) -> String` — OCR each source, join non-empty results with a blank line.
- `async fn ocr_pending(app, window, only_ids: Option<&[String]>)` — the batch/auto driver:
  - Selects highlights that **have images but `ocr_text IS NULL`** (or, when `only_ids` is given, just those ids). "Has images" = `format = 'image'` (Zotero, with `asset_path`) **or** the highlight's `source_data.images` array is non-empty / text contains `![image](`.
  - For each, gather its image sources (Zotero → the local `asset_path`; tweets → the `source_data.images` URLs), OCR them, write the concatenated text to `highlights.ocr_text`, and refresh that highlight's `search_index` row.
  - Emits `import:progress`-style events (reusing the existing progress UI) with current/total; honours a cancellation flag (reuse `AppState.is_syncing`-style guard or a dedicated `is_ocring` atomic).

The image-source extraction reuses the same logic the frontend uses (`![image](url)` from text + Zotero `asset_path`); a shared Rust helper returns the ordered source list for a highlight row.

### Schema + index

- `highlights` gains `ocr_text TEXT` (nullable). Added via idempotent `ALTER TABLE highlights ADD COLUMN ocr_text TEXT` guarded by a column-exists check.
- `search_index` (FTS5) gains an `ocr` column. FTS5 cannot `ALTER ADD COLUMN`, so on schema init: detect whether `search_index` has `ocr`; if not, `DROP TABLE search_index`, recreate it with the new column list, and **repopulate** it from `highlights` joined to `works` (one-time reindex). New installs create it with `ocr` from the start.
- `upsert_highlight` writes `h.ocr_text` into both the `highlights` row and the `search_index` `ocr` column.
- `HAYSTACK` gains `COALESCE(h.ocr_text,'')` so coverage ranking and regex scans see image text.
- `RESULT_COLS` + `map_row` + the `SearchResult` model gain `ocr_text: Option<String>` so the frontend can copy it. The `Highlight` model gains `ocr_text: Option<String>` too (defaulted `None` for all existing importers).

### Triggers

- **Auto-at-import** (default on): after an import persists its highlights, if `ocr_on_import` is true and on macOS, call `ocr_pending(app, window, Some(&new_highlight_ids))`. This covers the file importer, the Readwise tweet importer, and scheduled tweet syncs.
- **Manual batch**: a new command `ocr_images` (mirrors `qmd_reindex`) invoked from Settings → Import → "OCR images"; runs `ocr_pending(app, window, None)` over the whole backlog. Idempotent (skips rows that already have `ocr_text`).
- **Settings toggle** `ocr_on_import: bool` (default `true`) in config + the Settings "Sync"/"Search & view" area. When off, OCR happens only via the manual batch.

### Copy (no display)

- `SearchResult.ocr_text` flows to the frontend.
- `src/lib/copyFormats.ts` gains `imageText(row): string | null` returning `row.ocr_text` (trimmed, or null).
- `CopyMenu` gains a **"Text from image"** item, enabled only when `imageText(row)` is non-empty; it `copyText`s the OCR text.
- A command-palette command `copyImageText` ("Copy text from image"), no default chord.
- Nothing is rendered in the reading pane.

### Cross-platform

OCR Rust code is `cfg`-gated; the `ocr_text`/`ocr` columns exist on every platform (empty on Windows). The Settings OCR toggle, the "OCR images" button, and the "Text from image" copy item are hidden when OCR is unavailable (Windows) or simply never enabled because `ocr_text` is always empty there. A frontend `ocrAvailable()` (true on macOS) gates the toggle + batch button.

## Data flow

Import → highlights persisted → (if `ocr_on_import`) `ocr_pending(new ids)` → per highlight: gather image sources → download/locate → `ocr-helper` → concat text → `UPDATE highlights SET ocr_text` + refresh `search_index` row. Search: FTS `MATCH` now spans the `ocr` column; results carry `ocr_text`; "Text from image" copies it.

## Error handling

- Missing helper (e.g. resource not found) or non-macOS → OCR functions return `None`; import proceeds normally, `ocr_text` stays null.
- Image download failure / unreadable file / helper non-zero exit → that source contributes no text; other sources still processed; the highlight's `ocr_text` is the concatenation of whatever succeeded (or stays null if all fail, so it is retried on the next batch).
- A highlight with no images is never selected for OCR.
- Batch cancellation stops cleanly between highlights.

## Testing

- **Swift helper** (manual / CI script, mac-only): run against a committed fixture PNG containing known text (e.g. "HELLO OCR 123"); assert stdout contains it. Not part of `cargo test`.
- **Rust** (`cargo test`, platform-independent): with a stubbed `ocr_source` (a test seam returning canned text), assert `ocr_pending` selects only image-bearing highlights with null `ocr_text`, writes `ocr_text`, and updates the FTS row. Assert the schema migration: an old `search_index` (no `ocr` column) is recreated and repopulated, and a freshly opened new DB has the `ocr` column. Assert a search whose only matching term is in `ocr_text` returns the highlight.
- **Frontend** (Vitest): `imageText(row)` returns the text when present and null when empty/absent; `CopyMenu` enables "Text from image" only when present (logic-level test of the enable condition).
- **Manual QA**: on macOS, import a tweet with a text-screenshot image, confirm searching a word inside the image finds it, and "Text from image" copies the recognized text. Confirm the Windows build still compiles (CI) with OCR absent.
