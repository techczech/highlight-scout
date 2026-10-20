// The one command registry. The menu bar, the ⌘/ cheat sheet, the command palette, the Shortcuts page of the
// Settings window and the key handler all read it; nothing else names a chord. commands.json is the single list
// (the Rust side reads the same file), so a command is added there and nowhere else.
import { chordFromEvent, defineRegistry, dispatchKey, type Bindings, type CommandDef, type KeyContext, type KeyLike, type MenuDecl } from "@bridle/commands";
import commandsJson from "./commands.json";
import schemaJson from "./schema.json";
import { defineSchema } from "@bridle/settings";

export type Group = "Navigation" | "Highlight" | "View" | "Import" | "App";

export const DEFS = commandsJson as CommandDef<Group, void>[];

export const registry = defineRegistry<Group, void>(DEFS, { overlayIds: ["palette", "settings", "cheat"] });

/** App menus; the frame supplies App, Edit, View's trailing items, Window and Help. */
export const MENUS: MenuDecl<Group>[] = [
  { label: "Highlight", groups: ["Highlight"] },
  { label: "Go", groups: ["Navigation"] },
  { label: "View", groups: ["View"] },
  { label: "Import", groups: ["Import"] },
];

export const schema = defineSchema(schemaJson as Parameters<typeof defineSchema>[0]);

/** Keys that also run a command although the registry binds another chord: "?" for the cheat sheet, as before. */
export const KEY_ALIASES: Readonly<Record<string, { id: string; show: string }>> = {
  "⇧/": { id: "cheat", show: "?" },
};

/** The command a key press runs: the registry's binding, else an alias (outside text fields and overlays). */
export function commandForEvent(e: KeyLike & { key?: string }, bindings: Bindings, ctx: KeyContext): string | null {
  const id = dispatchKey(registry, e, bindings, ctx);
  if (id || ctx.inField || ctx.overlay) return id;
  const chord = e.key === "?" ? "⇧/" : chordFromEvent(e);
  return (chord && KEY_ALIASES[chord]?.id) || null;
}

/** Effective chords from the stored overrides (`state.keys` of the settings view). */
export function bindingsFrom(overrides: Record<string, string> | undefined): Bindings {
  return registry.resolveBindings(overrides ?? {}).bindings;
}

/** The quote copy on ⌘C: macOS keeps ⌘C for Copy, so it is not a registry chord; it copies the selected result's passage only when
 *  no text is selected (in the page or in the focused field). With a selection the native copy runs. */
export const PLAIN_COPY = { id: "copy-highlight", show: "⌘C" } as const;

/** Is this key press ⌘C (no other modifier)? */
export function isPlainCopy(e: KeyLike): boolean {
  return e.code === "KeyC" && e.metaKey && !e.ctrlKey && !e.altKey && !e.shiftKey;
}

/** Text selected where the key press lands: the page's selection, or the selected part of a focused text field. */
export function hasSelectedText(target: unknown, pageSelection: string): boolean {
  if (pageSelection) return true;
  const t = target as { selectionStart?: number | null; selectionEnd?: number | null } | null;
  return !!t && typeof t.selectionStart === "number" && typeof t.selectionEnd === "number" && t.selectionEnd > t.selectionStart;
}

/** The command ⌘C runs (the quote copy), or null to let the native copy run. */
export function plainCopyCommand(e: KeyLike, target: unknown, pageSelection: string, ctx: { overlay: boolean }): string | null {
  if (!isPlainCopy(e) || ctx.overlay || hasSelectedText(target, pageSelection)) return null;
  return PLAIN_COPY.id;
}

