import { describe, expect, test } from "vitest";
import {
  corpusBadge,
  errorLine,
  jobBuilt,
  jobVisible,
  noteLine,
  resultSummary,
  sourceLine,
  toCorpusError,
  wantsIndex,
  type ArchiveSearchResults,
  type IndexJob,
} from "./archive";

const results = (o: Partial<ArchiveSearchResults>): ArchiveSearchResults => ({
  schema_version: 1,
  query: "generative metaphor",
  corpora: ["writing", "tweets", "highlights"],
  total_documents: 3,
  total_passages: 5,
  results: new Array(3).fill(null),
  ...o,
});

const job = (o: Partial<IndexJob>): IndexJob => ({ phase: "idle", corpora: [], builds: [], message: "", ...o });

describe("archive search helpers", () => {
  test("every result's badge names its corpus", () => {
    expect(["writing", "tweets", "highlights"].map(corpusBadge)).toEqual(["Writing", "Tweet", "Highlight"]);
    expect(corpusBadge("notes")).toBe("Notes");
  });

  test("the summary counts documents and passages over the corpora searched", () => {
    expect(resultSummary(results({}))).toBe("3 documents · 5 passages · Writing, Tweet, Highlight");
    const top = results({ total_documents: 120, total_passages: 1, corpora: ["writing"] });
    top.results = new Array(50).fill(null);
    expect(resultSummary(top)).toBe("120 documents (top 50) · 1 passage · Writing");
  });

  test("backend errors are typed; index errors trigger a build, a missing registry says what to run", () => {
    const e = toCorpusError({ kind: "no_indexed_corpus", message: "no corpus has an index" });
    expect(wantsIndex(e)).toBe(true);
    expect(errorLine(e)).toMatch(/building it in the background/);
    const r = toCorpusError({ kind: "registry_missing", message: "no corpus registry at ~/.config/scout/corpora.toml" });
    expect(wantsIndex(r)).toBe(false);
    expect(errorLine(r)).toMatch(/scout corpora init-defaults/);
    expect(toCorpusError(new Error("boom"))).toEqual({ kind: "other", message: "boom" });
    expect(toCorpusError("plain")).toEqual({ kind: "other", message: "plain" });
  });

  test("notes and source lines read plainly", () => {
    expect(noteLine("scout: note: using writing; not indexed: tweets")).toBe("using writing; not indexed: tweets");
    expect(sourceLine({ path: "/w/essay.md", line_start: 8, line_end: 8 })).toBe("/w/essay.md:8");
    expect(sourceLine({ path: "/w/essay.md", line_start: 8, line_end: 10 })).toBe("/w/essay.md:8–10");
  });

  test("the status bar shows building, built, failed and unavailable, not a quiet check", () => {
    expect(["checking", "current", "idle"].map((p) => jobVisible(job({ phase: p as IndexJob["phase"] })))).toEqual([false, false, false]);
    expect(["building", "built", "failed", "unavailable"].map((p) => jobVisible(job({ phase: p as IndexJob["phase"] })))).toEqual([true, true, true, true]);
    expect(jobBuilt(job({ phase: "built" }))).toBe(true);
    expect(jobBuilt(job({ phase: "failed", builds: [{ corpus: "tweets", reason: "missing", error: "no root" }] }))).toBe(false);
    expect(
      jobBuilt(job({
        phase: "failed",
        builds: [
          { corpus: "writing", reason: "stale", error: null },
          { corpus: "tweets", reason: "missing", error: "no root" },
        ],
      })),
    ).toBe(true);
  });
});
