// One-time move of what the window kept in its browser storage before shared settings: the text size and the
// shortcut overrides. settings_migrate_local logs them as an app update and leaves any value already set; the old
// storage keys are removed once the host has taken them.
import { invoke } from "@tauri-apps/api/core";
import { canonChord, isValidChord } from "@bridle/commands";

/** The old camelCase command ids to the registry's ids. */
const OLD_IDS: Record<string, string> = {
  focusSearch: "focus-search", nextResult: "next-result", prevResult: "prev-result", nextGroup: "next-group", prevGroup: "prev-group",
  openSource: "open-source", copyHighlight: "copy-highlight", copyMarkdown: "copy-markdown", copyRichText: "copy-rich-text",
  copyImage: "copy-image", copyImageText: "copy-image-text", copyCitation: "copy-citation", rowActions: "row-actions",
  openWorkView: "open-work-view", openWorkWindow: "open-work-window", openWorkMarkdown: "open-work-markdown", findRelated: "find-related",
  togglePane: "toggle-pane", cycleSort: "cycle-sort", cycleGroup: "cycle-group", cycleDensity: "cycle-density", openTags: "open-tags",
  openFilters: "open-filters", clearColor: "clear-color", openPalette: "palette", openHelp: "cheat", openSettings: "settings",
  importUpdate: "import-readwise", importZotero: "import-zotero",
};

/** The old grammar's key names (`event.key`) to the chord grammar's. */
const KEY_NAMES: Record<string, string> = {
  ArrowDown: "↓", ArrowUp: "↑", ArrowLeft: "←", ArrowRight: "→", Enter: "↵", Escape: "Esc",
  Backspace: "⌫", Delete: "⌦", Tab: "⇥", " ": "Space",
};

/** With ⇧ held the old grammar stored the shifted character (US layout); the chord grammar names the physical key and ⇧. */
const SHIFTED: Record<string, string> = {
  "!": "1", "@": "2", "#": "3", $: "4", "%": "5", "^": "6", "&": "7", "*": "8", "(": "9", ")": "0", _: "-", "+": "=",
  "{": "[", "}": "]", "|": "\\", ":": ";", '"': "'", "<": ",", ">": ".", "?": "/", "~": "`",
};

/** "Mod+Shift+C" to "⌘⇧C"; "" stays "" (unbound); null when it is not a chord the registry could hold. */
export function oldComboToChord(combo: string): string | null {
  if (combo === "") return "";
  const parts = combo.split("+");
  // A "+" key itself leaves an empty last part.
  const key = combo.endsWith("++") ? "+" : parts[parts.length - 1];
  const mods = parts.slice(0, combo.endsWith("++") ? -2 : -1);
  let cmd = false, alt = false, shift = false;
  for (const m of mods) {
    if (m === "Mod") cmd = true;
    else if (m === "Alt") alt = true;
    else if (m === "Shift") shift = true;
    else return null;
  }
  let name = KEY_NAMES[key];
  if (!name && SHIFTED[key]) { name = SHIFTED[key]; shift = true; }
  if (!name) name = /^F\d{1,2}$/.test(key) ? key : key.toUpperCase();
  const chord = canonChord((cmd ? "⌘" : "") + (alt ? "⌥" : "") + (shift ? "⇧" : "") + name);
  return isValidChord(chord) ? chord : null;
}

export interface Dropped { oldId: string; combo: unknown; why: string }
export interface LocalMigration { settings: Record<string, unknown>; keys: Record<string, string>; dropped: Dropped[] }

/** What the old storage holds, in the shapes settings_migrate_local takes, and every override that cannot move (with why). */
export function readOldStorage(get: (k: string) => string | null): LocalMigration {
  const settings: Record<string, unknown> = {};
  const size = Number(get("textSize"));
  if (get("textSize") !== null && Number.isFinite(size) && size >= 80) settings["appearance.textSize"] = size;
  const keys: Record<string, string> = {};
  const dropped: Dropped[] = [];
  let ov: Record<string, unknown> = {};
  try {
    const parsed = JSON.parse(get("keybindings") || "{}") as unknown;
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) ov = parsed as Record<string, unknown>;
    else dropped.push({ oldId: "*", combo: parsed, why: "the override list is not a list of shortcuts" });
  } catch {
    dropped.push({ oldId: "*", combo: get("keybindings"), why: "the override list could not be read" });
  }
  for (const [oldId, combo] of Object.entries(ov)) {
    const id = OLD_IDS[oldId];
    const chord = typeof combo === "string" ? oldComboToChord(combo) : null;
    if (!id) dropped.push({ oldId, combo, why: "no such command any more" });
    else if (chord === null) dropped.push({ oldId, combo, why: "the key has no name in the new shortcut grammar" });
    else keys[id] = chord;
  }
  return { settings, keys, dropped };
}

const DONE = "settingsMigrated";

/** Runs once per window storage; safe to call on every launch. */
export async function migrateLocalStorage(storage: Storage = localStorage): Promise<void> {
  if (storage.getItem(DONE)) return;
  const { settings, keys, dropped } = readOldStorage((k) => storage.getItem(k));
  // Every override is either in `keys` or in `dropped` (genuinely unsupported), so the old storage may go once the host took `keys`.
  for (const d of dropped) console.warn(`Settings: the old shortcut for ${d.oldId} (${JSON.stringify(d.combo)}) was not moved: ${d.why}`);
  if (Object.keys(settings).length || Object.keys(keys).length) {
    const r = await invoke<{ ok: boolean }>("settings_migrate_local", { settings, keys });
    if (!r.ok) return;
  }
  storage.removeItem("textSize");
  storage.removeItem("keybindings");
  storage.setItem(DONE, "1");
}
