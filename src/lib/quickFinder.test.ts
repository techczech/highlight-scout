import { describe, expect, test } from "vitest";
import { doc, results } from "./quickFinder.fixtures";
import {
  appName,
  citationFlavours,
  copyLinkFor,
  countsLine,
  groupKey,
  groupResults,
  moveKey,
  pushRecent,
  requestFor,
  resolveCopyFormat,
  rowMeta,
  splitAroundSentence,
  visibleKeys,
  WRITEFLEX_BUNDLE,
} from "./quickFinder";

// Engine order interleaves corpora by rank; the finder groups them.
const mixed = [
  doc("highlights", "h1"), doc("writing", "w1"), doc("tweets", "t1"), doc("writing", "w2"),
  doc("highlights", "h2"), doc("writing", "w3", { date: "2020-06-28" }), doc("writing", "w4", { date: "2010-07-18" }),
];

describe("quick finder grouping", () => {
  test("groups Writing, Tweets, Highlights in that order, engine rank kept inside a group", () => {
    const g = groupResults(results(mixed), { corpus: "all" });
    expect(g.map((x) => x.corpus)).toEqual(["writing", "tweets", "highlights"]);
    expect(g[0].docs.map((d) => d.rel_path)).toEqual(["w1", "w2", "w3", "w4"]);
    expect(g.map((x) => x.more)).toEqual([false, false, false]);
  });

  test("under All each group previews its first rows; show all appears when there are more", () => {
    const g = groupResults(results(mixed), { corpus: "all" }, "best", 2);
    expect(g[0].docs.map((d) => d.rel_path)).toEqual(["w1", "w2"]);
    expect(g[0].fetched).toBe(4);
    expect(g[0].more).toBe(true);
    expect(g[1].more).toBe(false);
    // Truncated results: every group may have more.
    expect(groupResults(results(mixed, 90), { corpus: "all" }).every((x) => x.more)).toBe(true);
    // One corpus selected: every fetched row, no "show all".
    const one = groupResults(results(mixed.filter((d) => d.corpus === "writing"), 90), { corpus: "writing" }, "best", 2);
    expect(one).toHaveLength(1);
    expect(one[0].docs).toHaveLength(4);
    expect(one[0].more).toBe(false);
  });

  test("newest and oldest sort inside each group", () => {
    const g = groupResults(results(mixed), { corpus: "all" }, "newest");
    expect(g[0].docs.map((d) => d.rel_path)).toEqual(["w3", "w1", "w2", "w4"]);
    expect(groupResults(results(mixed), { corpus: "all" }, "oldest")[0].docs[0].rel_path).toBe("w4");
  });

  test("↑↓ walks the visible rows top to bottom; ⌥↓ jumps to the next group and wraps", () => {
    const g = groupResults(results(mixed), { corpus: "all" });
    expect(visibleKeys(g)).toEqual(["writing:w1", "writing:w2", "writing:w3", "writing:w4", "tweets:t1", "highlights:h1", "highlights:h2"]);
    expect(moveKey(g, "writing:w4", 1)).toBe("tweets:t1");
    expect(moveKey(g, "writing:w1", -1)).toBe("writing:w1");
    expect(moveKey(g, null, 1)).toBe("writing:w2");
    expect(groupKey(g, "writing:w2", 1)).toBe("tweets:t1");
    expect(groupKey(g, "tweets:t1", 1)).toBe("highlights:h1");
    expect(groupKey(g, "highlights:h2", 1)).toBe("writing:w1");
    expect(groupKey(g, "writing:w3", -1)).toBe("highlights:h1");
    expect(groupKey([], null, 1)).toBeNull();
  });
});

describe("quick finder requests", () => {
  test("the rail narrows with --in; a highlight source uses the engine's source: field", () => {
    expect(requestFor("paths metaphor", { corpus: "all" }, 50)).toEqual({ query: "paths metaphor", in: [], limit: 50 });
    expect(requestFor("paths", { corpus: "tweets" }, 50)).toEqual({ query: "paths", in: ["tweets"], limit: 50 });
    expect(requestFor("paths", { corpus: "highlights", source: "zotero" }, 50)).toEqual({ query: "paths source:zotero", in: ["highlights"], limit: 50 });
  });
});

