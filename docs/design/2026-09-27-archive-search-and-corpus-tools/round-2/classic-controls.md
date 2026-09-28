---
title: "Merged search round 2: every Classic control and where it lives"
date: 2026-09-28
source: "src/App.tsx, src/components/Toolbar.tsx, FilterPopover.tsx, SearchBar.tsx, ReadingPane.tsx, CopyMenu.tsx, TagPicker.tsx, src/lib/keybindings.ts, @scout/query v0.2.1 (query.ts); 0.6.0-preview.2"
directions: [HS-M1A, HS-M1B]
---

# Every Classic control, and where it lives in each direction

**HS-M1A (the rail is the scope):** under All corpora you get the HS-1B view. Picking Highlights, X, Readwise or Zotero in the rail shows a tool bar above the results, and adds Group and Sort to the list header.
**HS-M1B (one list, one Filters control):** the rail has tick boxes. Keyword / Semantic is a switch in the search box. Everything else sits in one "Filters · Group" popover, and anything not at its default shows as a removable chip.

"Highlight scope" means Highlights or one of its sources is selected (A) or ticked (B). A dash in the Frame column means the control keeps its place and is not drawn this round.

## Search row

| Classic control | HS-M1A | HS-M1B | Frame |
|---|---|---|---|
| Search box, with the query grammar in its placeholder | The search row, unchanged; the grammar works in every scope | The search row; the grammar line also shows in the empty window | A1–A4, B1–B4 |
| Searching spinner | In the search row, as in HS-1B | Same | – |
| **Filters** button (⌘⇧I) | In the tool bar, beside Tags. ⌘⇧I pressed under All switches to Highlights first | The one "Filters · Group" button in the search row, amber when anything is set | A2–A3, B1–B3 |
| ⟳ Refresh (re-run the search, reload counts) | Icon at the left end of the footer | Same | all |
| ⚙ Settings (⌘,) | Gear at the right end of the search row | Same | all |
| Scope toggle "Highlights / Writing · Tweets · Highlights" | **Removed.** The rail is the scope | **Removed.** The rail's tick boxes are the scope | – |
| "Classic search" button (quick finder) | **Removed**; frame 4 hint says where Classic went | **Removed** | A4 |

## Classic toolbar

| Classic control | HS-M1A | HS-M1B | Frame |
|---|---|---|---|
| Keyword / Semantic | Segmented switch at the left of the tool bar (highlight scope only) | Segmented switch inside the search box, always visible. Semantic covers highlights only, so turning it on unticks Writing and Tweets and says so | A2, B1–B2 |
| Semantic runs on ⏎ | "Semantic · ⏎ runs" in the search row | "⏎ runs" beside the switch | A2, B2 |
| Match: whole word / partial (keyword only) | Button after the switch, shown in Keyword mode only | "Match" row in the popover's View section, greyed in semantic | B3 |
| Sort: Most matches / Most recent / Oldest (⌘⇧S cycles) | List header, right. Under All it is the HS-1B sort. **One list of labels; see open question** | Popover, View section. ⌘⇧S cycles without opening it | A2, B3 |
| Group: Work / Author / Date (year) / Tag / None (⌘⇧G cycles) | List header, left (highlight scope). Under All, results stay grouped by corpus | Popover, View section. **Corpus** is one of the Group values, and is the default when more than one corpus is ticked | A2, B1, B3 |
| "then" sub-group | List header, after Group | Popover, after Group | A2, B3 |
| Rows: Minimal / Compact / Comfortable / Full quotes (⌘⇧D cycles) | Right end of the tool bar | Popover, View section | A2, B3 |
| Hide / Show pane (⌘\\) | Pane icon in the search row | Same | all |
| Tags (⌘⇧T, opens the tag picker, adds `tag:"…"`) | Tags button in the tool bar, opens the same picker | Tag field in the popover's Highlights section; ⌘⇧T opens the popover with the field focused | A2, B3 |

## Colour row

| Classic control | HS-M1A | HS-M1B | Frame |
|---|---|---|---|
| Colour swatches (up to 14 from the facets) | In the tool bar, all 14; the selected one is ringed | Popover, Highlights section; the choice shows as a "Colour: red ×" chip | A2, B2–B3 |
| "clear" beside the colours; ⌘⇧X clears the colour | "clear" link after the swatches; ⌘⇧X | The chip's ×, "clear all" in the chips row; ⌘⇧X | A2, B2 |

## Filters popover (⌘⇧I)

| Classic control | HS-M1A | HS-M1B | Frame |
|---|---|---|---|
| Quick: ★ Favorites · 🔖 Zotero · 🖼 Has image | Popover, as today | Popover, Highlights section | A3, B3 |
| Type: Articles · Books · Tweets · PDFs · Podcasts | Popover, as today | Popover, Highlights section | A3, B3 |
| Time: Any · 30 days · 6 months · Year | Popover, as today | Popover, "Any corpus" section, because it applies to writing and tweets too | A3, B3 |
| Clear all | Popover foot | Popover foot, plus "clear all" in the chips row | A3, B2–B3 |
| Keyboard: ↑↓ move, Space / Enter toggles, Esc closes, Home / End | Unchanged | Unchanged; the popover foot shows the keys | B3 |
| Count on the button, "Filters (n)", amber when set | Unchanged | "Filters (n) · Group: …" in amber | B2 |

