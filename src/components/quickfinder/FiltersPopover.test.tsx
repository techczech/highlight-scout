import { describe, expect, test, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { EMPTY_FILTERS } from "@scout/query";
import { FiltersPopover, type FiltersPopoverProps } from "./FiltersPopover";
import { DEFAULT_STATE, setMode, type SearchState } from "../../lib/searchModel";

const S = (o: Partial<SearchState> = {}): SearchState => ({ ...DEFAULT_STATE, ...o, filters: { ...EMPTY_FILTERS, ...(o.filters ?? {}) } });

function html(o: Partial<FiltersPopoverProps> = {}) {
  const p: FiltersPopoverProps = {
    state: S(),
    query: "paths metaphor",
    onState: vi.fn(),
    open: true,
    onOpenChange: vi.fn(),
    colors: ["red", "yellow", "#fff066"],
    tags: [{ tag: "epistemology", count: 4 }, { tag: "philosophy", count: 2 }],
    tagFocus: 0,
    onPickTag: vi.fn(),
    sourcesLabel: "X, Readwise, Zotero",
    ...o,
  };
  return renderToStaticMarkup(<FiltersPopover {...p} />);
}

const options = (h: string, select: string) => {
  const m = h.match(new RegExp(`aria-label="${select}"[^>]*>(.*?)</select>`));
  return [...(m?.[1] ?? "").matchAll(/<option value="([^"]+)"/g)].map((x) => x[1]);
};

describe("Filters · Group popover", () => {
  test("closed: just the button, naming the group; amber only when something is set", () => {
    const h = html({ open: false });
    expect(h).toContain("Filters · Group: Corpus");
    expect(h).not.toContain('data-testid="filters-popover"');
    expect(h).not.toContain("qf-fbtn on");
    const set = html({ open: false, state: S({ corpora: ["highlights"], color: "red", group: "work" }) });
    expect(set).toContain("Filters (1) · Group: Work");
    expect(set).toContain("qf-fbtn on");
  });

  test("all corpora ticked: View, Any corpus and Highlights (which says it narrows); Group offers Corpus", () => {
    const h = html();
    expect(h).toContain('data-section="view"');
    expect(h).toContain('data-section="any"');
    expect(h).toContain('data-section="highlights"');
    expect(h).toContain("These filters narrow the results to highlights.");
    expect(options(h, "Group")).toEqual(["corpus", "work", "author", "date", "none"]);
    expect(h).toMatch(/aria-label="then"[^>]*disabled/);
    // Every Classic popover option is there: quick toggles, types, time presets, colours, tags.
    for (const w of ["★ Favorites", "🔖 Zotero", "🖼 Has image", "Articles", "Books", "Tweets", "PDFs", "Podcasts", "30 days", "6 months", "Year", "Clear all", "Space toggles", "Esc closes"]) expect(h).toContain(w);
    expect((h.match(/aria-label="Colour /g) ?? []).length).toBe(3);
    expect(h).toContain(">epistemology</button>");
    expect(options(h, "Sort")).toEqual(["best", "newest", "oldest"]);
    expect(options(h, "Rows")).toEqual(["minimal", "compact", "comfortable", "full"]);
  });

  test("writing only: no Highlights section, and no Corpus in Group", () => {
    const h = html({ state: S({ corpora: ["writing"] }) });
    expect(h).not.toContain('data-section="highlights"');
    expect(options(h, "Group")).toEqual(["work", "author", "date", "none"]);
    expect(h).toContain('data-section="any"');
  });

  test("highlights only, semantic: Match greys out, the corpus engine's groups (no Tag, no then), no narrowing line", () => {
    const h = html({ state: { ...setMode(S({ corpora: ["highlights"] }), "semantic"), group: "work" } });
    expect(h).toMatch(/class="fp-row off" data-testid="match-row"/);
    expect((h.match(/<button[^>]*disabled=""[^>]*>(Whole word|Partial)</g) ?? []).length).toBe(2);
    expect(options(h, "Group")).toEqual(["work", "author", "date", "none"]);
    expect(h).toContain('data-section="highlights"');
    expect(h).not.toContain("These filters narrow");
  });

  test("the chosen values are shown checked", () => {
    const h = html({ state: S({ color: "red", filters: { ...EMPTY_FILTERS, favorite: true, types: ["books"], time: "t:6m" } }) });
    expect(h).toContain('aria-label="Colour red" aria-pressed="true"');
    expect(h).toMatch(/aria-checked="true"[^>]*><span class="box" aria-hidden="true">☑<\/span>★ Favorites/);
    expect(h).toMatch(/aria-checked="true"[^>]*><span class="box" aria-hidden="true">☑<\/span>Books/);
    expect(h).toMatch(/aria-checked="true"[^>]*><span class="box" aria-hidden="true">◉<\/span>6 months/);
  });
});
