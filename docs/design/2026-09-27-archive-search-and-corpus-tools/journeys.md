---
title: "Archive search, referencing and corpus tools: candidate journeys"
date: 2026-09-27
status: shapes picked 2026-09-27 (DTC review); mockups next
apps: [highlight-scout, archive-scout, writeflex]
record: _COORDINATION/highlights/_TASK-LOG/2026-09-27-archive-search-and-analysis-survey.md
---

# Archive search, referencing and corpus tools: candidate journeys

## Dominik's picks (DTC review, 2026-09-27 05:28)

| Journey | Pick | His comment |
|---|---|---|
| J7 ArchiveScout's role | **C:** ArchiveScout is the corpus lab; Highlight Scout stays the quick finder | "archive scout works on any archive it's a general purpose tool I want to release" |
| J1 Quote into a draft | **A and B:** the WriteFlex panel and the hotkey window | |
| J2 Sets | **A, B and C:** tray, saved-search sets, agent "more like these" | |
| J3 Completeness | **B:** silent sync at launch plus a summary toast | |
| J5 Collocations | **A and B:** a Collocates view and a word profile page, both in-app | "a sketchengine for reference corpora only or detailed linguistic analysis" |
| J8 Ask the archive | **B first, then A:** the Claude Code skill first, the in-app rail later | |
| J4, J6, J9 | **Approved as drawn** | |

- He picked none of the "other ideas". They stay in the list below as roadmap candidates, not in scope.
- **Reading of the J5 comment:** collocates and word profiles are built in-app. Sketch Engine is used only for comparing against reference corpora or for detailed linguistic analysis, as an "Open in Sketch Engine" hand-off, not a replacement.
- **Consequence of J7-C:**
  - ArchiveScout is a general-purpose, releasable corpus lab for any archive: survey, clean, KWIC, collocates, n-grams, comparisons and agent reports.
  - Highlight Scout is his quick finder over his own writing, tweets and highlights: search, cite and sets.
  - They share one index, and the CLI and skill sit over it.

### "Right when" lines added for picked candidates that lacked one

- **J2-B:** a saved-search set shows new matching pieces after the next sync, and pinned or unpinned choices survive the update.
- **J2-C:** each suggested item is shown with the passage that made it similar and joins the set only when he approves it.
- **J3-B:** after launch, the toast names the count added per source, and a failed source stays visible as a red line naming the source and the error until a later sync succeeds.
- **J5-A:** every collocate row links to a concordance whose line count equals the row's co-occurrence count, and the writing and highlights columns use the same window and score.
- **J5-B:** the word profile's counts match the Concordance, Collocates and N-grams views for the same word and filters.
- **J7-C:** "Analyse in ArchiveScout" opens the lab on the same query and corpus, with the same hit count as Highlight Scout showed.

These are drafts to be corrected, not decisions. Where a situation has candidates A, B and C, they are different ways of shaping the journey, and one or more can be rejected. The storyboard (`storyboard.html`) draws each state.

## What we know going in (2026-09-27)

- **The sync is switched off, not broken.** Every scheduled sync in Highlight Scout's settings is disabled.
  - Readwise was last pulled on 22 July, and Readwise now holds 732 highlights the archive lacks (72 July, 426 August, 234 September).
  - Zotero was last pulled on 1 July.
  - The installed app is 0.5.4; 0.5.5 was released but never installed.
- **The corpora to start with:**
  - his writing: 1,506 pieces, about 2M words, with genre, topics, summary, lang and date on every piece;
  - his tweets: 14,892;
  - his highlights: 14,724 works (X 9,683, Readwise 4,698, Zotero 343).
- **What he asked for:**
  - a hotkey window and a search panel inside WriteFlex, plus a command-line mode with an agent skill;
  - references as a quote with its citation, a link that opens the source, or a set of finds;
  - analysis as views to browse and as agent reports;
  - corpus tools: KWIC concordance, collocations and n-grams.

## A first look at real numbers (computed 2026-09-27)