## Banners and empty states

| Classic control | HS-M1A | HS-M1B | Frame |
|---|---|---|---|
| "Semantic search needs QMD" banner, Get QMD ↗ | Under the tool bar, when Semantic is on and QMD is missing | Under the chips row, same condition | – |
| "Press ↵ to search semantically for …" | Results area, highlight scope | Results area | – |
| "No highlights yet" + Import highlights → | Results area, when the library is empty | Same | – |
| Grammar hint line: `cat OR dog · "exact phrase" · -exclude · prefix* · au:scott ty:books y:2023 · /\bAI\b/` | Results area when the query is empty | Empty window, with `co:red` and `i:` added and the table of old keys | B4 |

## Results list

| Classic control | HS-M1A | HS-M1B | Frame |
|---|---|---|---|
| Group section heads (work title + author + count) | Work heads in highlight scope; corpus heads under All | Heads follow the Group value | A2, B2 |
| Row density (from Rows) | Unchanged | Unchanged | – |
| Infinite scroll (load more) | Unchanged | Unchanged | – |
| Click selects, opens the pane | Unchanged | Unchanged | – |
| Open detail (work view) | Unchanged (⌘⇧L) | Unchanged | – |
| Row copy actions (quick finder: Quote, Quote + citation, Link, + Set) | On the selected row in every scope, now including highlights | Same | A1–A2, B1–B2 |

## Reading pane

| Classic control | HS-M1A | HS-M1B | Frame |
|---|---|---|---|
| Text or image, Note, Citation | Pane body | Same | A2, B2 |
| Open PDF in Zotero ↗ · the source URL | Under the title | Same | A2, B2 |
| Copy ▾: Plain text · Markdown · Rich text · Image · Text from image · Citation | "Copy ▾" in the pane's metadata line | Same | A2, B2 |
| Date · type · location "n of m" · collections · tags · colour | Pane metadata line | Same | A2, B2 |
| ✦ Find related (⌘⇧F) | Pane action bar, violet | Same | A2, B2 |
| Show work highlights → (⌘⇧L) | Pane action bar | Same | A2, B2 |
| ⧉ New window (⌘⇧N) | Pane action bar | Same | A2, B2 |
| "⌘⇧C copies" preview and citation format (quick finder) | Pane foot, now also for highlights | Same | A2, B2 |

## Footer

| Classic control | HS-M1A | HS-M1B | Frame |
|---|---|---|---|
| "n shown" / total, import status and progress | Footer left, after Refresh | Same | all |
| Key hints | Footer right; highlight scope shows ⏎ source and ⌘⇧L work | Same | A2, B2 |
| Version button (release notes) | Footer right end | Same | – |

## Keyboard-only commands (Settings → Shortcuts, command palette)

All keep their bindings in both directions. In HS-M1A, a highlight-only command pressed under All corpora first switches the rail to Highlights. In HS-M1B, it ticks Highlights if it was unticked.

| Command | Key | Change |
|---|---|---|
| Focus search box | ⌘L | none |
| Next / previous result | ↓ / ↑ | none |
| Next / previous group | ⌥↓ / ⌥↑ | now steps through work groups too, not only corpus groups |
| Open source | ⏎ | none |
| Copy as plain text | ⌘C | none |
| Copy as Markdown / quote + citation | ⌘⇧C | one behaviour for every corpus: quote + citation |
| Copy as rich text · Copy image · Copy text from image | unbound | none |
| Copy citation | ⌥⌘C | none |
| Actions on the selected result | ⌘K | none |
| Show work highlights · Open work in new window · Open work Markdown file | ⌘⇧L · ⌘⇧N · ⌘⇧O | none |
| Find related highlights | ⌘⇧F | none |
| Toggle reading pane | ⌘\\ | none |
| Cycle sort · group · row density | ⌘⇧S · ⌘⇧G · ⌘⇧D | none |
| Filter by tag · Open filters · Clear colour filter | ⌘⇧T · ⌘⇧I · ⌘⇧X | see the note above this table |
| Command palette · Keyboard shortcuts · Settings | ⌘⇧P · ? · ⌘, | none |
| Update from Readwise · Import Zotero | ⌘R · ⌘⇧Z | none |

## Query grammar (@scout/query v0.2.1)

Unchanged in both directions, in every scope: 1 word as-is · 2 words AND · 3+ words OR by coverage · `AND` `OR` `|` · `-exclude` · `"phrase"` · `prefix*` · `/regex/flags` · `au:` `author:` · `ti:` `title:` · `ty:` `type:` (and `art:` `bo:` `tw:` `pdf:` `pod:` `sup:`) · `tag:` · `y:` `date:` `d:` · `after:` `since:` `from:` · `before:` `until:` `to:` · `i:` (has image) · `zo:` · `source:` · `co:` `color:` `colour:` · `in:` · `lang:` · `genre:`.

Tokens that only highlights have (`co:`, `i:`, `zo:`, `tag:`, `ty:`) narrow the results to highlights when typed under All corpora.

## Count

The eight control tables have 45 rows, counting three quick-finder controls that the merge also has to keep (row copy actions, the ⌘⇧C preview, the "Classic search" button). Both directions keep 43 of the 45, and every keyboard command and grammar token. Two rows are removed on purpose, because they are the switch between the two searches: the Highlights / Writing · Tweets · Highlights scope toggle and the "Classic search" button.
