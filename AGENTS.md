# Highlight Scout

## Project

- Tauri 2 + Rust backend + Vite/React/TS frontend. macOS-first desktop app.
- Purpose: multi-source highlight archive + fast local search.
- Governance + ADRs live in `~/gitrepos/_COORDINATION/highlights/` — read those before changing architecture.

## Architecture (do not relitigate; see _COORDINATION/highlights/_ADR/)

- Owns the full import pipeline, Rust-native, no Python (ADR-0006).
- Sources: Readwise REST, Zotero local SQLite (read-only/immutable), Kindle clippings (planned).
- Archive = v2 Markdown, committed to git: `readings/works/{slug}.md` (flat), `readings/fulltext/{slug}.md`, `readings/assets/{id}.png` (ADR-0003).
- Index = SQLite FTS5, local-only, never committed; synced via R2 (ADR-0001). Built from the Archive.
- annotation_color / annotation_type are first-class nullable fields (ADR-0003). Standard Zotero palette → names; custom → hex.
- Search: FTS default, QMD semantic via mode toggle, fast-follow (ADR-0005).
- Raycast extension is a passive fallback, not a parallel UI (ADR-0004).

## Layout

- `src-tauri/src/import/` — `readwise.rs`, `zotero.rs`, `archive.rs` (v2 writer + `make_slug`)
- `src-tauri/src/index/sqlite.rs` — schema, upsert, search, facets
- `src-tauri/src/commands/` — Tauri commands (`search`, `import`)
- `src-tauri/src/config.rs` — `~/.config/highlight-scout/config.toml`
- `src/` — React UI (`App.tsx`, `components/`, `lib/api.ts`)

## Conventions

- SQLite: system sqlite (no `bundled` feature — `rusqlite =0.31` pinned; newer needs unstable `cfg_select`).
- Slugs always include a source-id suffix for uniqueness; cap length before the suffix.
- Imports must be additive + resilient: a secondary fetch (e.g. Reader full text) must never fail the primary highlight import.
- Never render untrusted strings via `dangerouslySetInnerHTML`; FTS snippets go through `lib/snippet.tsx`.

## Build / test

- `bun run tauri dev` — run; `bun run build` — typecheck + frontend build.
- `cargo test --lib` (in `src-tauri/`) — pure-logic + real-DB integration tests (skip if no Zotero DB).
- Hide-on-blur is disabled in debug builds so devtools stay usable.

## Tasks

- Task log: `~/gitrepos/_COORDINATION/highlights/_TASK-LOG/`.