A quick pass over the 1,545 English files with genre metadata (1.7M words) found the following.

- "metaphor(s)" occurs 2,628 times. Its strongest collocates (±5 words, logDice) are *generative* (98), *metonymy* (70), *negotiation* (56), *hacking* (48), *conceptual* (60) and *understanding* (78).
- "scaffolding" occurs 64 times in 30 pieces.
- **The corpus needs cleaning before counts can be trusted.** The top n-grams include "amp amp amp" (266: leftover HTML entities), "i don t" (a broken curly apostrophe) and "p tel daily" (a boilerplate line). Czech words split at every accented letter until the tokeniser handles diacritics.
- Every corpus view therefore needs a normalisation step: entities, apostrophes, boilerplate, and Czech tokens and lemmas. This is one more reason the "understand any pile" aim matters even for the writing he has already consolidated.

## Aims

1. **Find and cite.** Anything I have written or saved, I can find in seconds and put into what I am writing, with where it came from.
2. **Complete without thinking.** My archive has everything new, and I can tell at a glance when it doesn't.
3. **See the language.** I can see how words and ideas behave across my corpora: in context, with their collocates, as recurring phrases, and over time.
4. **Understand any pile.** Any folder of documents I point at becomes something I can survey, clean and query with the same tools.
5. **Ask and get evidence.** I can ask a question of an archive and get an answer that cites the passages it rests on.

## Situations

| # | Situation | Aim |
|---|---|---|
| S1 | Mid-sentence in WriteFlex, he remembers writing about "paths" and metaphor years ago | 1 |
| S2 | Preparing a talk, he wants 8–10 of his own lines and sources on metaphor in one place | 1 |
| S3 | He opens Highlight Scout after weeks away and can't tell whether his recent reading is in | 2 |
| S4 | He wonders how he has used "scaffolding" since 2010, and whether it is his word or Vygotsky's | 3 |
| S5 | He wants to see what goes with "metaphor" in his writing, compared with what he reads | 3 |
| S6 | He wants his own recurring phrases, the ones he keeps reaching for | 3 |
| S7 | He points at a pile he has never surveyed (the 19 GB presentations library, or the journal) | 4 |
| S8 | He asks "how did my view of MOOCs change between 2012 and 2020?" | 5 |
| S9 | An agent (Claude Code, the Telegram assistant) needs the same search on his behalf | 1, 5 |

These could be in or out of scope; he decides: showing a colleague a concordance live in a session; a Czech-language query (lemmas matter: *metafora*, *metafory*, *metaforou*); working offline on the Air.

---

## J1. Quote into a draft (S1)

- **A: the WriteFlex side panel.**
  1. ⌃⌘F opens a Search panel on the right; he types `paths metaphor`.
  2. He sees ranked results with the source badge first: *Writing · 2016-06-23 · essay* "Repaved paths and generative metaphors: Expressing human purpose…". The matching sentence is shown highlighted.
  3. ⏎ inserts the quote as a blockquote with a citation line ("— Dominik Lukeš, *Repaved paths and generative metaphors*, 23 June 2016") and a link back to the piece. ⌘⏎ inserts only the link.
  - **Right when:** the draft contains the exact sentence, the citation line and a working link, and the cursor is back in the draft.
- **B: the hotkey window over any app.**
  1. ⌘⌥⇧H opens Highlight Scout over WriteFlex (or Word or Slack).
  2. He searches; ⌘⇧C copies the quote with its citation as Markdown, or as rich text when the target app is not Markdown.
  3. Esc returns to where he was, and he pastes.
  - **Right when:** the pasted text is the same in WriteFlex, Word and Slack, formatted for each.
- **C: inline recall while typing.** He types `[[?paths metaphor` in the editor, and a dropdown of three matching passages appears at the cursor. Picking one inserts a quote and link. Cheapest in keystrokes, but WriteFlex only.

## J2. Collect a set for a talk (S2)

