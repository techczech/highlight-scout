import { describe, expect, test } from "vitest";
import { analyseTerm, analyseTitle, analyseUrl, resultCountLine } from "./archivescout";

describe("Analyse in ArchiveScout", () => {
  test("the link carries the term and the ticked corpora", () => {
    expect(analyseUrl({ query: "scaffolding", corpora: ["writing"] })).toBe(
      "archivescout://explore?mode=kwic&q=scaffolding&in=writing",
    );
    expect(analyseUrl({ query: "generative metaphor", corpora: ["writing", "tweets", "highlights"] })).toBe(
      "archivescout://explore?mode=kwic&q=generative%20metaphor&in=writing,tweets,highlights",
    );
  });

  test("operators and filters are not the term", () => {
    expect(analyseTerm('cat OR dog -bird au:scott y:2023 "exact phrase" prefix*')).toBe("cat dog exact phrase prefix");
    expect(analyseTerm('-"not this" kept')).toBe("kept");
    expect(analyseTerm("/\\bAI\\b/")).toBe("");
  });

  test("special characters are escaped; no term, no link", () => {
    expect(analyseUrl({ query: "a&b=c #1", corpora: [] })).toBe("archivescout://explore?mode=kwic&q=a%26b%3Dc%20%231");
    expect(analyseUrl({ query: "au:scott", corpora: ["writing"] })).toBeNull();
    expect(analyseUrl({ query: "   ", corpora: ["writing"] })).toBeNull();
  });

  test("unticked highlight sources go along, only with highlights", () => {
    expect(analyseUrl({ query: "x", corpora: ["highlights"], sources: ["readwise", "zotero"] })).toBe(
      "archivescout://explore?mode=kwic&q=x&in=highlights&source=readwise,zotero",
    );
    expect(analyseUrl({ query: "x", corpora: ["writing"], sources: ["zotero"] })).not.toContain("source=");
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
