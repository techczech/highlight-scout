import { describe, expect, test } from "vitest";
import { EMPTY_FILTERS } from "@scout/query";
import { DEFAULT_STATE, archiveRequests, highlightPayload, onlyCorpus, type SearchState } from "./searchModel";
import { createRailCounter, railCountJob, rowCount, tickedHighlightTotal, type HighlightCounts, type RailCounts } from "./railCounts";
import type { ArchiveSearchRequest } from "./archive";

const KNOWN = ["x", "readwise", "zotero"];
const LISTED = ["writing", "tweets", "highlights"];
const S = (o: Partial<SearchState> = {}): SearchState => ({ ...DEFAULT_STATE, ...o, filters: { ...EMPTY_FILTERS, ...(o.filters ?? {}) } });

/** A promise the test resolves by hand. */
function deferred<T>() {
  let resolve!: (v: T) => void;
  let reject!: (e: unknown) => void;
  const promise = new Promise<T>((res, rej) => { resolve = res; reject = rej; });
  return { promise, resolve, reject };
}
const tick = () => new Promise((r) => setTimeout(r, 0));

function harness() {
  const archive: Array<{ req: ArchiveSearchRequest; d: ReturnType<typeof deferred<number>> }> = [];
  const hl: Array<{ d: ReturnType<typeof deferred<HighlightCounts>> }> = [];
  const seen: Array<RailCounts | null> = [];
  const counter = createRailCounter({
    countArchive: (req) => { const d = deferred<number>(); archive.push({ req, d }); return d.promise; },
    countHighlights: () => { const d = deferred<HighlightCounts>(); hl.push({ d }); return d.promise; },
    onChange: (c) => seen.push(c),
  });
  return { counter, archive, hl, seen, last: () => seen[seen.length - 1] };
}

describe("what the rail counts (ticket 08)", () => {
  test("no query, or semantic: no counts, the sizes show", () => {
    expect(railCountJob(S(), "  ", LISTED, KNOWN)).toBeNull();
    expect(railCountJob(S({ mode: "semantic" }), "testing", LISTED, KNOWN)).toBeNull();
  });

  test("one engine count per corpus, the same request its 'show all' makes", () => {
    const s = S({ corpora: ["writing"], filters: { ...EMPTY_FILTERS, time: "t:30d" } });
    const job = railCountJob(s, "testing", LISTED, KNOWN)!;
    expect(job.archive.map((a) => a.corpus)).toEqual(["writing", "tweets"]);
    for (const { corpus, request } of job.archive) {
      const showAll = archiveRequests(onlyCorpus(s, corpus as "writing"), "testing", 50, KNOWN)[0];
      expect({ ...request, limit: 0 }).toEqual({ ...showAll, limit: 0 });
      expect(request.limit).toBe(1);
    }
  });

  test("Highlights is counted in the highlight index over every source, as 'show all' searches it", () => {
    const s = S({ offSources: ["x"], color: "yellow" });
    const job = railCountJob(s, "testing", LISTED, KNOWN)!;
    expect(job.highlights).toEqual(highlightPayload(onlyCorpus(s, "highlights"), "testing", 0, 1, KNOWN));
    expect(job.highlights!.sources).toEqual([]);
    expect(job.highlights!.color).toBe("yellow");
  });

  test("a highlight filter narrows the query to highlights: writing and tweets are not searched", () => {
    const job = railCountJob(S({ color: "yellow" }), "testing", LISTED, KNOWN)!;
    expect(job.archive).toEqual([]);
    expect(job.notSearched).toEqual(["writing", "tweets"]);
    expect(railCountJob(S(), "testing co:red", LISTED, KNOWN)!.notSearched).toEqual(["writing", "tweets"]);
  });

  test("the key changes with the query and the filters, not with the ticks", () => {
    const k = (s: SearchState, q: string) => railCountJob(s, q, LISTED, KNOWN)!.key;
    expect(k(S(), "testing")).toBe(k(S({ corpora: ["tweets"] }), "testing"));
    expect(k(S(), "testing")).not.toBe(k(S(), "testin"));
    expect(k(S(), "testing")).not.toBe(k(S({ filters: { ...EMPTY_FILTERS, time: "t:6m" } }), "testing"));
  });
});