- **A: a set tray in the search window.**
  1. Each result has "+ Set" (⌘S). A tray at the bottom shows *Metaphor talk · 7 items*.
  2. He opens the set: an ordered list mixing his writing, his tweets and highlights (Lakoff & Johnson, *Metaphors We Live By*, from Readwise). He can drag to reorder and add a note per item.
  3. Export: as a Markdown piece in WriteFlex, as quote slides in TalkWeaver, or as a citation list.
  - **Right when:** the set is saved as a file in the archive, reopens with the same order and notes, and each item still opens its source.
- **B: a set grows from a saved search.** He saves `metaphor -tweets after:2015` as a set, then pins or unpins results. The set updates when new pieces match.
- **C: agent-assisted.** He picks 3 seeds, asks "find more like these across my writing and highlights", and approves the suggestions one by one.

## J3. Is my archive complete? (S3)

- **A: a status line with a stale warning.**
  1. At the top of the window: *Readwise: 732 new since 22 July · sync is off* in amber, and *Zotero: last 1 July*.
  2. "Sync now" runs every source and shows a count per source as it goes. A settings toggle turns on daily sync.
  - **Right when:** every source shows a last-synced time within its interval, or an amber line says which is stale and why.
- **B: sync at launch with a summary.** Syncing runs silently when the app opens. A toast reports "Added 732 highlights, 14 saved tweets, 3 Zotero items", and a failure keeps a red line until it is fixed.
- **C: a push to him.** The morning briefing on Telegram says "Highlight Scout hasn't synced Readwise for 8 days, with a Sync now button". This follows the attention rule that pull surfaces are not seen, so a failure should come to him.

## J4. KWIC concordance (S4)

1. He searches `scaffolding` and switches the view from Results to Concordance (⌘2). He sees KWIC lines centred on the keyword, the left and right context aligned, and the source, year and genre in a left column. For example: "…equation of ZPD with any assisted task or with **scaffolding** in general. Sources: Vygotsky…"
2. He sorts by the first word to the right (R1), then by the first to the left (L1). Filters: corpus (writing / tweets / highlights), year range, genre, language. The header count reads *30 pieces · 64 lines* (the real figure for his English writing).
3. Clicking a line opens the passage in the reading pane. "Copy line" copies it with its citation, and "+ Set" adds it to a set.
4. A distribution strip above the lines shows hits per year and per corpus, so he can see "his word" against "his sources' word".
- **Right when:** the line count equals the number of hits across the chosen corpora, sorting is stable, and every line opens at the exact passage.

## J5. Collocations and comparison (S5)

- **A: a Collocates view.** For `metaphor`: a table of collocates in a ±5 window, with a choice of score (logDice or MI), frequency and a KWIC link per row, The real top collocates in his English writing are *generative, hacking (from "metaphor hacking"), metonymy, negotiation, conceptual, understanding, explain*. A second column runs the same list on his highlights, and a "compare" toggle shows the words over- and under-represented in his writing against his reading.
- **B: a word profile page.** One page per word: frequency over time, top collocates by relation (adjective + metaphor, metaphor + of + N, verb + metaphor), top n-grams containing it, and the pieces that use it most. A word sketch, on his own corpus.
- **C: hand it to Sketch Engine.** "Open in Sketch Engine" uploads the chosen subcorpus there and gives the full word sketch, CQL and statistics without rebuilding them. The app keeps only KWIC, simple collocates and n-grams locally.

## J6. My recurring phrases (S6)

1. The N-grams view over his writing: 3- to 5-grams by frequency, with stopword-only grams filtered out. The real top phrases in his English writing include "large language models" (364), "in the same way" (225), "at the same time" (223), "being able to" (203), "the difference between" (152), "make sense of" (116) and "a foreign language" (112). Rows he adds to a phrase list stay findable.
2. A "since" slider, e.g. 2020 onwards, shows which phrases are new and which have faded.
3. Clicking a phrase opens its concordance (J4).
- **Right when:** each count links to exactly that many concordance lines.

