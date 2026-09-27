# 01: Search writing, tweets and highlights from Highlight Scout

**What to build:** Highlight Scout depends on scout-core `v0.2.0` (the git tag) and exposes Tauri commands backed by the scout-corpus facade: search with `--in` corpora, cite, and index status.
- The existing search box can search all three corpora. Results show a corpus badge first (Writing / Tweet / Highlight) and open in the reading pane with the original passage.
- Engine calls run off the main thread.
- If an index is stale or missing, the app runs an incremental `index build` in the background and says so.

**Feature / journey:** `quick-finder-search` (J1-B, the hotkey window over any app).
**Design:** `~/gitrepos/06_apps-utilities/01_desktop-apps/highlight-scout/docs/design/2026-09-27-archive-search-and-corpus-tools/round-1/hs-1-search-three-corpora.html` (HS-1B). This ticket only proves the data path; the HS-1B layout is ticket 02.
**Blocked by:** scout-core ticket 01 landed and tagged v0.2.0.
**Seams under test:** the Tauri command layer (a command → facade → a typed result), tested with a fixture corpus in a temp dir, as the existing Rust tests do.
**Status:** ready

**Context (cold read):**
- The Scout corpus engine lives in ~/gitrepos/06_apps-utilities/03_misc-utilities/scout-core (spec: docs/specs/2026-09-27-corpus-engine-and-cli.md; JSON shapes: docs/cli-json.md). It indexes three corpora: `writing` (Dominik's writing, 1,742 pieces), `tweets` (14,892, one doc per tweet) and `highlights` (the Highlight Scout archive, 14,914 works).
- The registry is at `~/.config/scout/corpora.toml`; the indexes are in `~/Library/Application Support/scout/indexes/`. The CLI is `scout`.
- **Invariants:**
  - sources are read-only;
  - quotes are original text;
  - counts agree across views;
  - the app and CLI call the same library functions (no logic duplicated in apps).
- Journeys and picks: `~/gitrepos/06_apps-utilities/01_desktop-apps/highlight-scout/docs/design/2026-09-27-archive-search-and-corpus-tools/journeys.md` (see "Dominik's picks").
- The app's existing highlight search (scout-index, FTS5) keeps working until this plan replaces it; the archive write path (sync, merge) is OUT of scope and must not change.
- Locked mockups: `~/gitrepos/06_apps-utilities/01_desktop-apps/highlight-scout/docs/design/2026-09-27-archive-search-and-corpus-tools/round-1/` (read `picks.md` there first).

- [ ] Searching "generative metaphor" shows the 2016 essay (Writing), tweets and highlights, each with a badge.
- [ ] The reading pane shows the original passage text, with the source file and line.
- [ ] Existing highlight search, filters and all existing tests still pass.
- [ ] No writes to any corpus source (the git status of the writing repo and the archive is unchanged by searching).
