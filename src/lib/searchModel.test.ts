import { describe, expect, test } from "vitest";
import { EMPTY_FILTERS } from "@scout/query";
import {
  DEFAULT_STATE,
  archiveRequests,
  buttonLabel,
  chips,
  clearAll,
  clearChip,
  clearFilters,
  cycleGroup,
  effectiveCorpora,
  effectiveGroup,
  effectiveSubgroup,
  engineFor,
  filterCount,
  groupOptions,
  highlightPayload,
  highlightSearchable,
  loadView,
  mergeResults,
  onlyCorpus,
  saveView,
  scopeNote,
  scopeWords,
  setMode,
  sourceTicked,
  toggleCorpus,
  toggleSource,
  withTag,
  type SearchState,
} from "./searchModel";
import { results, doc } from "./quickFinder.fixtures";

const KNOWN = ["x", "readwise", "zotero"];
const S = (o: Partial<SearchState> = {}): SearchState => ({ ...DEFAULT_STATE, ...o, filters: { ...EMPTY_FILTERS, ...(o.filters ?? {}) } });

function memStore(init: Record<string, string> = {}) {
  const m = new Map(Object.entries(init));
  return { getItem: (k: string) => m.get(k) ?? null, setItem: (k: string, v: string) => void m.set(k, v), m };
}

describe("the rail's tick boxes", () => {
  test("tick and untick corpora; the last one stays ticked", () => {
    let s = S();
    s = toggleCorpus(s, "tweets");
    expect(s.corpora).toEqual(["writing", "highlights"]);
    s = toggleCorpus(toggleCorpus(s, "writing"), "highlights");
    expect(s.corpora).toEqual(["highlights"]);
    s = toggleCorpus(s, "tweets");
    expect(s.corpora).toEqual(["tweets", "highlights"]); // rail order
  });

  test("sources: untick one, tick one while Highlights is off, all off unticks Highlights", () => {
    let s = toggleSource(S(), "zotero", KNOWN);
    expect(s.offSources).toEqual(["zotero"]);
    expect(sourceTicked(s, "zotero")).toBe(false);
    expect(sourceTicked(s, "x")).toBe(true);
    s = toggleSource(toggleSource(s, "x", KNOWN), "readwise", KNOWN);
    expect(s.corpora).toEqual(["writing", "tweets"]);
    expect(s.offSources).toEqual([]);
    s = toggleSource(s, "readwise", KNOWN);
    expect(s.corpora).toEqual(["writing", "tweets", "highlights"]);
    expect(s.offSources).toEqual(["x", "zotero"]);
    // Only Highlights ticked: its last source cannot be unticked.
    const only = S({ corpora: ["highlights"], offSources: ["x", "zotero"] });
    expect(toggleSource(only, "readwise", KNOWN)).toBe(only);
  });

  test("show all narrows to that corpus with every source", () => {
    const s = onlyCorpus(S({ offSources: ["x"] }), "writing");
    expect(s.corpora).toEqual(["writing"]);
    expect(s.offSources).toEqual([]);
  });
});

describe("Keyword / Semantic", () => {
  test("semantic unticks Writing and Tweets and says so; keyword gives them back", () => {
    const s = setMode(S(), "semantic");
    expect(s.corpora).toEqual(["highlights"]);
    expect(chips(s, "q").map((c) => c.id)).toContain("semantic");
    expect(scopeNote(s, "q")).toBe("Highlights only · ⌘⇧I edits");
    const back = clearChip(s, "semantic");
    expect(back.mode).toBe("keyword");
    expect(back.corpora).toEqual(["writing", "tweets", "highlights"]);
  });

  test("ticking Writing under semantic switches back to keyword", () => {
    const s = toggleCorpus(setMode(S(), "semantic"), "writing");
    expect(s.mode).toBe("keyword");
    expect(s.corpora).toEqual(["writing", "highlights"]);
  });

  test("the view remembers the ticks semantic will give back", () => {
    const st = memStore();
    saveView(setMode(S(), "semantic"), st);
    expect(loadView(st).corpora).toEqual(["writing", "tweets", "highlights"]);
    expect(loadView(st).mode).toBe("keyword");
  });
});

