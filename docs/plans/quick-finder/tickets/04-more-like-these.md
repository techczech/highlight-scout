# 04: "More like these" suggestions in a set

**What to build:** On the set page, "More like these…" asks the engine's `similar` (scout-core ticket 02) with the set's items as seeds. Suggestions appear under the seeds (HS-2A), each showing the passage, its shared terms ("why") and the seed it resembles, with Approve (adds to the set) and Dismiss. Dismissed suggestions are remembered in the set file (`dismissed:` list) so they don't return.

**Feature / journey:** `sets-suggest` (J2-C).
**Design:** `~/gitrepos/06_apps-utilities/01_desktop-apps/highlight-scout/docs/design/2026-09-27-archive-search-and-corpus-tools/round-1/hs-2-sets.html` (HS-2A suggestions frame).
**Blocked by:** 03; scout-core 02.
**Seams under test:** the suggestion model (seeds → suggestions, minus dismissed and existing items).
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

- [ ] Seeding "Metaphor talk" gives suggestions with shared terms; approving one adds it; dismissing one keeps it away after a reopen.
