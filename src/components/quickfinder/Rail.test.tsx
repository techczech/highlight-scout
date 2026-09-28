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
