import { describe, expect, test } from "vitest";
import { COMMANDS, eventToCombo } from "./keybindings";

const ev = (o: Partial<KeyboardEvent>) => ({ key: "", code: "", metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, ...o }) as KeyboardEvent;

describe("default keymap", () => {
  const byCombo = new Map(COMMANDS.filter((c) => c.default).map((c) => [c.default, c.id]));

  test("no two commands share a default chord", () => {
    const combos = COMMANDS.map((c) => c.default).filter(Boolean);
    expect(new Set(combos).size).toBe(combos.length);
  });

  test("estate chords (ADR-0011) keep their surfaces: ⌘⇧K is never repurposed, ⌘K is contextual actions", () => {
    expect(byCombo.get("Mod+Shift+K")).toBeUndefined();
    expect(byCombo.get("Mod+K")).toBe("rowActions");
    expect(byCombo.get("Mod+Shift+P")).toBe("openPalette");
    expect(byCombo.get("Mod+,")).toBe("openSettings");
    expect(byCombo.get("Mod+P")).toBeUndefined();
    expect(byCombo.get("Mod+F")).toBeUndefined();
  });

  test("quick finder chords: ⌘C quote, ⌘⇧C quote + citation, ⌥⌘C citation, ⌥↓/⌥↑ next and previous corpus", () => {
    expect(byCombo.get("Mod+C")).toBe("copyHighlight");
    expect(byCombo.get("Mod+Shift+C")).toBe("copyMarkdown");
    expect(byCombo.get("Mod+Alt+C")).toBe("copyCitation");
    expect(byCombo.get("Alt+ArrowDown")).toBe("nextGroup");
    expect(byCombo.get("Alt+ArrowUp")).toBe("prevGroup");
  });

  test("every Classic shortcut keeps its key in the one search (round-2 classic-controls table)", () => {
    const table: Record<string, string> = {
      focusSearch: "Mod+L", nextResult: "ArrowDown", prevResult: "ArrowUp", nextGroup: "Alt+ArrowDown", prevGroup: "Alt+ArrowUp",
      openSource: "Enter", copyHighlight: "Mod+C", copyMarkdown: "Mod+Shift+C", copyRichText: "", copyImage: "", copyImageText: "",
      copyCitation: "Mod+Alt+C", rowActions: "Mod+K", openWorkView: "Mod+Shift+L", openWorkWindow: "Mod+Shift+N", openWorkMarkdown: "Mod+Shift+O",
      findRelated: "Mod+Shift+F", togglePane: "Mod+\\", cycleSort: "Mod+Shift+S", cycleGroup: "Mod+Shift+G", cycleDensity: "Mod+Shift+D",
      openTags: "Mod+Shift+T", openFilters: "Mod+Shift+I", clearColor: "Mod+Shift+X", openPalette: "Mod+Shift+P", openHelp: "?",
      openSettings: "Mod+,", importUpdate: "Mod+R", importZotero: "Mod+Shift+Z",
    };
    expect(Object.fromEntries(COMMANDS.map((c) => [c.id, c.default]))).toEqual(table);
  });

  test("⌥ chords are read from the physical key, not the composed glyph", () => {
    expect(eventToCombo(ev({ key: "ç", code: "KeyC", metaKey: true, altKey: true }))).toBe("Mod+Alt+C");
    expect(eventToCombo(ev({ key: "ArrowDown", code: "ArrowDown", altKey: true }))).toBe("Alt+ArrowDown");
    expect(eventToCombo(ev({ key: "C", code: "KeyC", metaKey: true, shiftKey: true }))).toBe("Mod+Shift+C");
    expect(eventToCombo(ev({ key: "Alt", code: "AltLeft", altKey: true }))).toBeNull();
  });
});