describe("which engine answers", () => {
  test("more than highlights: the corpus engine; highlights alone: the highlight index", () => {
    expect(engineFor(S(), "paths")).toBe("archive");
    expect(engineFor(S({ corpora: ["writing"] }), "paths")).toBe("archive");
    expect(engineFor(S({ corpora: ["highlights"] }), "paths")).toBe("highlights");
  });

  test("a highlight filter or token narrows to highlights, and the note says why", () => {
    const red = S({ color: "red" });
    expect(effectiveCorpora(red, "paths")).toEqual(["highlights"]);
    expect(engineFor(red, "paths")).toBe("highlights");
    expect(scopeNote(red, "paths")).toMatch(/narrow the results to highlights/);
    for (const q of ["x co:red", "x i:", "x zo:", 'x tag:"ai"', "x ty:books"]) expect(engineFor(S(), q)).toBe("highlights");
    expect(engineFor(S(), "x au:lakoff y:2020 in:writing")).toBe("archive");
    expect(engineFor(S({ filters: { ...EMPTY_FILTERS, time: "t:30d" } }), "x")).toBe("archive"); // time is any-corpus
  });

  test("the search box names what it searches", () => {
    expect(scopeWords(["writing", "tweets", "highlights"])).toBe("writing, tweets and highlights");
    expect(scopeWords(["highlights"])).toBe("highlights");
  });
});

describe("the corpus engine's requests", () => {
  test("every corpus ticked is one request over all; fewer names them; time becomes after:", () => {
    expect(archiveRequests(S(), "paths metaphor", 50, KNOWN)).toEqual([{ query: "paths metaphor", in: [], limit: 50 }]);
    expect(archiveRequests(S({ corpora: ["writing", "tweets"] }), "paths", 50, KNOWN)).toEqual([{ query: "paths", in: ["writing", "tweets"], limit: 50 }]);
    const [r] = archiveRequests(S({ corpora: ["writing"], filters: { ...EMPTY_FILTERS, time: "t:12m" } }), "paths", 50, KNOWN);
    expect(r.query).toMatch(/^paths after:\d{4}-\d{2}-\d{2}$/);
  });

  test("a partial set of sources is its own request, so writing and tweets are not dropped", () => {
    const s = S({ corpora: ["writing", "highlights"], offSources: ["zotero"] });
    expect(archiveRequests(s, "paths", 50, KNOWN)).toEqual([
      { query: "paths", in: ["writing"], limit: 50 },
      { query: "paths source:x,readwise", in: ["highlights"], limit: 50 },
    ]);
  });

  test("merged answers keep request order and sum the totals", () => {
    const a = results([doc("writing", "a")], 3);
    const b = results([doc("highlights", "h")], 4);
    const m = mergeResults([a, b])!;
    expect(m.results.map((d) => d.corpus)).toEqual(["writing", "highlights"]);
    expect(m.total_documents).toBe(7);
    expect(mergeResults([a])).toBe(a);
    expect(mergeResults([])).toBeNull();
  });
});

describe("the highlight index request", () => {
  const H = (o: Partial<SearchState> = {}) => S({ corpora: ["highlights"], ...o });

  test("popover filters, colour, sort words and match fold into Classic's payload", () => {
    const p = highlightPayload(H({ color: "red", sort: "newest", partial: true, filters: { ...EMPTY_FILTERS, favorite: true, types: ["books"] } }), "cat", 0, 80, KNOWN)!;
    expect(p.color).toBe("red");
    expect(p.sort).toBe("recent");
    expect(p.favorite).toBe(true);
    expect(p.types).toEqual(["books"]);
    expect(p.fts).toBe("cat*");
    expect(p.sources).toEqual([]);
  });

  test("ticked sources restrict; the Zotero quick filter intersects them", () => {
    expect(highlightPayload(H({ offSources: ["zotero"] }), "cat", 0, 80, KNOWN)!.sources).toEqual(["x", "readwise"]);
    expect(highlightPayload(H({ offSources: ["x"], filters: { ...EMPTY_FILTERS, zotero: true } }), "cat", 0, 80, KNOWN)!.sources).toEqual(["zotero"]);
    expect(highlightPayload(H({ offSources: ["zotero"], filters: { ...EMPTY_FILTERS, zotero: true } }), "cat", 0, 80, KNOWN)).toBeNull();
  });

  test("a typed source: is honoured; filters alone are searchable, an empty box is not", () => {
    expect(highlightPayload(H(), "cat source:readwise", 0, 80, KNOWN)!.source).toBe("readwise");
    expect(highlightSearchable(H(), "")).toBe(false);
    expect(highlightSearchable(H({ color: "red" }), "")).toBe(true);
  });
});

