// Central, remappable keyboard-shortcut registry. Bindings are stored in
// localStorage (overrides over the defaults) and edited in Settings → Shortcuts.
// Combo syntax: "Mod+Shift+C" where Mod = Cmd (macOS) / Ctrl. Non-letter keys
// use their event.key name: "Enter", "ArrowDown", ",".
//
// Commands are scoped to the page they act on (`pages`): the search, an open
// set, or a set's suggestions. A command with no `pages` (palette, settings,
// help, imports) works on every page. A chord is unique within each page;
// the same key may mean "next result" on the search and "next item" on a set.

export type CommandId =
  | "focusSearch"
  | "nextResult"
  | "prevResult"
  | "openSource"
  | "copyHighlight"
  | "copyMarkdown"
  | "copyRichText"
  | "copyImage"
  | "copyCitation"
  | "openWorkView"
  | "openWorkWindow"
  | "openWorkMarkdown"
  | "findRelated"
  | "togglePane"
  | "cycleSort"
  | "cycleGroup"
  | "cycleDensity"
  | "openTags"
  | "openFilters"
  | "openPalette"
  | "openHelp"
  | "openSettings"
  | "importUpdate"
  | "importZotero"
  | "clearColor"
  | "copyImageText"
  | "nextGroup"
  | "prevGroup"
  | "addToSet"
  | "rowActions"
  // The set page (HS-2A)
  | "setNext"
  | "setPrev"
  | "setMoveDown"
  | "setMoveUp"
  | "setOpenSource"
  | "setEditNote"
  | "setRemove"
  | "setRename"
  | "setMoreLike"
  | "setUpdateMatches"
  | "setBack"
  // A set's "More like these…" suggestions
  | "suggestNext"
  | "suggestPrev"
  | "suggestAdd"
  | "suggestReject"
  | "suggestStop";

/** The page a command acts on. */
export type Page = "search" | "set" | "suggestions";

export interface Command {
  id: CommandId;
  label: string;
  group: string;
  default: string;
  /** Where it applies; absent = every page. */
  pages?: Page[];
}

const S: Page[] = ["search"];
const SET: Page[] = ["set"];
const SUG: Page[] = ["suggestions"];

export const COMMANDS: Command[] = [
  { id: "focusSearch", label: "Focus search box", group: "Navigation", default: "Mod+L", pages: S },
  { id: "nextResult", label: "Next result", group: "Navigation", default: "ArrowDown", pages: S },
  { id: "prevResult", label: "Previous result", group: "Navigation", default: "ArrowUp", pages: S },
  { id: "nextGroup", label: "Next group (corpus, work, author…)", group: "Navigation", default: "Alt+ArrowDown", pages: S },
  { id: "prevGroup", label: "Previous group", group: "Navigation", default: "Alt+ArrowUp", pages: S },
  { id: "openSource", label: "Open source", group: "Actions", default: "Enter", pages: S },
  { id: "copyHighlight", label: "Copy as plain text", group: "Actions", default: "Mod+C", pages: S },
  { id: "copyMarkdown", label: "Copy quote + citation", group: "Actions", default: "Mod+Shift+C", pages: S },
  { id: "copyRichText", label: "Copy as rich text", group: "Actions", default: "", pages: S },
  { id: "copyImage", label: "Copy image", group: "Actions", default: "", pages: S },
  { id: "copyImageText", label: "Copy text from image", group: "Actions", default: "", pages: S },
  // ⌥⌘C: ⌘⇧K is the estate-wide navigation switcher (ADR-0011), never repurposed.
  { id: "copyCitation", label: "Copy citation", group: "Actions", default: "Mod+Alt+C", pages: S },
  // ⌥⌘S: ⌘S stays the Mac's save; "add to set" is an app action (⌥⌘ family), S for set.
  { id: "addToSet", label: "Add to set", group: "Actions", default: "Mod+Alt+S", pages: S },
  { id: "rowActions", label: "Actions on the selected result", group: "Actions", default: "Mod+K", pages: S },
  { id: "openWorkView", label: "Show work highlights", group: "Actions", default: "Mod+Shift+L", pages: S },
  { id: "openWorkWindow", label: "Open work in new window", group: "Actions", default: "Mod+Shift+N", pages: S },
  { id: "openWorkMarkdown", label: "Open work Markdown file", group: "Actions", default: "Mod+Shift+O", pages: S },
  { id: "findRelated", label: "Find related highlights", group: "Actions", default: "Mod+Shift+F", pages: S },
  { id: "togglePane", label: "Toggle reading pane", group: "View", default: "Mod+\\", pages: S },
  { id: "cycleSort", label: "Cycle sort", group: "View", default: "Mod+Shift+S", pages: S },
  { id: "cycleGroup", label: "Cycle group", group: "View", default: "Mod+Shift+G", pages: S },
  { id: "cycleDensity", label: "Cycle row density", group: "View", default: "Mod+Shift+D", pages: S },
  { id: "openTags", label: "Filter by tag", group: "View", default: "Mod+Shift+T", pages: S },
  { id: "openFilters", label: "Open filters", group: "View", default: "Mod+Shift+I", pages: S },
  { id: "clearColor", label: "Clear colour filter", group: "View", default: "Mod+Shift+X", pages: S },
  { id: "openPalette", label: "Command palette", group: "App", default: "Mod+Shift+P" },
  { id: "openHelp", label: "Keyboard shortcuts", group: "App", default: "?" },
  { id: "openSettings", label: "Open settings", group: "App", default: "Mod+," },
  { id: "importUpdate", label: "Update from Readwise", group: "Import", default: "Mod+R" },
  { id: "importZotero", label: "Import Zotero", group: "Import", default: "Mod+Shift+Z" },
  { id: "setNext", label: "Next item", group: "Set page", default: "ArrowDown", pages: SET },
  { id: "setPrev", label: "Previous item", group: "Set page", default: "ArrowUp", pages: SET },
  { id: "setMoveDown", label: "Move the item down", group: "Set page", default: "Alt+ArrowDown", pages: SET },
  { id: "setMoveUp", label: "Move the item up", group: "Set page", default: "Alt+ArrowUp", pages: SET },
  { id: "setOpenSource", label: "Open the item's source", group: "Set page", default: "Enter", pages: SET },
  { id: "setEditNote", label: "Edit the item's note", group: "Set page", default: "N", pages: SET },
  { id: "setRemove", label: "Remove the item from the set", group: "Set page", default: "Backspace", pages: SET },
  { id: "setRename", label: "Rename the set", group: "Set page", default: "", pages: SET },
  { id: "setMoreLike", label: "More like these…", group: "Set page", default: "", pages: SET },
  { id: "setUpdateMatches", label: "Update matches (a saved search)", group: "Set page", default: "", pages: SET },
  { id: "setBack", label: "Back to the search", group: "Set page", default: "Escape", pages: SET },
  { id: "suggestNext", label: "Next suggestion", group: "Suggestions", default: "ArrowDown", pages: SUG },
  { id: "suggestPrev", label: "Previous suggestion", group: "Suggestions", default: "ArrowUp", pages: SUG },
  { id: "suggestAdd", label: "Add the suggestion to the set", group: "Suggestions", default: "Y", pages: SUG },
  { id: "suggestReject", label: "Not this (it does not come back)", group: "Suggestions", default: "N", pages: SUG },
  { id: "suggestStop", label: "Stop suggesting", group: "Suggestions", default: "Escape", pages: SUG },
];

