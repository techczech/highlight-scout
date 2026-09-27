# 05: "Analyse in ArchiveScout ↗" (HS-5A)

**What to build:** A button beside the result count ("30 pieces · 64 matches" style, using real counts) that opens ArchiveScout on the same query and corpora, through ArchiveScout's URL scheme (built in archive-scout plan ticket 02). If ArchiveScout isn't installed, the button says so and links to its release page; it is never a dead button. Both apps use the word "lines" for concordance lines.

**Feature / journey:** `handoff-archivescout` (J7-C).
**Design:** `~/gitrepos/06_apps-utilities/01_desktop-apps/highlight-scout/docs/design/2026-09-27-archive-search-and-corpus-tools/round-1/hs-5-analyse-in-archivescout.html` (HS-5A).
**Blocked by:** 02; archive-scout corpus-lab ticket 02 (the URL scheme).
**Seams under test:** URL construction (query + corpora → archivescout:// URL), unit tested.
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

- [ ] Clicking it opens ArchiveScout's concordance with the same term and corpora and the same line count.
