export const APP_VERSION = "0.4.1";

/** Newest first. Bump APP_VERSION and add an entry for every change. */
export const RELEASE_NOTES: Array<{ version: string; notes: string[] }> = [
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
