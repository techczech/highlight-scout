# 07: One search: the classic tools fold into the quick finder (HS-M1B)

**What to build:** Dominik's pick (DTC review-merged-search, 2026-09-28): **HS-M1B, one list, one Filters control**, with "Best matches" as the sort wording. His condition: "we need to preserve the features of see in context and the larger font of the old design".
- **Remove the two modes:** the scope toggle and the "Classic search" button go. The app has ONE search.
- **The rail becomes tick boxes:** one or more of Writing, Tweets, Highlights (with X, Readwise, Zotero under Highlights). The corpus filter becomes multi-select.
- **Keyword / Semantic** is a switch inside the search box. Semantic covers highlights only, so turning it on narrows to highlights and says so.
- **One "Filters · Group" popover** holding every other classic tool, adapting to the ticked corpora: Group (Corpus / Work / Author / Date…), then, Sort, Rows (density), Match mode, colours, favourites/sources/types, has-image, tags and date presets. Anything not at its default shows as a removable chip in a "Showing" row.
- **Every control in `~/gitrepos/06_apps-utilities/01_desktop-apps/highlight-scout/docs/design/2026-09-27-archive-search-and-corpus-tools/round-2/classic-controls.md` keeps its place** (43 kept, 2 removed on purpose). The table is the checklist, and every row must be ticked in the report.
- **Kept from the classic reading pane:**
  - "see in context": the surrounding passages or highlights of the work around the selected one, as classic shows them;
  - the larger reading font the classic pane used.
  - Measure both in the classic code before removing it, and match them.
- **Keyboard:** every classic shortcut still works (the table lists them).

**Also in this ticket:**
- move all scout-core dependencies to **v0.2.3** (the multi-corpus quote fix);
- fix the classic footer hint ("⌘⇧P pane" is wrong; the pane toggle is ⌘\).

**Feature / journey:** `quick-finder-search` (J1); the old-app capabilities are core.
**Design:** `~/gitrepos/06_apps-utilities/01_desktop-apps/highlight-scout/docs/design/2026-09-27-archive-search-and-corpus-tools/round-2/hs-m1b-one-list-one-filters.html`, its screens, and `classic-controls.md`.
**Blocked by:** None (branch feat/quick-finder-01, HEAD after preview.3).
**Seams under test:** the filter model (ticked corpora + filters → engine/classic query; chips ↔ state round trip); a UI test for the popover's adaptive sections; the mount smoke (already in the build).
**Status:** landed (preview.4)

- [ ] One search box; no mode switch anywhere.
- [ ] Each of the 43 classic controls is reachable (a test or a checklist in the report).
- [ ] The see-in-context and reading-pane font size equal the classic ones (report the values).
- [ ] Version 0.6.0-preview.4.
