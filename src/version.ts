export const APP_VERSION = "0.6.0-preview.2";

/** Newest first. Bump APP_VERSION and add an entry for every change. */
export const RELEASE_NOTES: Array<{ version: string; notes: string[] }> = [
  {
    version: "0.6.0-preview.2",
    notes: [
      "Preview. Highlight Scout now opens in the search over your writing, your tweets and your highlights. It remembers the search you used last: if you switch to \"Classic search\", the next launch opens there.",
      "Searches find pieces by their title as well as their text, and a word with a bold or italic part inside it is found as one whole word. The archive index rebuilds once in the background after this update; it is not rebuilt again on later launches.",
    ],
  },
  {
    version: "0.6.0-preview.1",
    notes: [
      "Preview. One search over your writing, your tweets and your highlights. Switch to it with \"Writing · Tweets · Highlights\" in the toolbar; \"Classic search\" switches back to the highlight search with its filters and colours.",
      "The left rail picks what to search: all three, one of them, or the highlights from X, Readwise or Zotero, each with its real size. Your recent searches are listed below; click one to run it again. Sets are listed but arrive in the next preview.",
      "Results are grouped Writing, then Tweets, then Highlights. \"show all\" searches that one corpus. ↑↓ moves through the results and ⌥↓ jumps to the next group.",
      "The selected result carries its copy buttons. ⌘C copies the passage; ⌘⇧C copies it with its citation, the full title and both the archive and the public link. It pastes as Markdown into WriteFlex and as formatted text anywhere else; the setting beside the preview can fix it to one or the other.",
      "The reading pane shows the passage with the paragraph before it, the matched sentence marked, and exactly what ⌘⇧C will copy.",
      "Esc hides the window and returns you to the app you came from.",
      "Copy citation in the classic search moved from ⌘⇧K to ⌥⌘C: ⌘⇧K is kept free across all the apps for jumping between things. ⌘K moves to the copy buttons of the selected result.",
      "Settings → Sync is now a table: each source with when it last synced and what the last sync did, or why it failed and when. The sync summary and the red failure line have a new look, with Details, Retry and Sync settings buttons.",
    ],
  },
  {
    version: "0.5.8",
    notes: [
      "Merging the duplicate files that older versions left for some Readwise books no longer leaves two copies of the same highlight. When the same highlight appears in both files, it becomes one entry: the tags from both are combined, both notes are kept if they differ, and nothing is lost.",
      "The same passage highlighted on different dates stays as separate highlights.",
    ],
  },
  {
    version: "0.5.7",
    notes: [
      "Readwise highlights sync again. Readwise changed how it numbers the pages of a large export, and Highlight Scout could not read the reply; all pages are now fetched.",
      "When Readwise syncs only what changed, each book's file keeps its earlier highlights and gains the new ones, instead of being replaced by just the changed highlights. Highlights you delete in Readwise are removed from the book's file.",
      "Syncing no longer rewrites files that have not changed. A Zotero, Readwise or saved-tweet item keeps its original import date, and its updated date moves only when its highlights or notes actually change, so the archive stays quiet between syncs.",
      "If a sync reply ever fails to read again, the error names the field that failed.",
    ],
  },
  {
    version: "0.5.6",
    notes: [
      "New highlights arrive on their own: when Highlight Scout opens it syncs Readwise highlights, Readwise saved tweets and Zotero in the background, and the window stays usable while it runs.",
      "A short summary appears when the sync finishes, for example \"Added 12 highlights · no new saved tweets · 3 Zotero items\", or \"Up to date\". Hover it to see when each source last synced.",
      "If a source fails, a red line names it and the reason (for example \"Readwise: token rejected (401)\") until a later sync of that source succeeds; the other sources still sync.",
      "Settings → Sync has one switch (sync when the app opens, on by default), a repeat interval while the app is open (every 6 hours by default), a Sync now button, and each source's last-synced time. The old per-source schedules are gone.",
    ],
  },
  {
    version: "0.5.5",
    notes: [
      "Settings → Sources now uses one local highlights folder; the old Readwise archive seed path is no longer shown.",
      "Readwise imports are API-only, and the local archive can be backed up to or restored from Cloudflare R2.",
      "Backup detects changed files and re-uploads them (not just new ones); restore brings back the backup copy of any file that differs locally.",
      "Restore also replaces the local search index with the backed-up one; the index rebuilds from your archive on the next import if needed.",
    ],
  },
  {
    version: "0.5.4",
    notes: [
      "Text inside images is now searchable: Highlight Scout reads images with on-device OCR (macOS) so you can find tweets and screenshots by the words in the picture.",
      "New \"Text from image\" copy option for image highlights.",
      "Runs automatically on import (toggle in Settings → Sync) — or use Settings → Import → \"OCR images\" to process your existing library.",
    ],
  },
  {
    version: "0.5.3",
    notes: [
      "Tweets imported from Readwise now keep their structure: threads show a divider between each tweet, and quoted/replied tweets appear as an indented quote with the author, image and date.",
      "Re-run Settings → Import → \"Readwise saved tweets\" to refresh existing tweets with the new formatting.",
    ],
  },
  {
    version: "0.5.2",
    notes: [
      "Tweets now render properly: inline images, quoted/reply context as blockquotes, and clickable article links.",
      "Copy any highlight four ways (reading pane Copy menu, or the command palette): plain text, Markdown, rich text (formatted with images, pastable into Word/Pages/Gmail), or the image itself.",
      "⌘C copies plain text, ⌘⇧C copies Markdown; rich text and image copy are in the Copy menu and command palette.",
    ],
  },
  {
    version: "0.5.1",
    notes: [
      "The preview now changes only when you click a result, not when the mouse passes over it.",
      "Default row density is now Comfortable (first 4 lines) instead of Full.",
      "Esc no longer hides or closes the window: it backs out of a work view, overlay or search, and otherwise does nothing.",
      "The global hotkey only summons the window (it never hides it), and the main window stays open while the app runs — closing it quits the app.",
      "Esc in the command palette closes just the palette, not the whole app.",
    ],
  },
  {
    version: "0.5.0",
    notes: [
      "Import your saved tweets from Readwise (Settings → Import → \"Readwise saved tweets\"): full text, images and links.",
      "Scheduled syncs (Settings → Sync): run Readwise highlights, Readwise tweets and Zotero on a recurring schedule while the app is open.",
      "Launch at login is now an opt-in setting (Settings → Sync) instead of always on.",
    ],
  },
  {
    version: "0.4.7",
    notes: [
      "Fixed row density: Compact (2 lines), Comfortable (4 lines) and Full now render distinctly (a CSS display clash had made them identical).",
      "Minimal mode hides the author/year columns when the reading pane is open, showing them only when the pane is closed.",
    ],
  },
  {
    version: "0.4.6",
    notes: [
      "Larger default window so the full toolbar is visible on first launch.",
      "New Text size setting (Settings → Search & view): scale all text up to 210%.",
      "Redefined row density: Minimal (one line + author/year columns), Compact (first 2 lines + author·year·title), Comfortable (first 4 lines), Full (entire quote).",
    ],
  },
  {
    version: "0.4.5",
    notes: [
      "Import lives in Settings now (its own tab) — it's a rare action, so it no longer takes a big header button.",
      "Prominent ⟳ Refresh and ⚙ Settings buttons in the top-right.",
      "Empty state's Import button opens Settings → Import.",
    ],
  },
  {
    version: "0.4.4",
    notes: [
      "Empty state now has a prominent Import button when you have no highlights yet.",
      "Import and Settings moved to the top-right corner (out of the view toolbar).",
      "Refresh: results and counts auto-refresh after an import and when the window is shown, plus a manual ⟳ button.",
    ],
  },
  {
    version: "0.4.3",
    notes: [
      "macOS build is now ad-hoc signed, and install docs explain the first-launch “Open Anyway” step (the app isn't notarised yet).",
    ],
  },
  {
    version: "0.4.2",
    notes: [
      "Fixed the Windows build: SQLite is now bundled (compiled from source) so it links on Windows, not just macOS.",
    ],
  },
  {
    version: "0.4.1",
    notes: [
      "Cross-platform paths: archive defaults to ~/Documents/Highlight Scout (overridable); config/index move to the OS app-data dir so Windows works. Existing installs keep their current location.",
      "Semantic mode shows a friendly “install QMD” banner instead of erroring when QMD isn't present.",
      "Release groundwork: GitHub Actions build/release workflow, MIT licence, public-ready README, and a /docs website.",
    ],
  },
  {
    version: "0.4.0",
    notes: [
      "Universal imports — no Readwise/Zotero needed: Import CSV (with a column-mapping screen + saved per-file mappings), Kindle My Clippings.txt, and JSON.",
      "Export all to JSON (Highlight Scout's own round-trip format).",
      "Re-importing the same file is idempotent (content-hash IDs) — no duplicates.",
      "Groundwork for the open-source release.",
    ],
  },
  {
    version: "0.3.4",
    notes: [
      "Import log: every import (and error) is recorded with time, counts, and duration — open via Import ▾ → View import log.",
      "Rate limits: requests now honour Retry-After and back off on 429 instead of failing.",
      "Confirmed the /export endpoint (240/min) is the right Readwise API — the readwise CLI uses the slower 20/min list endpoint.",
    ],
  },
  {
    version: "0.3.3",
    notes: [
      "Find related now opens its own window: source quote on top, related highlights below with a strength bar + % match.",
      "Semantic results are labelled “keyword + semantic” vs “✦ semantic”, with matched terms highlighted in the list too.",
      "Stop words (the, of, a…) are excluded from keyword matching and highlighting; quoted phrases and prefix* are kept.",
    ],
  },
  {
    version: "0.3.2",
    notes: [
      "Fixed: semantic/find-related crash when the text contained hyphens (QMD read them as negation) — queries are now sanitised.",
    ],
  },
  {
    version: "0.3.1",
    notes: [
      "Semantic search is much faster — uses a typed lex+vec query (skips the slow LLM expansion): ~0.5–1s instead of ~8s.",
      "✦ Find related: button (or ⌘⇧F) on a highlight finds semantically related highlights across your library.",
      "Keyword search: 'Match: whole word / partial' toggle (partial = prefix match, e.g. cat → category).",
    ],
  },
  {
    version: "0.3.0",
    notes: [
      "Semantic search via QMD — switch the mode toggle to Semantic and press ↵ to find highlights by meaning.",
      "Import ▾ → Rebuild semantic index (QMD) builds/refreshes the embeddings over your archive.",
      "Semantic results map back to individual highlights and render like keyword results.",
    ],
  },
  {
    version: "0.2.5",
    notes: [
      "Fixed: new searches now select and scroll to the first row at the top, not a mid-list match; arrow keys start from the top.",
    ],
  },
  {
    version: "0.2.4",
    notes: [
      "Global hotkey is now ⌘⌥⇧H.",
      "Fixed: the window no longer vanishes right after it opens (removed hide-on-blur; it stays until you toggle the hotkey or close it).",
    ],
  },
  {
    version: "0.2.3",
    notes: [
      "Fixed: keyboard shortcuts now work everywhere (global listener, not focus-dependent).",
      "Command palette (⌘⇧P or ?) — search and run any action, shows its shortcut.",
      "Markdown now renders in result rows and the reading pane (bold, italic, code, links).",
      "Reading-pane toggle moved to ⌘\\ (⌘⇧P is now the palette).",
    ],
  },
  {
    version: "0.2.2",
    notes: [
      "Every action now has a keyboard shortcut, all remappable in Settings → Shortcuts.",
      "Settings reorganised into tabs: Sources, Search & view, Shortcuts, About.",
      "Granular import progress with a progress bar and live counts.",
    ],
  },
  {
    version: "0.2.1",
    notes: [
      "Readwise: seed from the existing archive and update incrementally via the export API (fixes the 429 “too many requests” errors).",
      "Zotero: full citations (Copy citation), collections shown as chips, and “Open PDF in Zotero” via zotero:// links.",
      "Grouping shows full author names; added a second “then by” subgroup level.",
      "Open a work in its own window to line up several side by side.",
      "Row density toggle: Compact / Comfortable / Full quotes.",
      "Version shown in the footer and Settings.",
    ],
  },
  {
    version: "0.2.0",
    notes: ["Full Raycast feature parity: query grammar, reading pane, sort/group/scope, tags, work view, settings, Zotero images."],
  },
];