## J7. Survey a new pile (S7): where ArchiveScout fits

- **A: ArchiveScout stays its own app for messy piles.**
  1. He points ArchiveScout at `dominiks-presentations-library`.
  2. The existing Analysis view: *19 GB, about 53k files; 1,949 in _PRESENTATIONS*, then the split into text, media and needs-extraction, with word counts. The survey has not been run on this pile yet, so the split is left blank here rather than guessed.
  3. Triage what goes in, convert what needs conversion, then "Index for search". The pile becomes a corpus that Highlight Scout, the WriteFlex panel and the CLI can all search, with the same KWIC, collocate and n-gram views.
  - **Right when:** the new corpus appears as a filter chip in Highlight Scout with its piece count.
- **B: merge them, with "Add a folder…" in Highlight Scout.** ArchiveScout's survey and triage become a sheet inside Highlight Scout: one app, with corpora as its first-class idea. ArchiveScout the app is retired, and its code moves in.
- **C: ArchiveScout becomes the corpus lab, and Highlight Scout stays the quick finder.** Highlight Scout does search, cite and sets. ArchiveScout owns everything analytical: survey, KWIC, collocates, n-grams, comparisons and agent reports, for any corpus including the writing corpus. They share one index.

## J8. Ask the archive (S8)

- **A: the rail in the app.** He asks the question in a docked rail. The agent searches with the same tools (concordance, date filters) and returns a report: three phases (2012 "backlash" essays, 2015 retreat, 2020 "hype bubbles" retrospective), each claim with linked quotes. The report is saved beside the corpus.
- **B: from Claude Code, through the skill.** He asks in any session; the agent calls the command-line mode and writes an HTML report with verified quotes. This is the August archive council made repeatable.
- **C: from Telegram.** "Ask my archive: …" goes to the assistant on the mini, and a short answer with 3 cited quotes comes back, plus a link to the full report.
- **Right when:** every quote in the report is found verbatim in the corpus by the checker, and each has a working link.

## J9. Agents use the same search (S9)

An agent in a terminal:

```
$ scout search "paths metaphor" --in writing --cite
1. Repaved paths and generative metaphors… (essay, 2016-06-23)
   "…" — writing/2016/repaved-paths.md#L41

$ scout kwic scaffolding --in writing,highlights --sort R1 --json
$ scout collocates metaphor --window 5 --score logdice --top 30
$ scout ngrams --in writing --n 3-5 --since 2020
$ scout sync --status
readwise   732 new · last 2026-07-22 · off
```

- **Right when:** the CLI and the app return the same hits for the same query, and a `scout` skill tells agents when and how to use it.

---

## Other ideas to accept or reject

- **"Said it before."** While he writes in WriteFlex, a quiet margin marker says "you wrote a close version of this sentence in 2016", with a link.
- **Read → wrote lineage.** For a piece of his writing, show the highlights he saved on the same topic in the months before he wrote it.
- **Compare two slices (keyness).** For example, writing 2010–2015 against 2020–2026, or his writing against his highlights. The words and phrases that distinguish each side, each linked to its concordance.
- **Topic-over-time view.** A timeline built on the existing topics and genre metadata, showing when each topic peaks in his writing and in his reading.
- **On this day.** One piece or highlight from this date in past years, delivered by push (Telegram), since a pull surface would not be seen.
- **Czech-aware search.** Lemma matching for Czech (*metafora* matches *metafory*, *metaforou*), so Czech pieces are not undercounted.
- **Quote-check for any draft.** Paste a draft; every quotation in it is checked against the archive and flagged if it is not verbatim.
- **Sets become TalkWeaver quote slides** directly, with the source line on each slide.
- **A Raycast command** kept alive over the same index (the current extension points at a dead path).

## Open for Dominik

1. Which candidates are wrong, per journey?
2. J7 is the big one: should ArchiveScout stay separate (A), be merged into Highlight Scout (B), or become the corpus lab (C)?
3. Which "other ideas" are in?