describe("the per-corpus count state", () => {
  const job = (q: string) => railCountJob(S(), q, LISTED, KNOWN)!;

  test("counts arrive per corpus; highlight sources come with Highlights, 0 where nothing matched", async () => {
    const h = harness();
    h.counter.run(job("testing"));
    expect(h.last()!.pending).toEqual(["writing", "tweets", "highlights"]);
    expect(rowCount(h.last()!, "writing")).toEqual({ value: undefined, pending: true });
    h.archive[0].d.resolve(12);
    h.archive[1].d.resolve(27);
    h.hl[0].d.resolve({ total: 40, sources: { x: 30, readwise: 10 } });
    await tick();
    const c = h.last()!;
    expect(c.pending).toEqual([]);
    expect(c.corpora).toEqual({ writing: 12, tweets: 27, highlights: 40 });
    expect(c.sources).toEqual({ x: 30, readwise: 10, zotero: 0 });
    expect(rowCount(c, "highlights", "zotero")).toEqual({ value: 0, pending: false });
    expect(tickedHighlightTotal(c, [])).toBe(40);
    expect(tickedHighlightTotal(c, ["x"])).toBe(10);
  });

  test("a new query recounts, showing the old numbers as pending; the old query's late answers are dropped", async () => {
    const h = harness();
    h.counter.run(job("test"));
    h.archive[0].d.resolve(5);
    await tick();
    h.counter.run(job("testing"));
    expect(h.last()!.corpora.writing).toBe(5);
    expect(rowCount(h.last()!, "writing").pending).toBe(true);
    // The first query's slow answers arrive after the second started.
    h.archive[1].d.resolve(999);
    h.hl[0].d.resolve({ total: 999, sources: {} });
    await tick();
    expect(h.last()!.corpora.tweets).toBeUndefined();
    expect(h.last()!.corpora.highlights).toBeUndefined();
    h.archive[2].d.resolve(3);
    h.archive[3].d.resolve(4);
    h.hl[1].d.resolve({ total: 7, sources: { x: 7 } });
    await tick();
    expect(h.last()!.corpora).toEqual({ writing: 3, tweets: 4, highlights: 7 });
    expect(h.last()!.key).toBe(job("testing").key);
  });

  test("a counted query is cached: returning to it asks nothing; clear() forgets", async () => {
    const h = harness();
    h.counter.run(job("testing"));
    h.archive.forEach((a) => a.d.resolve(1));
    h.hl[0].d.resolve({ total: 2, sources: {} });
    await tick();
    h.counter.run(job("other"));
    h.counter.run(job("testing"));
    expect(h.archive).toHaveLength(4);
    expect(h.last()!.pending).toEqual([]);
    expect(h.last()!.corpora.highlights).toBe(2);
    h.counter.clear();
    h.counter.run(job("testing"));
    expect(h.archive).toHaveLength(6);
  });

  test("clearing the query drops counts in flight and goes back to sizes", async () => {
    const h = harness();
    h.counter.run(job("testing"));
    h.counter.run(null);
    expect(h.last()).toBeNull();
    h.archive[0].d.resolve(9);
    await tick();
    expect(h.last()).toBeNull();
  });

  test("a failed count shows a dash and is not cached", async () => {
    const h = harness();
    h.counter.run(job("testing"));
    h.archive[0].d.reject(new Error("index missing"));
    h.archive[1].d.resolve(2);
    h.hl[0].d.reject(new Error("db"));
    await tick();
    expect(h.last()!.corpora).toEqual({ writing: null, tweets: 2, highlights: null });
    expect(h.last()!.sources).toEqual({ x: null, readwise: null, zotero: null });
    expect(tickedHighlightTotal(h.last(), [])).toBeNull();
    h.counter.run(job("testing"));
    expect(h.archive).toHaveLength(4);
  });

  test("a malformed answer is a failed count, not a crash", async () => {
    const h = harness();
    h.counter.run(job("testing"));
    h.archive[0].d.resolve(undefined as unknown as number);
    h.archive[1].d.resolve(1);
    h.hl[0].d.resolve(null as unknown as HighlightCounts);
    await tick();
    expect(h.last()!.corpora).toEqual({ writing: null, tweets: 1, highlights: null });
  });

  test("not-searched corpora are null at once", () => {
    const h = harness();
    h.counter.run(railCountJob(S({ color: "red" }), "testing", LISTED, KNOWN));
    expect(h.last()!.corpora).toMatchObject({ writing: null, tweets: null });
    expect(h.last()!.pending).toEqual(["highlights"]);
  });
});
