# 03: Sets: collect, annotate, export (HS-2A)

**What to build:**
- **Sets as files** in the archive: `<archive_path>/sets/<slug>.md`. This is the ONLY new state; it is git-tracked with the archive.
  - Frontmatter: `title`, `created`, `updated`, `kind: hand | saved-search`, `query` (for saved-search sets).
  - Body: an ordered item list, each item with `passage` (the engine passage id), `corpus`, `source_path`, `quote` (the original text at the time of adding), `pinned` (saved-search sets) and `note`.
  - Items resolve by passage id first. If the id no longer resolves (the source was edited), fall back to `verify-quote` on the stored quote and repair the id. If both fail, the item shows "source changed" and keeps its stored quote.
- **The set page (HS-2A):**
  - "+ Set" (⌘S only if free in the app; report the chord) adds to the current set; a tray shows "Metaphor talk · 7 items";
  - the set page has ordered items with drag and ⌥↑↓ reordering, per-item notes, and ← Search;
  - Export (the right 30%): a WriteFlex piece (Markdown: quote + citation per item, notes as paragraphs between), TalkWeaver quote slides (one quote per slide with its source line; check the TalkWeaver content format in `~/gitrepos/06_apps-utilities/01_desktop-apps/talk-weaver` and write the simplest importable Markdown), and a citation list.
- **Saved-search sets:** a query plus pinned and unpinned items; new matches appear after a sync or index build.

**Feature / journey:** `sets` (J2-A, J2-B).
**Design:** `~/gitrepos/06_apps-utilities/01_desktop-apps/highlight-scout/docs/design/2026-09-27-archive-search-and-corpus-tools/round-1/hs-2-sets.html` (HS-2A frames).
**Blocked by:** 02.
**Seams under test:** the set file read/write module (parse → model → render, byte-stable round trip; id repair); the export renderers (golden files).
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

- [ ] Creating, reordering, annotating and reopening a set keeps the order and notes byte-stable.
- [ ] Each of the three exports produces a file the target app opens (the driver checks WriteFlex and TalkWeaver).
- [ ] A set item whose source line moved is repaired through verify-quote; a test covers it.
- [ ] Set files never write outside `<archive_path>/sets/`.
