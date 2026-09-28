import { describe, expect, test, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { HighlightResults, highlightMeta, type HighlightResultsProps } from "./HighlightResults";
import { HighlightPane, positionLine } from "./HighlightPane";
import { groupRows } from "../../lib/grouping";
import type { SearchResult } from "../../types";

function hl(id: string, o: Partial<SearchResult> = {}): SearchResult {
  return {
    highlight_id: id, work_id: "w1", slug: "newell", text: `The digital computer as a tool ${id}`, note: null,
    title: "Computer Simulation of Human Thinking", author: "Newell", authors: [], work_type: "article",
    source_system: "zotero", source_id: null, url: "https://www.jstor.org/stable/1708447", highlighted_at: "2021-05-01T00:00:00Z",
    tags: ["ai"], location: "2011", annotation_color: "red", annotation_type: null, format: "text", asset_path: null,
    citation: "Newell, A. (1961).", collections: ["Minds"], zotero_link: "zotero://open-pdf/1", relevance: null, snippet: "", ocr_text: null,
    ...o,
  };
}

const rows = [hl("a"), hl("b", { location: "2012" }), hl("c", { work_id: "w2", title: "Minds, brains, and programs", author: "Searle" }), hl("d", { work_id: "w2", title: "Minds, brains, and programs", author: "Searle" })];

function props(o: Partial<HighlightResultsProps> = {}): HighlightResultsProps {
  return {
    query: "computer", terms: ["computer"], rows, sections: groupRows(rows, "work", "none", "matches"), density: "compact",
    semantic: false, showPane: true, groupLabel: "Group: Work", sort: "best", onSort: vi.fn(), activeId: "a", onActivate: vi.fn(),
    onOpenDetail: vi.fn(), onScrollEnd: vi.fn(), hasMore: false,
    actions: { onQuote: vi.fn(), onQuoteCitation: vi.fn(), onLink: vi.fn(), copied: null, backTo: null, ready: true },
    empty: <p>none</p>,
    ...o,
  };
}

describe("highlight index results in the one list", () => {
  test("work groups head the rows; the selected row carries the copy actions", () => {
    const h = renderToStaticMarkup(<HighlightResults {...props()} />);
    expect(h).toContain('<span class="lbl">Computer Simulation of Human Thinking</span>');
    expect(h).toContain('<span class="lbl">Minds, brains, and programs</span>');
    expect(h).toContain("Group: Work");
    expect(h).toContain("Zotero · location 2011 · 2021");
    expect((h.match(/data-testid="row-actions"/g) ?? []).length).toBe(1);
    expect(h).toContain('class="qf-list d-compact"');
  });

  test("ungrouped rows name author and work; Rows density is the list's class; semantic shows its badges", () => {
    expect(highlightMeta(rows[2], false)).toEqual(["Zotero", "location 2011", "Searle", "Minds, brains, and programs", "2021"]);
    const h = renderToStaticMarkup(<HighlightResults {...props({ sections: null, density: "full", semantic: true, rows: [hl("a", { relevance: 0.82 })] })} />);
    expect(h).toContain("d-full");
    expect(h).toContain("82%");
    expect(h).toContain("keyword + semantic");
    const min = renderToStaticMarkup(<HighlightResults {...props({ density: "minimal", showPane: false })} />);
    expect(min).toContain('class="body min"');
    expect(min).toContain('<span class="au">Newell</span>');
  });

  test("no rows: the given empty state", () => {
    expect(renderToStaticMarkup(<HighlightResults {...props({ rows: [], sections: null, empty: <p>Press ↵</p> })} />)).toContain("Press ↵");
  });
});

describe("the highlight reading pane keeps Classic's pane", () => {
  test("position in the work, as Classic shows it", () => {
    expect(positionLine(rows[0], null)).toBe("location 2011");
    expect(positionLine(rows[0], { pos: 3, total: 12, max_loc: 4400 })).toBe("3 of 12 · location 2011 of 4400");
    expect(positionLine(hl("t", { work_type: "tweet" }), null)).toBeNull();
  });

  test("text in Classic's reading size, note, citation, links, Copy ▾, metadata and every action", () => {
    const h = renderToStaticMarkup(
      <HighlightPane row={hl("a", { note: "my note" })} terms={["computer"]} position={{ pos: 3, total: 12, max_loc: 4400 }} format="auto" onFormat={vi.fn()} copyPreview onCopyPreview={vi.fn()}
        onOpenUrl={vi.fn()} onFindRelated={vi.fn()} onShowWork={vi.fn()} onNewWindow={vi.fn()} onToast={vi.fn()} />,
    );
    expect(h).toContain('class="p-quote classic"'); // 15px / 1.625 in quickfinder.css, as Classic's text-[15px] leading-relaxed
    expect(h).toContain("border-left-color:#ef4444");
    for (const w of ["Open PDF in Zotero", "jstor.org", "Copy ▾", "3 of 12 · location 2011 of 4400", "my note", "Newell, A. (1961).", "Minds", "Find related", "Show work highlights →", "⧉ New window", "⌘⇧C copies"]) expect(h).toContain(w);
    expect(h).toContain("&gt; The digital computer as a tool a"); // the ⌘⇧C preview: Classic's Markdown quote
  });
});
