// Central, remappable keyboard-shortcut registry. Bindings are stored in
// localStorage (overrides over the defaults) and edited in Settings → Shortcuts.
// Combo syntax: "Mod+Shift+C" where Mod = Cmd (macOS) / Ctrl. Non-letter keys
// use their event.key name: "Enter", "ArrowDown", ",".

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
  | "openPalette"
  | "openHelp"
  | "openSettings"
  | "importUpdate"
  | "importSeed"
  | "importZotero"
  | "clearColor";

export interface Command {
  id: CommandId;
  label: string;
  group: string;
  default: string;
}

export const COMMANDS: Command[] = [
  { id: "focusSearch", label: "Focus search box", group: "Navigation", default: "Mod+L" },
  { id: "nextResult", label: "Next result", group: "Navigation", default: "ArrowDown" },
  { id: "prevResult", label: "Previous result", group: "Navigation", default: "ArrowUp" },
  { id: "openSource", label: "Open source", group: "Actions", default: "Enter" },
  { id: "copyHighlight", label: "Copy as plain text", group: "Actions", default: "Mod+C" },
  { id: "copyMarkdown", label: "Copy as Markdown", group: "Actions", default: "Mod+Shift+C" },
  { id: "copyRichText", label: "Copy as rich text", group: "Actions", default: "" },
  { id: "copyImage", label: "Copy image", group: "Actions", default: "" },
  { id: "copyCitation", label: "Copy citation", group: "Actions", default: "Mod+Shift+K" },
  { id: "openWorkView", label: "Show work highlights", group: "Actions", default: "Mod+Shift+L" },
  { id: "openWorkWindow", label: "Open work in new window", group: "Actions", default: "Mod+Shift+N" },
  { id: "openWorkMarkdown", label: "Open work Markdown file", group: "Actions", default: "Mod+Shift+O" },
  { id: "findRelated", label: "Find related highlights", group: "Actions", default: "Mod+Shift+F" },
  { id: "togglePane", label: "Toggle reading pane", group: "View", default: "Mod+\\" },
  { id: "cycleSort", label: "Cycle sort", group: "View", default: "Mod+Shift+S" },
  { id: "cycleGroup", label: "Cycle group", group: "View", default: "Mod+Shift+G" },
  { id: "cycleDensity", label: "Cycle row density", group: "View", default: "Mod+Shift+D" },
  { id: "openTags", label: "Filter by tag", group: "View", default: "Mod+Shift+T" },
  { id: "clearColor", label: "Clear colour filter", group: "View", default: "Mod+Shift+X" },
  { id: "openPalette", label: "Command palette", group: "App", default: "Mod+Shift+P" },
  { id: "openHelp", label: "Keyboard shortcuts", group: "App", default: "?" },
  { id: "openSettings", label: "Open settings", group: "App", default: "Mod+," },
  { id: "importUpdate", label: "Update from Readwise", group: "Import", default: "Mod+R" },
  { id: "importSeed", label: "Seed from Readwise archive", group: "Import", default: "Mod+Shift+R" },
  { id: "importZotero", label: "Import Zotero", group: "Import", default: "Mod+Shift+Z" },
];

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

/** combo → commandId, for fast dispatch. */
export function comboMap(): Record<string, CommandId> {
  const map: Record<string, CommandId> = {};
  const bindings = resolveBindings();
  for (const c of COMMANDS) {
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