describe("Group, sort and rows", () => {
  test("Corpus is offered, and the default, only while more than one corpus is searched", () => {
    expect(groupOptions(S(), "q")).toEqual(["corpus", "work", "author", "date", "none"]);
    expect(effectiveGroup(S(), "q")).toBe("corpus");
    const w = S({ corpora: ["writing"] });
    expect(groupOptions(w, "q")).not.toContain("corpus");
    expect(effectiveGroup(w, "q")).toBe("none");
    expect(groupOptions(S({ corpora: ["highlights"] }), "q")).toEqual(["work", "author", "date", "tag", "none"]);
  });

  test("⌘⇧G cycles the offered groups; then applies to highlights only", () => {
    const s = cycleGroup(S(), "q");
    expect(effectiveGroup(s, "q")).toBe("work");
    const h = S({ corpora: ["highlights"], group: "work", subgroup: "date" });
    expect(effectiveSubgroup(h, "q")).toBe("date");
    expect(effectiveSubgroup({ ...h, corpora: ["writing", "highlights"] }, "q")).toBe("none");
  });
});

describe("chips ↔ state", () => {
  test("every non-default choice is a chip, and each × undoes exactly it", () => {
    const s = S({
      corpora: ["highlights"], mode: "semantic", color: "red", group: "work", subgroup: "date", sort: "oldest", density: "full", partial: true,
      filters: { favorite: true, zotero: true, hasImage: true, types: ["books", "pdfs"], time: "t:6m" },
    });
    const list = chips(s, "q");
    expect(list.map((c) => c.id)).toEqual(["color", "group", "subgroup", "sort", "density", "semantic", "favorite", "zotero", "hasImage", "type:books", "type:pdfs", "time"]);
    expect(list.find((c) => c.id === "group")!.label).toBe("Group: Work");
    for (const c of list) {
      const after = chips(clearChip(s, c.id), "q").map((x) => x.id);
      expect(after).not.toContain(c.id);
      // Clearing Group also drops its "then"; leaving semantic shows the Match chip it hid.
      expect(after.length).toBe(list.length - 1 - (c.id === "group" ? 1 : 0) + (c.id === "semantic" ? 1 : 0));
    }
    expect(chips(clearAll(s, "q"), "q")).toEqual([]);
    expect(clearAll(s, "q").corpora).toEqual(["highlights"]); // ticks stay
    expect(chips(S(), "q")).toEqual([]);
  });

  test("Match: partial shows only in keyword mode", () => {
    expect(chips(S({ partial: true }), "q").map((c) => c.id)).toEqual(["partial"]);
  });

  test("the button counts filters (not the view) and names the group", () => {
    const s = S({ corpora: ["highlights"], color: "red", group: "work" });
    expect(filterCount(s)).toBe(1);
    expect(buttonLabel(s, "q")).toBe("Filters (1) · Group: Work");
    expect(buttonLabel(S(), "q")).toBe("Filters · Group: Corpus");
    expect(filterCount(clearFilters({ ...s, filters: { ...EMPTY_FILTERS, favorite: true, time: "t:30d" } }))).toBe(0);
  });

  test("a picked tag goes into the query, as Classic's tag picker did", () => {
    expect(withTag("", "ai")).toBe('tag:"ai"');
    expect(withTag("metaphor ", "ai")).toBe('metaphor tag:"ai"');
  });
});

describe("persistence", () => {
  test("round trip of the view; bad or old values fall back to the defaults", () => {
    const st = memStore({ group: "work", density: "comfortable" }); // pre-merge keys are ignored
    expect(loadView(st)).toMatchObject({ group: "corpus", density: "compact", corpora: ["writing", "tweets", "highlights"] });
    saveView(S({ corpora: ["tweets", "highlights"], offSources: ["x"], group: "author", sort: "newest", density: "full", partial: true }), st);
    expect(loadView(st)).toMatchObject({ corpora: ["tweets", "highlights"], offSources: ["x"], group: "author", sort: "newest", density: "full", partial: true, color: null, mode: "keyword" });
    st.m.set("search.view", "{not json");
    expect(loadView(st).corpora).toEqual(["writing", "tweets", "highlights"]);
    st.m.set("search.view", JSON.stringify({ corpora: [], group: "bogus" }));
    expect(loadView(st)).toMatchObject({ corpora: ["writing", "tweets", "highlights"], group: "corpus" });
  });
});
