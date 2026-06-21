import { describe, expect, test } from "vitest";
import type { SearchResult } from "../types";
import { imageSources, toHtml, toMarkdown, toPlainText } from "./copyFormats";

function tweet(overrides: Partial<SearchResult> = {}): SearchResult {
  return {
    highlight_id: "x-1",
    work_id: "x-w-1",
    slug: "s",
    text:
      "my take\n\n— Quoting @c:\n> the original\n\n🔗 https://example.com/post\n\n![image](https://p/a.jpg)\n\n![image](https://p/b.jpg)",
    note: null,
    title: "a tweet",
    author: "alice",
    authors: [],
    work_type: "tweet",
    source_system: "x",
    source_id: "1",
    url: "https://x.com/alice/1",
    highlighted_at: null,
    tags: [],
    location: null,
    annotation_color: null,
    annotation_type: null,
    format: "plain",
    asset_path: null,
    citation: null,
    collections: [],
    zotero_link: null,
    relevance: null,
    snippet: "",
    ...overrides,
  };
}

describe("copyFormats", () => {
  test("toPlainText strips markdown, keeps URL, drops images, adds attribution", () => {
    const out = toPlainText(tweet());
    expect(out).not.toContain("![");
    expect(out).not.toContain(">");
    expect(out).toContain("the original");
    expect(out).toContain("https://example.com/post");
    expect(out.trimEnd().endsWith("— alice, a tweet")).toBe(true);
  });

  test("toMarkdown keeps images and quotes", () => {
    const out = toMarkdown(tweet());
    expect(out).toContain("![image](https://p/a.jpg)");
    expect(out).toContain("> ");
    expect(out).toContain("— alice, a tweet");
  });

  test("toHtml has both images, a blockquote, and attribution", () => {
    const out = toHtml(tweet());
    expect((out.match(/<img /g) || []).length).toBe(2);
    expect(out).toContain("<blockquote>");
    expect(out).toContain("— alice, a tweet");
  });

  test("toHtml escapes angle brackets in text", () => {
    const out = toHtml(tweet({ text: "a < b & c" }));
    expect(out).toContain("a &lt; b &amp; c");
  });

  test("single newlines within a paragraph become <br> in HTML", () => {
    const out = toHtml(tweet({ text: "line one\nline two\nline three" }));
    expect(out).toContain("line one<br>line two<br>line three");
  });

  test("single newlines within a paragraph are preserved in plain text", () => {
    const out = toPlainText(tweet({ text: "➤ a\n➤ b\n➤ c" }));
    expect(out).toContain("➤ a\n➤ b\n➤ c");
  });

  test("imageSources returns embedded URLs in order", () => {
    expect(imageSources(tweet())).toEqual([
      { url: "https://p/a.jpg" },
      { url: "https://p/b.jpg" },
    ]);
  });

  test("imageSources uses asset_path for Zotero image annotations", () => {
    expect(
      imageSources(tweet({ format: "image", asset_path: "/a/b.png", text: "" })),
    ).toEqual([{ path: "/a/b.png" }]);
  });

  test("hr renders as <hr> in HTML and --- in plain text", () => {
    const row = tweet({ text: "a\n\n---\n\nb" });
    expect(toHtml(row)).toContain("<hr>");
    expect(toPlainText(row)).toContain("---");
  });
});
