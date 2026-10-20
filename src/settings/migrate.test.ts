import { describe, expect, test } from "vitest";
import { oldComboToChord, readOldStorage } from "./migrate";
import { registry } from "./registry";

describe("moving the old browser-storage values", () => {
  test("old combos become chords", () => {
    expect(oldComboToChord("Mod+Shift+C")).toBe("⌘⇧C");
    expect(oldComboToChord("Mod+Alt+C")).toBe("⌘⌥C");
    expect(oldComboToChord("Alt+ArrowDown")).toBe("⌥↓");
    expect(oldComboToChord("Enter")).toBe("↵");
    expect(oldComboToChord("Mod+,")).toBe("⌘,");
    expect(oldComboToChord("")).toBe("");
    expect(oldComboToChord("Hyper+X")).toBeNull();
  });

  test("every key the old grammar accepted maps (Sol r2 New 6)", () => {
    expect(oldComboToChord("Mod+Backspace")).toBe("⌘⌫");
    expect(oldComboToChord("Mod+Delete")).toBe("⌘⌦");
    expect(oldComboToChord("Mod+Tab")).toBe("⌘⇥");
    expect(oldComboToChord("Mod+ ")).toBe("⌘Space");
    expect(oldComboToChord("Mod+Alt+Escape")).toBe("⌘⌥Esc");
    expect(oldComboToChord("Mod+ArrowLeft")).toBe("⌘←");
    expect(oldComboToChord("Mod+F5")).toBe("⌘F5");
    expect(oldComboToChord("Mod+\\")).toBe("⌘\\");
    expect(oldComboToChord("Mod+Shift+?")).toBe("⌘⇧/");
    expect(oldComboToChord("?")).toBe("⇧/");
    expect(oldComboToChord("Mod++")).toBe("⌘⇧=");
    expect(oldComboToChord("Mod+Shift+!")).toBe("⌘⇧1");
    expect(oldComboToChord("Mod+Home")).toBeNull(); // no Home key in the chord grammar
  });

  test("an override that cannot move is reported, never silently lost", () => {
    const store: Record<string, string> = { keybindings: JSON.stringify({ copyMarkdown: "Mod+Backspace", cycleSort: "Mod+Home", gone: "Mod+Q", openTags: 7 }) };
    const r = readOldStorage((k) => store[k] ?? null);
    expect(r.keys).toEqual({ "copy-markdown": "⌘⌫" });
    expect(r.dropped.map((d) => d.oldId).sort()).toEqual(["cycleSort", "gone", "openTags"]);
    expect(readOldStorage((k) => (k === "keybindings" ? "{not json" : null)).dropped).toHaveLength(1);
  });

  test("text size and overrides are read, with old ids mapped to registry ids", () => {
    const store: Record<string, string> = { textSize: "145", keybindings: JSON.stringify({ cycleSort: "Mod+Alt+S", importZotero: "", nope: "Mod+Q", openHelp: "Mod+Shift+Z" }) };
    const r = readOldStorage((k) => store[k] ?? null);
    expect(r.settings).toEqual({ "appearance.textSize": 145 });
    expect(r.keys).toEqual({ "cycle-sort": "⌘⌥S", "import-zotero": "", cheat: "⌘⇧Z" });
    for (const id of Object.keys(r.keys)) expect(registry.byId(id)).toBeDefined();
  });

  test("nothing stored, nothing to move; a bad size is ignored", () => {
    expect(readOldStorage(() => null)).toEqual({ settings: {}, keys: {}, dropped: [] });
    expect(readOldStorage((k) => (k === "textSize" ? "12" : null)).settings).toEqual({});
  });
});