describe("quick finder copy", () => {
  test("Auto copies Markdown into WriteFlex and rich text anywhere else", () => {
    expect(resolveCopyFormat("auto", WRITEFLEX_BUNDLE)).toBe("markdown");
    expect(resolveCopyFormat("auto", "com.microsoft.Word")).toBe("rich");
    expect(resolveCopyFormat("auto", null)).toBe("rich");
    expect(resolveCopyFormat("markdown", "com.microsoft.Word")).toBe("markdown");
    expect(resolveCopyFormat("rich", WRITEFLEX_BUNDLE)).toBe("rich");
    expect(appName(WRITEFLEX_BUNDLE)).toBe("WriteFlex");
    expect(appName("com.tinyspeck.slackmacgap")).toBe("Slackmacgap");
    expect(appName(null)).toBeNull();
  });

  // The formats as the backend hands them over (corpus_copy golden forms).
  const passage = {
    cited: { citation: { markdown: "> Q.\n\n— Dominik Lukeš, *Full Title*, 23 June 2016 · [archive](writeflex://open?path=%2Fa.md&line=3) · [public](https://medium.com/x)\n" } },
    html: '<blockquote><p>Q.</p></blockquote><p>— Dominik Lukeš, <em>Full Title</em>, 23 June 2016 · <a href="writeflex://open?path=%2Fa.md&amp;line=3">archive</a> · <a href="https://medium.com/x">public</a></p>',
    plain: "“Q.”\n— Dominik Lukeš, Full Title, 23 June 2016 · <https://medium.com/x>\n",
  };
  const visible = (f: { html: string; text: string }, how: string) => [
    f.html.replace(/<[^>]*>/g, ""),
    how === "markdown" ? f.text.replace(/\]\([^)\s]+\)/g, "]") : f.text,
  ];

  test("⌘⇧C always writes both flavours; plain is Markdown only for WriteFlex", () => {
    const md = citationFlavours(passage, "markdown");
    expect(md.html).toBe(passage.html);
    expect(md.text).toBe("> Q.\n\n— Dominik Lukeš, *Full Title*, 23 June 2016 · [archive](writeflex://open?path=%2Fa.md&line=3) · [public](https://medium.com/x)");
    const rich = citationFlavours(passage, "rich");
    expect(rich.html).toBe(passage.html);
    expect(rich.text).toBe("“Q.”\n— Dominik Lukeš, Full Title, 23 June 2016 · <https://medium.com/x>");
  });

  test("no flavour shows a raw writeflex:// URL as text", () => {
    for (const how of ["markdown", "rich"] as const) {
      for (const text of visible(citationFlavours(passage, how), how)) expect(text).not.toContain("writeflex://");
    }
    // The check bites: the engine's old plain form printed the path.
    const old = { ...passage, plain: "“Q.”\n— A · archive: writeflex://open?path=%2Fa.md · public: https://medium.com/x" };
    expect(visible(citationFlavours(old, "rich"), "rich")[1]).toContain("writeflex://");
  });

  test("Link copies the public URL, else the archive link", () => {
    expect(copyLinkFor({ public_url: "https://medium.com/x", link: "writeflex://open?path=a" })).toBe("https://medium.com/x");
    expect(copyLinkFor({ public_url: null, link: "writeflex://open?path=a" })).toBe("writeflex://open?path=a");
    expect(copyLinkFor({ public_url: null, link: null })).toBeNull();
  });
});

describe("quick finder rows, pane and rail", () => {
  test("recent searches: newest first, no duplicates, capped", () => {
    expect(pushRecent(["scaffolding"], "paths metaphor")).toEqual(["paths metaphor", "scaffolding"]);
    expect(pushRecent(["paths metaphor", "scaffolding"], " Scaffolding ")).toEqual(["Scaffolding", "paths metaphor"]);
    expect(pushRecent(["a1", "b1"], "x")).toEqual(["a1", "b1"]);
    expect(pushRecent(["a1", "b1", "c1"], "d1", 3)).toEqual(["d1", "a1", "b1"]);
  });

  test("meta lines: writing date · genre, tweets date · @handle, highlights source · author", () => {
    expect(rowMeta(doc("writing", "w", { genre: "essay" }))).toEqual({ parts: ["2016-06-23", "essay"] });
    expect(rowMeta(doc("tweets", "t", { date: "2025-07-26T10:12:00Z", public_url: "https://x.com/techczech/status/1" }))).toEqual({ parts: ["2025-07-26", "@techczech"] });
    expect(rowMeta(doc("highlights", "h", { source: "readwise", author: "Helen Sword", title: "Air & Light & Time & Space" }))).toEqual({
      by: "Helen Sword",
      parts: ["Readwise", "Air & Light & Time & Space"],
    });
  });

  test("the pane marks the matched sentence inside the passage", () => {
    expect(splitAroundSentence("First. But being open matters. Last.", "But being open matters.")).toEqual({
      before: "First. ",
      sentence: "But being open matters.",
      after: " Last.",
    });
    expect(splitAroundSentence("Whole passage.", "Whole passage.")).toEqual({ before: "", sentence: "Whole passage.", after: "" });
    expect(splitAroundSentence("Whole passage.", "not in it")).toEqual({ before: "", sentence: "Whole passage.", after: "" });
  });

  test("the footer names each corpus size in its unit", () => {
    expect(
      countsLine([
        { corpus: "writing", docs: 1506, indexed: true },
        { corpus: "tweets", docs: 14892, indexed: true },
        { corpus: "highlights", docs: 14724, indexed: true },
      ]),
    ).toBe("Writing 1,506 pieces · Tweets 14,892 · Highlights 14,724 works");
    expect(countsLine([{ corpus: "writing", docs: 3, indexed: false }])).toBe("");
  });
});
