import { describe, expect, test, vi } from "vitest";
import { isValidElement, type ReactElement, type ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { GroupedResults, type GroupedResultsProps, type RowActionHandlers } from "./GroupedResults";
import { groupResults } from "../../lib/quickFinder";
import { doc, results } from "../../lib/quickFinder.fixtures";

// Shallow render without a DOM: expand the hook-free components into host
// elements, so a test can find a button and call its onClick.
type Host = { type: string; props: Record<string, unknown>; children: Host[] };
function expand(node: ReactNode): Host[] {
  if (node === null || node === undefined || typeof node === "boolean") return [];
  if (Array.isArray(node)) return node.flatMap(expand);
  if (!isValidElement(node)) return [];
  const el = node as ReactElement<Record<string, unknown>>;
  if (typeof el.type === "function") return expand((el.type as (p: unknown) => ReactNode)(el.props));
  if (typeof el.type !== "string") return expand(el.props.children as ReactNode); // fragments
  return [{ type: el.type, props: el.props, children: expand(el.props.children as ReactNode) }];
}
function findAll(nodes: Host[], pred: (h: Host) => boolean): Host[] {
  return nodes.flatMap((n) => [...(pred(n) ? [n] : []), ...findAll(n.children, pred)]);
}
const click = (h: Host) => (h.props.onClick as (e: { stopPropagation: () => void }) => void)({ stopPropagation() {} });

const docs = [
  doc("highlights", "h1", { source: "readwise", author: "Helen Sword", title: "Air & Light" }),
  doc("writing", "repaved.md", { title: "Repaved paths and generative metaphors: Expressing human purposes with technology", genre: "essay" }),
  doc("tweets", "stream/2025-07.md#1", { public_url: "https://x.com/techczech/status/1" }),
  doc("writing", "hack.md", { title: "Hacking a metaphor in five steps", genre: "guide" }),
];

function props(o: Partial<GroupedResultsProps> = {}, a: Partial<RowActionHandlers> = {}): GroupedResultsProps {
  const r = results(docs, 40);
  return {
    query: "paths metaphor",
    terms: ["paths", "metaphor"],
    results: r,
    groups: groupResults(r, { corpus: "all" }),
    filter: { corpus: "all" },
    sort: "best",
    onSort: vi.fn(),
    activeKey: "writing:repaved.md",
    onSelect: vi.fn(),
    onShowAll: vi.fn(),
    actions: { onQuote: vi.fn(), onQuoteCitation: vi.fn(), onLink: vi.fn(), copied: null, backTo: "WriteFlex", ready: true, ...a },
    loading: false,
    error: "",
    summary: "40 documents (top 4) · 4 passages",
    ...o,
  };
}

describe("GroupedResults", () => {
  test("renders Writing, Tweets, Highlights groups with badges, titles for writing and show all", () => {
    const html = renderToStaticMarkup(<GroupedResults {...props()} />);
    const order = ["data-corpus=\"writing\"", "data-corpus=\"tweets\"", "data-corpus=\"highlights\""].map((s) => html.indexOf(s));
    expect(order.every((i, k) => i > 0 && (k === 0 || i > order[k - 1]))).toBe(true);
    expect(html).toContain("grouped by corpus");
    expect(html).toContain(">Writing</span>");
    expect(html).toContain("Repaved paths and generative metaphors: Expressing human purposes with technology");
    expect(html).toContain("@techczech");
    expect(html).toContain("Helen Sword");
    expect((html.match(/data-action="show-all"/g) ?? []).length).toBe(3); // results were truncated (40 found, 4 fetched)
    // Only the selected row carries the actions.
    expect((html.match(/data-testid="row-actions"/g) ?? []).length).toBe(1);
    expect(html).toContain("Quote + citation <kbd>⌘⇧C</kbd>");
    expect(html).toMatch(/data-action="set" disabled=""/);
    // Query terms are marked in the snippet, as text (never HTML from the source).
    expect(renderToStaticMarkup(<GroupedResults {...props({ groups: groupResults(results([doc("writing", "a", { hits: [{ passage_id: "writing:a:1", line_start: 1, line_end: 1, line: 1, quote: "new <b>paths</b> trodden", score: 1, link: null }] })]), { corpus: "all" }), activeKey: null })} />))
      .toContain("<span>new &lt;b&gt;</span><mark>paths</mark><span>&lt;/b&gt; trodden</span>");
  });

  test("row actions call their handlers; show all opens that corpus; a row click selects it", () => {
    const p = props();
    const tree = expand(<GroupedResults {...p} />);
    const byAction = (a: string) => findAll(tree, (h) => h.props["data-action"] === a);
    click(byAction("quote")[0]);
    click(byAction("quote-citation")[0]);
    click(byAction("link")[0]);
    expect(p.actions.onQuote).toHaveBeenCalledOnce();
    expect(p.actions.onQuoteCitation).toHaveBeenCalledOnce();
    expect(p.actions.onLink).toHaveBeenCalledOnce();
    expect(byAction("set")[0].props.disabled).toBe(true);

    click(byAction("show-all")[1]);
    expect(p.onShowAll).toHaveBeenCalledWith("tweets");

    const rows = findAll(tree, (h) => h.props["data-testid"] === "archive-row");
    expect(rows.map((r) => r.props["data-key"])).toEqual(["writing:repaved.md", "writing:hack.md", "tweets:stream/2025-07.md#1", "highlights:h1"]);
    (rows[1].props.onClick as () => void)();
    expect(p.onSelect).toHaveBeenCalledWith("writing:hack.md");
  });

  test("after ⌘⇧C the row confirms in place and says where Esc returns", () => {
    const html = renderToStaticMarkup(<GroupedResults {...props({}, { copied: "citation" })} />);
    expect(html).toContain("Copied quote + citation");
    expect(html).toContain("esc back to WriteFlex");
    expect(html).not.toContain('data-action="quote-citation"');
  });

  test("no passage loaded yet: the copy actions are disabled", () => {
    const tree = expand(<GroupedResults {...props({}, { ready: false })} />);
    const q = findAll(tree, (h) => h.props["data-action"] === "quote-citation")[0];
    expect(q.props.disabled).toBe(true);
  });

  test("empty, loading and error states", () => {
    expect(renderToStaticMarkup(<GroupedResults {...props({ groups: [], results: null })} />)).toContain("Search your writing, tweets and highlights.");
    expect(renderToStaticMarkup(<GroupedResults {...props({ groups: [], results: null, loading: true })} />)).toContain("Searching writing, tweets and highlights…");
    expect(renderToStaticMarkup(<GroupedResults {...props({ groups: [], error: "Archive search failed: x" })} />)).toContain("Archive search failed: x");
    expect(renderToStaticMarkup(<GroupedResults {...props({ groups: [], results: results([]) })} />)).toContain("No results for “paths metaphor”");
  });
});
