import { describe, expect, test } from "vitest";
import { loadScope, saveScope, type PrefStore } from "./scopePref";

// Stands in for localStorage; one instance is one install's local settings.
function store(initial: Record<string, string> = {}): PrefStore & { data: Map<string, string> } {
  const data = new Map(Object.entries(initial));
  return {
    data,
    getItem: (k) => data.get(k) ?? null,
    setItem: (k, v) => void data.set(k, v),
  };
}

describe("search scope at launch", () => {
  test("a first launch opens the three-corpus quick finder", () => {
    expect(loadScope(store())).toBe("archive");
  });

  test("the last-used scope is what the next launch opens", () => {
    const s = store();
    saveScope("highlights", s);
    expect(loadScope(s)).toBe("highlights");
    saveScope("archive", s);
    expect(loadScope(s)).toBe("archive");
  });

  test("preview.1's automatic classic setting does not keep an upgraded install in classic", () => {
    expect(loadScope(store({ scope: "highlights" }))).toBe("archive");
  });

  test("an unreadable stored value falls back to the quick finder", () => {
    expect(loadScope(store({ searchScope: "semantic" }))).toBe("archive");
  });
});
