export const APP_VERSION = "0.2.3";

/** Newest first. Bump APP_VERSION and add an entry for every change. */
export const RELEASE_NOTES: Array<{ version: string; notes: string[] }> = [
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
