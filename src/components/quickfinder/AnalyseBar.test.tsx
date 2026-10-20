import { describe, expect, test, vi } from "vitest";
import { isValidElement, type ReactElement, type ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { AnalyseBar, type AnalyseBarProps } from "./AnalyseBar";

type Host = { type: string; props: Record<string, unknown>; children: Host[] };
function expand(node: ReactNode): Host[] {
  if (node === null || node === undefined || typeof node === "boolean") return [];
  if (Array.isArray(node)) return node.flatMap(expand);
  if (!isValidElement(node)) return [];
  const el = node as ReactElement<Record<string, unknown>>;
  if (typeof el.type === "function") return expand((el.type as (p: unknown) => ReactNode)(el.props));
  if (typeof el.type !== "string") return expand(el.props.children as ReactNode);
  return [{ type: el.type, props: el.props, children: expand(el.props.children as ReactNode) }];
}
const buttons = (n: Host[]): Host[] => n.flatMap((h) => [...(h.type === "button" ? [h] : []), ...buttons(h.children)]);

const base = (o: Partial<AnalyseBarProps> = {}): AnalyseBarProps => ({
  count: "30 pieces · 64 matches",
  scope: "in Writing",
  term: "scaffolding",
  title: "Opens ArchiveScout on “scaffolding” in Writing",
  installed: true,
  onAnalyse: vi.fn(),
  onGet: vi.fn(),
  ...o,
});

describe("AnalyseBar", () => {
  test("shows the real counts and a button that hands the search over", () => {
    const p = base();
    const html = renderToStaticMarkup(<AnalyseBar {...p} />);
    expect(html).toContain("30 pieces · 64 matches");
    expect(html).toContain("in Writing");
    expect(html).toContain("Analyse in ArchiveScout");
    const [b] = buttons(expand(<AnalyseBar {...p} />));
    (b.props.onClick as () => void)();
    expect(p.onAnalyse).toHaveBeenCalledOnce();
    expect(b.props.disabled).toBe(false);
  });

  test("not known yet counts as installed", () => {
    expect(renderToStaticMarkup(<AnalyseBar {...base({ installed: null })} />)).toContain("Analyse in ArchiveScout");
  });

  test("not installed: the button says so and opens the release page, never dead", () => {
    const p = base({ installed: false });
    const html = renderToStaticMarkup(<AnalyseBar {...p} />);
    expect(html).toContain("ArchiveScout is not installed · get it");
    expect(html).not.toContain("Analyse in ArchiveScout");
    const [b] = buttons(expand(<AnalyseBar {...p} />));
    expect(b.props.disabled).toBeUndefined();
    (b.props.onClick as () => void)();
    expect(p.onGet).toHaveBeenCalledOnce();
  });

  test("a query with nothing to look up leaves the button off, and says why", () => {
    const [b] = buttons(expand(<AnalyseBar {...base({ term: "" })} />));
    expect(b.props.disabled).toBe(true);
    expect(String(b.props.title)).toContain("Type a word");
  });
});
