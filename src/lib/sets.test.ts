import { describe, expect, test } from "vitest";
import {
  acrossLine,
  addedLine,
  applyLocal,
  currentSet,
  loadCurrentSet,
  mixLine,
  moveTarget,
  saveCurrentSet,
  seedRange,
  selectionAfterRemove,
  trayLabel,
  type ResolvedItem,
  type SetItem,
  type SetSummary,
} from "./sets";

const it = (p: string, corpus = "writing", note = ""): ResolvedItem => ({
  item: { passage: p, corpus, source_path: "a.md", quote: "q", pinned: false, note } satisfies SetItem,
  status: "ok",
  cited: null,
});
const ids = (l: ResolvedItem[]) => l.map((r) => r.item.passage);

describe("set tray and headings", () => {
  test("the tray says the set and its size", () => {
    expect(trayLabel("Metaphor talk", 7)).toBe("Metaphor talk · 7 items");
    expect(trayLabel("Metaphor talk", 1)).toBe("Metaphor talk · 1 item");
  });

  test("the heading counts the corpora in the set", () => {
    const items = [...Array(3)].map(() => ({ corpus: "writing" })).concat([{ corpus: "tweets" }], [...Array(3)].map(() => ({ corpus: "highlights" })));
    expect(mixLine(items)).toBe("7 items · 3 writing · 1 tweet · 3 highlights");
    expect(mixLine([])).toBe("0 items");
    expect(mixLine([{ corpus: "highlights" }])).toBe("1 item · 1 highlight");
  });

  test("the added toast", () => {
    expect(addedLine("Metaphor talk", 8, false)).toBe("Added to Metaphor talk · item 8");
    expect(addedLine("Metaphor talk", 8, true)).toBe("Already in Metaphor talk");
  });
});

describe("reordering and editing the page's copy", () => {
  test("⌥↑ and ⌥↓ stop at the ends", () => {
    expect(moveTarget(0, -1, 4)).toBeNull();
    expect(moveTarget(3, 1, 4)).toBeNull();
    expect(moveTarget(1, 1, 4)).toBe(2);
    expect(moveTarget(1, -1, 4)).toBe(0);
  });

  test("move, note, pin and remove match the backend's edits", () => {
    const l = [it("a"), it("b"), it("c")];
    expect(ids(applyLocal(l, { op: "move", from: 2, to: 0 }))).toEqual(["c", "a", "b"]);
    expect(ids(applyLocal(l, { op: "move", from: 0, to: 9 }))).toEqual(["a", "b", "c"]);
    expect(applyLocal(l, { op: "note", index: 1, note: " two\nlines " })[1].item.note).toBe("two lines");
    expect(applyLocal(l, { op: "pin", index: 0, pinned: true })[0].item.pinned).toBe(true);
    expect(ids(applyLocal(l, { op: "remove", index: 1 }))).toEqual(["a", "c"]);
    expect(l[1].item.note).toBe("");
  });

  test("the selection after a removal stays in range", () => {
    expect(selectionAfterRemove(2, 3)).toBe(1);
    expect(selectionAfterRemove(0, 3)).toBe(0);
    expect(selectionAfterRemove(0, 1)).toBe(0);
  });
});

describe("more like these", () => {
  test("the seeds line and the corpora line", () => {
    expect(seedRange(3)).toBe("items 1–3");
    expect(seedRange(14)).toBe("items 1–10");
    expect(seedRange(1)).toBe("item 1");
    expect(acrossLine([{ corpus: "highlights" }, { corpus: "writing" }])).toBe("across writing and highlights");
    expect(acrossLine([{ corpus: "tweets" }])).toBe("across tweets");
    expect(acrossLine([{ corpus: "writing" }, { corpus: "tweets" }, { corpus: "highlights" }])).toBe("across writing, tweets and highlights");
    expect(acrossLine([])).toBe("");
  });
});

describe("the set + Set adds to", () => {
  const sets: SetSummary[] = [{ id: "metaphor-talk", title: "Metaphor talk", kind: "hand", items: 7, updated: "d" }];
  test("the remembered set while it exists", () => {
    expect(currentSet(sets, "metaphor-talk")?.title).toBe("Metaphor talk");
    expect(currentSet(sets, "gone")).toBeNull();
    expect(currentSet(sets, null)).toBeNull();
  });

  test("it is remembered between launches", () => {
    const m = new Map<string, string>();
    const store = { getItem: (k: string) => m.get(k) ?? null, setItem: (k: string, v: string) => void m.set(k, v), removeItem: (k: string) => void m.delete(k) };
    expect(loadCurrentSet(store)).toBeNull();
    saveCurrentSet("metaphor-talk", store);
    expect(loadCurrentSet(store)).toBe("metaphor-talk");
    saveCurrentSet(null, store);
    expect(loadCurrentSet(store)).toBeNull();
  });
});
