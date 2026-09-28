import { describe, expect, test, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { COPY_PREVIEW_KEY, CopyPreview, loadCopyPreview, saveCopyPreview } from "./CopyPreview";

function memStore(init: Record<string, string> = {}) {
  const m = new Map(Object.entries(init));
  return { getItem: (k: string) => m.get(k) ?? null, setItem: (k: string, v: string) => void m.set(k, v), m };
}

const html = (open: boolean) =>
  renderToStaticMarkup(
    <CopyPreview open={open} onToggle={vi.fn()} format="auto" onFormat={vi.fn()} text="> A quote — Author">
      <button>Find related</button>
    </CopyPreview>,
  );

describe("what ⌘⇧C copies folds away (ticket 08)", () => {
  test("closed: only the disclosure beside the pane's buttons, no preview or format", () => {
    const h = html(false);
    expect(h).toContain("Find related");
    expect(h).toContain('data-testid="copy-preview-toggle"');
    expect(h).toContain('aria-expanded="false"');
    expect(h).not.toContain("cite-box");
    expect(h).not.toContain("Citation format");
    expect(h).not.toContain("&gt; A quote");
  });

  test("open: the preview and its format, as before", () => {
    const h = html(true);
    expect(h).toContain('aria-expanded="true"');
    expect(h).toContain('data-testid="cite-box"');
    expect(h).toContain("&gt; A quote — Author");
    expect(h).toContain('aria-label="Citation format"');
    expect(h).toContain("Auto: Markdown in WriteFlex, rich text elsewhere");
  });

  test("closed by default; the choice is remembered", () => {
    const store = memStore();
    expect(loadCopyPreview(store)).toBe(false);
    saveCopyPreview(true, store);
    expect(store.m.get(COPY_PREVIEW_KEY)).toBe("open");
    expect(loadCopyPreview(store)).toBe(true);
    saveCopyPreview(false, store);
    expect(loadCopyPreview(store)).toBe(false);
    expect(loadCopyPreview(memStore({ [COPY_PREVIEW_KEY]: "junk" }))).toBe(false);
  });

  test("a store that throws is a closed preview, not a crash", () => {
    const bad = { getItem: () => { throw new Error("blocked"); }, setItem: () => { throw new Error("blocked"); } };
    expect(loadCopyPreview(bad)).toBe(false);
    expect(() => saveCopyPreview(true, bad)).not.toThrow();
  });
});
