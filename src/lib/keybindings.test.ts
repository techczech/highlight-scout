import { describe, expect, test } from "vitest";
import { COMMANDS, comboMap, commandForKey, commandsFor, eventToCombo, type Page } from "./keybindings";

const ev = (o: Partial<KeyboardEvent>) => ({ key: "", code: "", metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, ...o }) as KeyboardEvent;

describe("default keymap", () => {
  const PAGES: Page[] = ["search", "set", "suggestions"];
  const byCombo = new Map(commandsFor("search").filter((c) => c.default).map((c) => [c.default, c.id]));

  test("no two commands share a default chord on any page", () => {
    for (const page of PAGES) {
      const combos = commandsFor(page).map((c) => c.default).filter(Boolean);
      expect(new Set(combos).size, page).toBe(combos.length);
    }
  });

  test("the palette, help and settings work on every page, a set's page included", () => {
    for (const page of PAGES) {
      const m = comboMap(page);
      expect(m["Mod+Shift+P"], page).toBe("openPalette");
      expect(m["Mod+,"], page).toBe("openSettings");
      expect(commandsFor(page).map((c) => c.id)).toContain("openHelp");
    }
  });

  test("the set page's keys are registered commands, scoped to it", () => {
    const set = comboMap("set");
    expect(set).toMatchObject({
      ArrowDown: "setNext", ArrowUp: "setPrev", "Alt+ArrowDown": "setMoveDown", "Alt+ArrowUp": "setMoveUp",
      Enter: "setOpenSource", N: "setEditNote", Backspace: "setRemove", Escape: "setBack",
    });
    expect(comboMap("suggestions")).toMatchObject({ ArrowDown: "suggestNext", ArrowUp: "suggestPrev", Y: "suggestAdd", N: "suggestReject", Escape: "suggestStop" });
    // The search keeps its own meaning for the same keys.
    expect(comboMap("search")).toMatchObject({ ArrowDown: "nextResult", Enter: "openSource" });
    expect(comboMap("search").N).toBeUndefined();
    // Each set command is in the palette there, and only there.
    for (const id of ["setMoreLike", "setUpdateMatches", "setRename", "setRemove"] as const) {
      expect(commandsFor("set").map((c) => c.id)).toContain(id);
      expect(commandsFor("search").map((c) => c.id)).not.toContain(id);
    }
  });

  test("typing in a field runs only ⌘ chords: a note never removes its item", () => {
    const set = comboMap("set");
    expect(commandForKey("Backspace", set, true)).toBeNull();
    expect(commandForKey("N", set, true)).toBeNull();
    expect(commandForKey("Escape", set, true)).toBeNull();
    expect(commandForKey("Backspace", set, false)).toBe("setRemove");
    expect(commandForKey("Mod+Shift+P", set, true)).toBe("openPalette");
    expect(commandForKey(eventToCombo(ev({ key: "n", code: "KeyN" })), set, false)).toBe("setEditNote");
    expect(commandForKey(null, set, false)).toBeNull();
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
    expect(Object.fromEntries(commandsFor("search").filter((c) => c.id !== "addToSet").map((c) => [c.id, c.default]))).toEqual(table);
  });

  test("⌥⌘S adds the selected result to the current set; ⌘S stays free for save", () => {
    expect(byCombo.get("Mod+Alt+S")).toBe("addToSet");
    expect(COMMANDS.filter((c) => c.default === "Mod+S")).toHaveLength(0);
    expect(eventToCombo(ev({ key: "ß", code: "KeyS", metaKey: true, altKey: true }))).toBe("Mod+Alt+S");
  });

  test("⌥ chords are read from the physical key, not the composed glyph", () => {
    expect(eventToCombo(ev({ key: "ç", code: "KeyC", metaKey: true, altKey: true }))).toBe("Mod+Alt+C");
    expect(eventToCombo(ev({ key: "ArrowDown", code: "ArrowDown", altKey: true }))).toBe("Alt+ArrowDown");
    expect(eventToCombo(ev({ key: "C", code: "KeyC", metaKey: true, shiftKey: true }))).toBe("Mod+Shift+C");
    expect(eventToCombo(ev({ key: "Alt", code: "AltLeft", altKey: true }))).toBeNull();
  });
});