/** Whether a command applies on a page. */
export function onPage(c: Command, page: Page): boolean {
  return !c.pages || c.pages.includes(page);
}

/** The commands of a page (and the app-wide ones), for the palette and dispatch. */
export function commandsFor(page: Page): Command[] {
  return COMMANDS.filter((c) => onPage(c, page));
}

const STORAGE_KEY = "keybindings";

/** Build the combo string for a keyboard event, or null if it is only modifiers. */
export function eventToCombo(e: KeyboardEvent | React.KeyboardEvent): string | null {
  const key = e.key;
  if (key === "Meta" || key === "Control" || key === "Shift" || key === "Alt") return null;

  const parts: string[] = [];
  if (e.metaKey || e.ctrlKey) parts.push("Mod");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");

  let k = key;
  // With ⌥ held, macOS reports the composed glyph (⌥C = "ç"); use the
  // physical key so "Mod+Alt+C" can be bound.
  if (e.altKey && /^Key[A-Z]$/.test(e.code)) k = e.code.slice(3);
  else if (e.altKey && /^Digit[0-9]$/.test(e.code)) k = e.code.slice(5);
  if (k.length === 1) k = k.toUpperCase();
  // Normalise so Shift+letter reports the letter, not the shifted glyph.
  parts.push(k);
  return parts.join("+");
}

export function comboLabel(combo: string): string {
  return combo
    .replace("Mod", navigator.platform.includes("Mac") ? "⌘" : "Ctrl")
    .replace("Shift", "⇧")
    .replace("Alt", navigator.platform.includes("Mac") ? "⌥" : "Alt")
    .replace("ArrowDown", "↓")
    .replace("ArrowUp", "↑")
    .replace("Enter", "↵")
    .replace("Backspace", "⌫")
    .replace("Escape", "esc")
    .replace(/\+/g, " ");
}

function overrides(): Partial<Record<CommandId, string>> {
  try {
    return JSON.parse(localStorage.getItem(STORAGE_KEY) || "{}");
  } catch {
    return {};
  }
}

/** Resolved bindings: defaults merged with user overrides. */
export function resolveBindings(): Record<CommandId, string> {
  const ov = overrides();
  const out = {} as Record<CommandId, string>;
  for (const c of COMMANDS) out[c.id] = ov[c.id] ?? c.default;
  return out;
}

/** combo → commandId on one page, for fast dispatch. */
export function comboMap(page: Page = "search"): Record<string, CommandId> {
  const map: Record<string, CommandId> = {};
  const bindings = resolveBindings();
  for (const c of commandsFor(page)) {
    const b = bindings[c.id];
    if (b) map[b] = c.id;
  }
  return map;
}

export function setBinding(id: CommandId, combo: string): void {
  const ov = overrides();
  ov[id] = combo;
  localStorage.setItem(STORAGE_KEY, JSON.stringify(ov));
}

export function resetBindings(): void {
  localStorage.removeItem(STORAGE_KEY);
}

/**
 * The command a key runs on a page, or null. In a text field only chords
 * with ⌘ (Ctrl) run a command, so typing a note never removes an item; the
 * field keeps its own Esc.
 */
export function commandForKey(combo: string | null, map: Record<string, CommandId>, inEditable: boolean): CommandId | null {
  if (!combo) return null;
  if (inEditable && !combo.startsWith("Mod+")) return null;
  return map[combo] ?? null;
}
