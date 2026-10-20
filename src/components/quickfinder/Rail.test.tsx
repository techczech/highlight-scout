import { describe, expect, test, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { Rail, type RailProps } from "./Rail";
import type { RailCounts } from "../../lib/railCounts";
import type { CorpusCount } from "../../lib/archive";

const counts: CorpusCount[] = [
  { corpus: "writing", docs: 1742, indexed: true, sources: {} },
  { corpus: "tweets", docs: 14892, indexed: true, sources: {} },
  { corpus: "highlights", docs: 15010, indexed: true, sources: { x: 9864, readwise: 4794, zotero: 352 } },
];

function html(results: RailCounts | null) {
  const p: RailProps = {
    counts, results, corpora: ["writing", "tweets", "highlights"], sourceOn: () => true,
    onCorpus: vi.fn(), onSource: vi.fn(), recent: [], onRecent: vi.fn(),
  };
  return renderToStaticMarkup(<Rail {...p} />);
}

function setsHtml(extra: Partial<RailProps>) {
  const p: RailProps = {
    counts, results: null, corpora: ["writing"], sourceOn: () => true,
    onCorpus: vi.fn(), onSource: vi.fn(), recent: [], onRecent: vi.fn(), ...extra,
  };
  return renderToStaticMarkup(<Rail {...p} />);
}

describe("the rail's sets", () => {
  test("no sets yet says so, with a way to make one", () => {
    const h = setsHtml({ onNewSet: vi.fn() });
    expect(h).toContain("No sets yet");
    expect(h).toContain('data-action="new-set"');
    expect(h).not.toContain("save-search-set");
  });

  test("each set is a row with its size; the current one is marked; a query can be saved as a set", () => {
    const h = setsHtml({
      sets: [
        { id: "metaphor-talk", title: "Metaphor talk", items: 7, current: true, kind: "hand" },
        { id: "scaffolding", title: "Scaffolding", items: 3, current: false, kind: "saved-search" },
      ],
      onOpenSet: vi.fn(), onNewSet: vi.fn(), onSaveSearch: vi.fn(),
    });
    expect(h).toContain('data-set="metaphor-talk"');
    expect(h).toMatch(/setrow cur"[^>]*data-set="metaphor-talk"/);
    expect(h).toContain(">7<");
    expect(h).not.toContain("No sets yet");
    expect(h).toContain('data-action="save-search-set"');
  });
});

describe("the rail's numbers (ticket 08)", () => {
  test("no query: the sizes, in the quiet style", () => {
    const h = html(null);
    expect(h).toContain('class="n size" title="In the archive">1,742<');
    expect(h).toContain('class="n size" title="In the archive">9,864<');
    expect(h).not.toContain("n hits");
    expect(h).toContain("Counts are pieces, tweets and works.");
  });

  test("a query: each row's result count, faded while recounting, a dash where not searched", () => {
    const h = html({
      key: "k",
      corpora: { writing: 2, tweets: null, highlights: 31 },
      sources: { x: 20, readwise: 11, zotero: 0 },
      pending: ["highlights"],
    });
    expect(h).toContain('class="n hits" title="Results for this query">2<');
    expect(h).toContain('class="n hits none" title="Not searched by this query">–<');
    expect(h).toContain('class="n hits stale" title="Results for this query">31<');
    expect(h).toContain('class="n hits stale" title="Results for this query">0<');
    expect(h).not.toContain("1,742");
    expect(h).toContain("Counts are results");
  });
});
