import { describe, expect, test, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { createElement } from "react";
import { estimateLine, meaningNotice, type CorpusVectors, type MeaningJob, type MeaningState } from "./meaning";
import { MeaningBanner } from "../components/quickfinder/MeaningBanner";

const ALL = ["writing", "tweets", "highlights"];
const idle: MeaningJob = { phase: "idle", corpora: [], corpus: null, done: 0, total: 0, message: "" };

function state(vectors: Record<string, CorpusVectors["state"]>, o: Partial<MeaningState> = {}): MeaningState {
  const corpora = Object.entries(vectors).map(([corpus, s]) => ({ corpus, state: s, why: null, passages: 1000 }));
  const todo = corpora.filter((c) => c.state === "missing" || c.state === "stale").map((c) => c.corpus);
  return {
    available: true,
    model: "minilm-l12@256",
    model_ready: true,
    corpora,
    estimate: todo.length ? { corpora: todo, minutes: 9, megabytes: 265, download: null } : null,
    job: idle,
    running: false,
    ...o,
  };
}

const banner = (n: ReturnType<typeof meaningNotice>) => renderToStaticMarkup(createElement(MeaningBanner, { notice: n, onBuild: vi.fn() }));

describe("the vector-state UI (ticket 09)", () => {
  test("missing: offers Build meaning index with its time and size; Semantic says it is unavailable", () => {
    const n = meaningNotice(state({ writing: "missing", tweets: "missing", highlights: "missing" }), null, ALL)!;
    expect(n.kind).toBe("missing");
    expect(n.action).toBe("Build meaning index");
    expect(n.text).toContain("Writing, Tweets and Highlights (not built)");
    expect(n.text).toContain("about 9 min and 265 MB");
    expect(n.text).toContain("Until then Semantic is unavailable");
    const h = banner(n);
    expect(h).toContain('data-state="missing"');
    expect(h).toContain('data-testid="meaning-build"');
  });

  test("stale for one ticked corpus: the rest are searched by meaning meanwhile", () => {
    const n = meaningNotice(state({ writing: "stale", tweets: "current", highlights: "current" }), null, ["writing", "highlights"])!;
    expect(n.kind).toBe("missing");
    expect(n.text).toContain("Writing (out of date)");
    expect(n.text).toContain("Until then Semantic searches Highlights by meaning and the rest by their words.");
  });

  test("only the ticked corpora count: a missing corpus that is not ticked says nothing", () => {
    expect(meaningNotice(state({ writing: "current", tweets: "missing", highlights: "current" }), null, ["writing", "highlights"])).toBeNull();
  });

  test("current: no notice at all", () => {
    const st = state({ writing: "current", tweets: "current", highlights: "current" });
    expect(meaningNotice(st, null, ALL)).toBeNull();
    expect(banner(null)).toBe("");
  });

  test("building: progress, no button", () => {
    const job: MeaningJob = { phase: "building", corpora: ["writing"], corpus: "writing", done: 12000, total: 48000, message: "" };
    const n = meaningNotice(state({ writing: "missing" }), job, ALL)!;
    expect(n.kind).toBe("building");
    expect(n.text).toContain("Writing: 12,000 of 48,000 passages (25%)");
    expect(n.action).toBeUndefined();
    const h = banner(n);
    expect(h).toContain('data-state="building"');
    expect(h).not.toContain("meaning-build");
    // The backend's own word that a build is running counts too.
    expect(meaningNotice(state({ writing: "missing" }, { running: true }), null, ALL)!.kind).toBe("building");
  });

  test("failed: the message and a way to try again", () => {
    const job: MeaningJob = { ...idle, phase: "failed", message: "Meaning index build failed: disk full" };
    const n = meaningNotice(state({ writing: "missing" }), job, ALL)!;
    expect(n).toEqual({ kind: "failed", text: "Meaning index build failed: disk full", action: "Try again" });
  });

  test("the model not on disk: the build names the one-off download, and nothing is searched by meaning", () => {
    const st = state({ writing: "current", tweets: "current", highlights: "current" }, {
      model_ready: false,
      estimate: { corpora: [], minutes: 0, megabytes: 0, download: "downloading embedding model minilm-l12 (Xenova/paraphrase-multilingual-MiniLM-L12-v2, about 490 MB) once to /x" },
    });
    const n = meaningNotice(st, null, ALL)!;
    expect(n.kind).toBe("missing");
    expect(n.text).toContain("a one-off model download of about 490 MB");
    expect(n.text).toContain("Until then Semantic is unavailable");
  });

  test("no model in this build: unavailable, no button", () => {
    const n = meaningNotice(state({ writing: "missing" }, { available: false }), null, ALL)!;
    expect(n.kind).toBe("unavailable");
    expect(n.action).toBeUndefined();
  });

  test("the estimate line", () => {
    expect(estimateLine({ corpora: ["w"], minutes: 9, megabytes: 1265, download: null })).toBe("about 9 min and 1,265 MB");
    expect(estimateLine({ corpora: ["w"], minutes: 2, megabytes: 40, download: "downloading … about 490 MB …" })).toBe("about 2 min and 40 MB, plus a one-off model download of about 490 MB");
  });
});
