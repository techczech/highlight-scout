# 08: Rail shows result counts; the copy preview folds away

**What to build:** Dominik's preview.4 verdict (DTC one-search-preview-4, 2026-09-28; screenshots in `_DTC/highlight-scout/2026-09-28-one-search-preview-4.shots/`):
1. **"should show numbers of result":** with a query, each rail row (Writing, Tweets, Highlights, X, Readwise, Zotero) shows the number of RESULTS for this query in that corpus or source, not its size. With no query, show the corpus size as now, visibly in a quieter style. The counts come from the engine and the highlight index (one count call per corpus/source, cached per query); they are never computed from the visible page.
2. **"should not take up space — hide it under accordion or (i)":** the "⌘⇧C copies" preview block in the reading pane is collapsed by default behind a small disclosure or (i) beside the Copy buttons. Expanded, it shows as now. The state is remembered. The reading text gets the freed space.

**Design:** the HS-M1B frames (`docs/design/2026-09-27-archive-search-and-corpus-tools/round-2/`); these two changes are his corrections to them.
**Blocked by:** None (feat/quick-finder-01, preview.4).
**Seams under test:** the search model's per-corpus count state (the query changes, the counts refresh, stale answers are dropped); the pane disclosure state.
**Status:** ready

- [ ] Typing "testing" shows per-corpus result counts in the rail, and they match each corpus's "show all" total.
- [ ] The copy preview is hidden until opened; the version is 0.6.0-preview.5.
