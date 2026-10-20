import { describe, expect, test } from "vitest";
import { analyseLink, analyseTitle, resultCountLine } from "./archivescout";
import { DEFAULT_STATE, type SearchState } from "./searchModel";

const st = (o: Partial<SearchState> = {}): SearchState => ({ ...DEFAULT_STATE, ...o, filters: { ...DEFAULT_STATE.filters, ...(o.filters ?? {}) } });
const link = (query: string, o: Partial<SearchState> = {}, known: string[] = ["readwise", "x", "zotero"]) => analyseLink(st(o), query, known);

describe("Analyse in ArchiveScout", () => {
  test("the link carries the term and the searched corpora", () => {
    expect(link("scaffolding", { corpora: ["writing"] }).url).toBe("archivescout://explore?mode=kwic&q=scaffolding&in=writing");
    expect(link('"generative metaphor"').url).toBe(
      "archivescout://explore?mode=kwic&q=generative%20metaphor&in=writing,tweets,highlights",
    );
  });

  test("date tokens become ArchiveScout's date filters, not words of the term", () => {
    const l = link("metaphor after:2015", { corpora: ["writing"] });
    expect(l.url).toBe("archivescout://explore?mode=kwic&q=metaphor&in=writing&after=2015");
    expect(l.notCarried).toEqual([]);
    expect(link("metaphor before:2020-06 y:2016", { corpora: ["writing"] }).url).toBe(
      "archivescout://explore?mode=kwic&q=metaphor&in=writing&before=2020-06&y=2016",
    );
    // An inclusive year range, as the engine reads it.
    expect(link("metaphor y:2010-2015", { corpora: ["writing"] }).url).toContain("&after=2010&before=2016");
    expect(link("metaphor lang:en genre:essay", { corpora: ["writing"] }).url).toContain("&lang=en&genre=essay");
    // A typed in: is the engine's corpus choice.
    expect(link("metaphor in:tweets").url).toContain("&in=tweets");
  });

  test("the popover's time window goes along as after:", () => {
    const url = link("metaphor", { corpora: ["writing"], filters: { ...DEFAULT_STATE.filters, time: "t:12m" } }).url!;
    expect(url).toMatch(/&after=\d{4}-\d{2}-\d{2}$/);
  });

  test("what the concordance cannot take is named, never silently dropped", () => {
    const or = link("metaphor OR scaffolding");
    expect(or.term).toBe("metaphor");
    expect(or.url).toContain("q=metaphor&");
    expect(or.notCarried).toEqual(["OR", "more words (scaffolding)"]);
    expect(link("metaphor -bird au:scott /x+/").notCarried).toEqual(["excluded words (-bird)", "regex", "author"]);
    expect(link("metaphor", { color: "yellow", filters: { ...DEFAULT_STATE.filters, favorite: true } }).notCarried).toEqual(["colour", "favourites"]);
    expect(analyseTitle("metaphor", ["writing"], ["OR"])).toContain("filters not carried over: OR");
    expect(analyseTitle("metaphor", ["writing"], [])).not.toContain("not carried");
  });

  test("special characters are escaped; no term, no link", () => {
    expect(link('"a&b=c #1"', { corpora: ["writing"] }).url).toBe("archivescout://explore?mode=kwic&q=a%26b%3Dc%20%231&in=writing");
    expect(link("au:scott", { corpora: ["writing"] }).url).toBeNull();
    expect(link("   ", { corpora: ["writing"] }).url).toBeNull();
  });

  test("unticked highlight sources go along, only with highlights", () => {
    expect(link("x", { corpora: ["highlights"], offSources: ["x"] }).url).toBe(
      "archivescout://explore?mode=kwic&q=x&in=highlights&source=readwise,zotero",
    );
    expect(link("x", { corpora: ["writing"], offSources: ["x"] }).url).not.toContain("source=");
  });

  test("the count line uses the real counts", () => {
    expect(resultCountLine({ total_documents: 30, total_passages: 64 })).toBe("30 pieces · 64 matches");
    expect(resultCountLine({ total_documents: 1, total_passages: 1 })).toBe("1 piece · 1 match");
    expect(resultCountLine(null)).toBe("");
  });

  test("the tooltip names the corpora", () => {
    expect(analyseTitle("scaffolding", ["writing"])).toContain("in Writing");
    expect(analyseTitle("x", ["writing", "tweets", "highlights"])).toContain("all corpora");
  });
});
