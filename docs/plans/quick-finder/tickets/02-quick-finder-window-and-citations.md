# 02: The quick finder window (HS-1B) with citations

**What to build:** The HS-1B layout, as drawn:
- **left rail:** Search in (All corpora, Writing, Tweets, Highlights with the X/Readwise/Zotero sub-counts), Sets (a placeholder list until ticket 03), and Recent searches (persisted locally);
- **results** grouped by corpus, with "show all" per group;
- **row actions** on the selected result: Quote ⌘C, Quote + citation ⌘⇧C, Link, + Set (disabled until 03);
- **reading pane** showing the passage in context and exactly what ⌘⇧C will copy.

Citation format (from the engine's `cite`):
- `— Author, *Full Title*, D Month YYYY · [archive](…) · [public](…)`, with the full title always and both links when a public URL exists;
- rich text outside WriteFlex, Markdown inside it (the "Auto" setting as drawn).

Also HS-3A **Settings → Sync as a table:** per source, the last synced time, the result and any error. The toast and red line already exist; restyle them to the mockup.

**Feature / journey:** `quick-finder-search`, `citation-copy` and `sync-status` (J1-B, J3-B).
**Design:** `~/gitrepos/06_apps-utilities/01_desktop-apps/highlight-scout/docs/design/2026-09-27-archive-search-and-corpus-tools/round-1/hs-1-search-three-corpora.html` (HS-1B frames) and `hs-3-sync.html` (HS-3A frames); `round-1/picks.md`.
**Blocked by:** 01.
**Seams under test:** citation formatting via the engine facade (unit); a UI component test for the grouped results and the row actions, sized like the existing frontend tests.
**Status:** landed (preview)

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

- [ ] It matches the HS-1B frames at 1200×780 in light and dark (the driver screenshots and compares).
- [ ] ⌘⇧C on a writing result puts the full-title citation with both links on the clipboard: Markdown when the frontmost app is WriteFlex, rich HTML otherwise.
- [ ] Keyboard: ↑↓ move, ⌥↓ goes to the next corpus group, Esc hides. No new chord collides with the estate keymap (ADR-0011; ⌘⇧K is reserved estate-wide, so move the existing "Copy citation" off it and report where).
- [ ] The Settings → Sync table shows the three sources with their real last-synced times.
