# Highlight Scout v0.5.2 — Formatting & Copy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render tweet highlights correctly (inline images, blockquotes, bare-URL links, headings) and let the user copy any highlight as plain text, markdown, rich text, or image.

**Architecture:** A single inline tokenizer + block splitter in `src/lib/markdown.tsx` feeds three consumers — the React display renderer, and the HTML/plain-text copy serializers in a new `src/lib/copyFormats.ts`. Image bitmap copy is a new Rust command (`copy_image`) that downloads (tweets) or reads (Zotero) the image, decodes it with the `image` crate, and writes it to the clipboard. Rich text uses the clipboard plugin's `writeHtml`.

**Tech Stack:** TypeScript + React 19 (Vite), Tauri 2 (Rust), `@tauri-apps/plugin-clipboard-manager`, `image` crate, Vitest (new, for pure-logic unit tests).

**Spec:** `docs/superpowers/specs/2026-06-21-formatting-and-copy-v052-design.md`

---

## File Structure

**New files:**
- `src/lib/copyFormats.ts` — pure serializers: `toPlainText`, `toMarkdown`, `toHtml`, `imageSources`. No React imports.
- `src/lib/copyFormats.test.ts` — unit tests for the above.
- `src/lib/markdown.test.ts` — unit tests for `tokenize` and `splitBlocks`.
- `src/components/CopyMenu.tsx` — `Copy ▾` dropdown wiring the four formats + citation.
- `src-tauri/src/commands/clipboard.rs` — `copy_image` command + `decode_rgba` helper + Rust test.
- `vitest.config.ts` — minimal Vitest config (node environment).

**Modified files:**
- `src/lib/markdown.tsx` — extract `tokenize()` + `InlineToken`, add `splitBlocks()` + `Block`, refactor React renderers to consume them; add image/blockquote/heading/bare-URL support.
- `src/lib/clipboard.ts` — add `copyHtml`, `copyImage`.
- `src/lib/keybindings.ts` — add `copyRichText`, `copyImage` command ids; relabel copy commands; skip empty bindings in `comboMap`.
- `src/components/ReadingPane.tsx` — replace the lone "Copy citation" button with `<CopyMenu>`.
- `src/App.tsx` — copy command implementations now use `copyFormats`; wire `copyRichText` + `copyImage`.
- `src-tauri/src/commands/mod.rs` — `pub mod clipboard;`.
- `src-tauri/src/lib.rs` — register `commands::clipboard::copy_image`.
- `src-tauri/Cargo.toml` — add `image` dependency.
- `src-tauri/capabilities/default.json` — add `clipboard-manager:allow-write-image`, `clipboard-manager:allow-write-html`.
- `src/version.ts`, `src-tauri/tauri.conf.json` — bump to 0.5.2 + release notes.

---

### Task 1: Vitest test infrastructure

The frontend has no JS test runner. Add Vitest so the pure tokenizer/copy logic is TDD-able. Respect the npm release-age gate (use a version ≥7 days old; `vitest@^2.1.9` is well-aged).

**Files:**
- Create: `vitest.config.ts`
- Modify: `package.json` (devDependencies + `test` script)

- [ ] **Step 1: Install Vitest**

```bash
cd /Users/dominiklukes/gitrepos/14_apps-and-utilities/highlight-scout
bun add -d vitest@^2.1.9
```

- [ ] **Step 2: Create `vitest.config.ts`**

```ts
import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
  },
});
```

- [ ] **Step 3: Add a `test` script to `package.json`**

In the `"scripts"` block add:

```json
    "test": "vitest run"
```

- [ ] **Step 4: Add a smoke test to confirm the runner works**

Create `src/lib/smoke.test.ts`:

```ts
import { expect, test } from "vitest";

test("vitest runs", () => {
  expect(1 + 1).toBe(2);
});
```

- [ ] **Step 5: Run it**

Run: `bun run test`
Expected: 1 passed (`src/lib/smoke.test.ts`).

- [ ] **Step 6: Delete the smoke test and commit**

```bash
rm src/lib/smoke.test.ts
git add package.json vitest.config.ts bun.lockb
git commit -m "test: add Vitest for pure-logic unit tests"
```

---

### Task 2: Inline tokenizer (`tokenize` + `InlineToken`)

Extract a pure tokenizer from the existing `parseInline`, adding **image** and **bare-URL** tokens. Images are matched before links; bare URLs only match in plain-text runs (markdown links/images consume their own URLs first via alternation order).

**Files:**
- Modify: `src/lib/markdown.tsx:8-9` (replace `INLINE_RE`, add types + `tokenize`)
- Test: `src/lib/markdown.test.ts`

- [ ] **Step 1: Write failing tests**

Create `src/lib/markdown.test.ts`:

```ts
import { describe, expect, test } from "vitest";
import { tokenize } from "./markdown";

describe("tokenize", () => {
  test("plain text is one text token", () => {
    expect(tokenize("hello world")).toEqual([{ t: "text", v: "hello world" }]);
  });

  test("bold, italic, code", () => {
    expect(tokenize("**b** *i* `c`")).toEqual([
      { t: "bold", v: "b" },
      { t: "text", v: " " },
      { t: "italic", v: "i" },
      { t: "text", v: " " },
      { t: "code", v: "c" },
    ]);
  });

  test("markdown link", () => {
    expect(tokenize("see [docs](https://x.io/a)")).toEqual([
      { t: "text", v: "see " },
      { t: "link", text: "docs", url: "https://x.io/a" },
    ]);
  });

  test("image is its own token, matched before a link", () => {
    expect(tokenize("![pic](https://pbs.twimg.com/media/A.jpg)")).toEqual([
      { t: "image", alt: "pic", url: "https://pbs.twimg.com/media/A.jpg" },
    ]);
  });

  test("bare URL autolinks", () => {
    expect(tokenize("🔗 https://example.com/post")).toEqual([
      { t: "text", v: "🔗 " },
      { t: "link", text: "https://example.com/post", url: "https://example.com/post" },
    ]);
  });

  test("URL inside a markdown link is not double-matched", () => {
    expect(tokenize("[a](https://x.io)")).toEqual([
      { t: "link", text: "a", url: "https://x.io" },
    ]);
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `bun run test src/lib/markdown.test.ts`
Expected: FAIL — `tokenize` is not exported.

- [ ] **Step 3: Implement `tokenize` + types**

In `src/lib/markdown.tsx`, replace the `INLINE_RE` declaration (lines 8-9) with:

```tsx
export type InlineToken =
  | { t: "text"; v: string }
  | { t: "bold"; v: string }
  | { t: "italic"; v: string }
  | { t: "code"; v: string }
  | { t: "link"; text: string; url: string }
  | { t: "image"; alt: string; url: string };

// Ordered alternation: image before link so `![](url)` wins; bare URL last so it
// only matches plain-text runs (markdown link/image URLs are consumed by their
// own branch first).
const INLINE_RE = new RegExp(
  [
    /!\[(?<imgAlt>[^\]]*)\]\((?<imgUrl>https?:\/\/[^)\s]+)\)/,
    /\*\*(?<b1>[^*]+)\*\*/,
    /__(?<b2>[^_]+)__/,
    /\*(?<i1>[^*\n]+)\*/,
    /(?<![A-Za-z0-9])_(?<i2>[^_\n]+)_(?![A-Za-z0-9])/,
    /`(?<code>[^`]+)`/,
    /\[(?<linkText>[^\]]+)\]\((?<linkUrl>https?:\/\/[^)\s]+)\)/,
    /(?<bare>https?:\/\/[^\s)]+)/,
  ]
    .map((r) => r.source)
    .join("|"),
  "g",
);

/** Split inline markdown into typed tokens. Pure — shared by the React renderer
 * and the copy serializers. */
export function tokenize(text: string): InlineToken[] {
  const out: InlineToken[] = [];
  let last = 0;
  let m: RegExpExecArray | null;
  INLINE_RE.lastIndex = 0;
  while ((m = INLINE_RE.exec(text))) {
    if (m.index > last) out.push({ t: "text", v: text.slice(last, m.index) });
    const g = m.groups!;
    if (g.imgUrl != null) out.push({ t: "image", alt: g.imgAlt || "", url: g.imgUrl });
    else if (g.b1 != null) out.push({ t: "bold", v: g.b1 });
    else if (g.b2 != null) out.push({ t: "bold", v: g.b2 });
    else if (g.i1 != null) out.push({ t: "italic", v: g.i1 });
    else if (g.i2 != null) out.push({ t: "italic", v: g.i2 });
    else if (g.code != null) out.push({ t: "code", v: g.code });
    else if (g.linkUrl != null) out.push({ t: "link", text: g.linkText!, url: g.linkUrl });
    else if (g.bare != null) out.push({ t: "link", text: g.bare, url: g.bare });
    last = INLINE_RE.lastIndex;
  }
  if (last < text.length) out.push({ t: "text", v: text.slice(last) });
  return out;
}
```

> Note: the existing `parseInline` still references the old single-regex shape and will be rewritten in Task 4. It may not compile cleanly until then; that's expected. Tests in this task only exercise `tokenize`.

- [ ] **Step 4: Run to verify pass**

Run: `bun run test src/lib/markdown.test.ts`
Expected: PASS (6 tests).

- [ ] **Step 5: Commit**

```bash
git add src/lib/markdown.tsx src/lib/markdown.test.ts
git commit -m "feat: inline tokenizer with image + bare-URL tokens"
```

---

### Task 3: Block splitter (`splitBlocks` + `Block`)

Add a pure block splitter that recognises headings (`#`/`##`/`###`+) and `> ` blockquote groups, leaving everything else as paragraphs.

**Files:**
- Modify: `src/lib/markdown.tsx` (add `Block` type + `splitBlocks`)
- Test: `src/lib/markdown.test.ts` (append)

- [ ] **Step 1: Write failing tests**

Append to `src/lib/markdown.test.ts`:

```ts
import { splitBlocks } from "./markdown";

describe("splitBlocks", () => {
  test("headings clamp at level 3", () => {
    expect(splitBlocks("# A\n## B\n#### C")).toEqual([
      { t: "heading", level: 1, text: "A" },
      { t: "heading", level: 2, text: "B" },
      { t: "heading", level: 3, text: "C" },
    ]);
  });

  test("consecutive > lines group into one quote, marker stripped", () => {
    expect(splitBlocks("> one\n> two")).toEqual([
      { t: "quote", lines: ["one", "two"] },
    ]);
  });

  test("blank lines separate paragraphs", () => {
    expect(splitBlocks("a\n\nb")).toEqual([
      { t: "para", text: "a" },
      { t: "para", text: "b" },
    ]);
  });

  test("tweet shape: body, attribution line, quote, image", () => {
    const text = "my take\n\n— Quoting @c:\n> the original\n\n![image](https://p/x.jpg)";
    expect(splitBlocks(text)).toEqual([
      { t: "para", text: "my take" },
      { t: "para", text: "— Quoting @c:" },
      { t: "quote", lines: ["the original"] },
      { t: "para", text: "![image](https://p/x.jpg)" },
    ]);
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `bun run test src/lib/markdown.test.ts`
Expected: FAIL — `splitBlocks` is not exported.

- [ ] **Step 3: Implement `splitBlocks`**

Add to `src/lib/markdown.tsx` (after `tokenize`):

```tsx
export type Block =
  | { t: "heading"; level: 1 | 2 | 3; text: string }
  | { t: "quote"; lines: string[] }
  | { t: "para"; text: string };

const HEADING_RE = /^(#{1,6})\s+(.*)$/;
const QUOTE_RE = /^>\s?/;

/** Split block-level markdown into headings, quote groups and paragraphs.
 * Paragraphs are runs of non-blank, non-heading, non-quote lines. */
export function splitBlocks(text: string): Block[] {
  const lines = (text || "").split("\n");
  const blocks: Block[] = [];
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    const h = HEADING_RE.exec(line);
    if (h) {
      blocks.push({ t: "heading", level: Math.min(3, h[1].length) as 1 | 2 | 3, text: h[2] });
      i++;
      continue;
    }
    if (QUOTE_RE.test(line)) {
      const q: string[] = [];
      while (i < lines.length && QUOTE_RE.test(lines[i])) {
        q.push(lines[i].replace(QUOTE_RE, ""));
        i++;
      }
      blocks.push({ t: "quote", lines: q });
      continue;
    }
    if (line.trim() === "") {
      i++;
      continue;
    }
    const para: string[] = [];
    while (
      i < lines.length &&
      lines[i].trim() !== "" &&
      !QUOTE_RE.test(lines[i]) &&
      !HEADING_RE.test(lines[i])
    ) {
      para.push(lines[i]);
      i++;
    }
    blocks.push({ t: "para", text: para.join("\n") });
  }
  return blocks;
}
```

- [ ] **Step 4: Run to verify pass**

Run: `bun run test src/lib/markdown.test.ts`
Expected: PASS (all tokenize + splitBlocks tests).

- [ ] **Step 5: Commit**

```bash
git add src/lib/markdown.tsx src/lib/markdown.test.ts
git commit -m "feat: block splitter for headings and blockquotes"
```

---

### Task 4: React renderers consume tokens/blocks

Rewrite the React rendering to use `tokenize`/`splitBlocks`. Inline images render as `<img>` in block context and as a `🖼` marker in one-line list context. Blockquotes and headings render in block context.

**Files:**
- Modify: `src/lib/markdown.tsx:32-78` (replace `parseInline`, `renderInlineMarkdown`, `renderMarkdown`)

- [ ] **Step 1: Replace `parseInline` with a token renderer**

Replace the `parseInline` function (lines 32-59) with:

```tsx
function headingClass(level: 1 | 2 | 3): string {
  return level === 1
    ? "mt-2 text-lg font-semibold text-zinc-800"
    : level === 2
      ? "mt-2 text-base font-semibold text-zinc-800"
      : "mt-2 text-sm font-semibold text-zinc-800";
}

/** Render inline tokens to React. listMode collapses images to a 🖼 marker so
 * single-line rows stay single-line. */
function renderTokens(
  tokens: InlineToken[],
  terms: string[] | undefined,
  key: string,
  listMode = false,
): ReactNode[] {
  return tokens.map((tk, i) => {
    const k = `${key}-${i}`;
    switch (tk.t) {
      case "text":
        return <span key={k}>{markTerms(tk.v, terms, k)}</span>;
      case "bold":
        return <strong key={k}>{markTerms(tk.v, terms, k)}</strong>;
      case "italic":
        return <em key={k}>{markTerms(tk.v, terms, k)}</em>;
      case "code":
        return (
          <code key={k} className="rounded bg-zinc-100 px-1 text-[0.9em]">
            {tk.v}
          </code>
        );
      case "link":
        return (
          <button
            key={k}
            onClick={() => openUrl(tk.url)}
            className="text-blue-500 hover:underline"
          >
            {markTerms(tk.text, terms, k)}
          </button>
        );
      case "image":
        return listMode ? (
          <span key={k}>🖼 </span>
        ) : (
          <img
            key={k}
            src={tk.url}
            alt={tk.alt}
            className="my-2 max-h-80 max-w-full rounded border border-zinc-200"
          />
        );
    }
  });
}
```

- [ ] **Step 2: Rewrite the two exported renderers**

Replace `renderInlineMarkdown` and `renderMarkdown` (lines 61-78) with:

```tsx
/** Inline render (newlines collapsed to spaces) — for one-line list rows. */
export function renderInlineMarkdown(text: string, terms?: string[]): ReactNode {
  return <>{renderTokens(tokenize((text || "").replace(/\s+/g, " ").trim()), terms, "il", true)}</>;
}

/** Block render (headings, blockquotes, paragraphs) — for the reading pane. */
export function renderMarkdown(text: string, terms?: string[]): ReactNode {
  const blocks = splitBlocks((text || "").trim());
  return (
    <>
      {blocks.map((b, i) => {
        if (b.t === "heading") {
          const H = `h${b.level}` as "h1" | "h2" | "h3";
          return (
            <H key={i} className={headingClass(b.level)}>
              {renderTokens(tokenize(b.text), terms, `h${i}`)}
            </H>
          );
        }
        if (b.t === "quote") {
          return (
            <blockquote key={i} className="my-2 border-l-2 border-zinc-300 pl-3 text-zinc-600">
              {b.lines.map((l, j) => (
                <div key={j}>{renderTokens(tokenize(l), terms, `q${i}-${j}`)}</div>
              ))}
            </blockquote>
          );
        }
        return (
          <p key={i} className={i > 0 ? "mt-2" : undefined}>
            {renderTokens(tokenize(b.text.replace(/\n/g, " ")), terms, `p${i}`)}
          </p>
        );
      })}
    </>
  );
}
```

- [ ] **Step 3: Verify the build typechecks**

Run: `bunx tsc --noEmit`
Expected: no errors (the old `parseInline` and old `INLINE_RE` are fully replaced; `markTerms`, `openUrl`, `ReactNode`, `tokenize`, `splitBlocks`, `InlineToken` are all in scope).

- [ ] **Step 4: Run the unit tests (regression)**

Run: `bun run test`
Expected: PASS (tokenize + splitBlocks unchanged).

- [ ] **Step 5: Build the frontend**

Run: `bun run build`
Expected: build succeeds.

- [ ] **Step 6: Commit**

```bash
git add src/lib/markdown.tsx
git commit -m "feat: render inline images, blockquotes, headings, bare URLs"
```

---

### Task 5: Copy serializers (`copyFormats.ts`)

Pure functions producing each copy format from a `SearchResult`, built on `tokenize`/`splitBlocks`. No React.

**Files:**
- Create: `src/lib/copyFormats.ts`
- Test: `src/lib/copyFormats.test.ts`
- Reference: `src/lib/format.ts:73` (`markdownQuote`), `src/types.ts` (`SearchResult`)

- [ ] **Step 1: Write failing tests**

Create `src/lib/copyFormats.test.ts`:

```ts
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
});
```

- [ ] **Step 2: Run to verify failure**

Run: `bun run test src/lib/copyFormats.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement `copyFormats.ts`**

Create `src/lib/copyFormats.ts`:

```ts
import type { SearchResult } from "../types";
import { markdownQuote } from "./format";
import { type InlineToken, splitBlocks, tokenize } from "./markdown";

function escapeHtml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

function escapeAttr(s: string): string {
  return escapeHtml(s).replace(/"/g, "&quot;");
}

function inlinePlain(tokens: InlineToken[]): string {
  return tokens
    .map((tk) => {
      switch (tk.t) {
        case "text":
        case "bold":
        case "italic":
        case "code":
          return tk.v;
        case "link":
          return tk.text === tk.url ? tk.url : `${tk.text} (${tk.url})`;
        case "image":
          return "";
      }
    })
    .join("");
}

function inlineHtml(tokens: InlineToken[]): string {
  return tokens
    .map((tk) => {
      switch (tk.t) {
        case "text":
          return escapeHtml(tk.v);
        case "bold":
          return `<strong>${escapeHtml(tk.v)}</strong>`;
        case "italic":
          return `<em>${escapeHtml(tk.v)}</em>`;
        case "code":
          return `<code>${escapeHtml(tk.v)}</code>`;
        case "link":
          return `<a href="${escapeAttr(tk.url)}">${escapeHtml(tk.text)}</a>`;
        case "image":
          return `<img src="${escapeAttr(tk.url)}" alt="${escapeAttr(tk.alt)}">`;
      }
    })
    .join("");
}

function attribution(row: SearchResult): string {
  return `— ${row.author || "Unknown"}, ${row.title}`;
}

/** Readable text, all markdown stripped, images dropped, attribution appended. */
export function toPlainText(row: SearchResult): string {
  const body = splitBlocks((row.text || "").trim())
    .map((b) => {
      if (b.t === "heading") return inlinePlain(tokenize(b.text));
      if (b.t === "quote") return b.lines.map((l) => inlinePlain(tokenize(l))).join("\n");
      return inlinePlain(tokenize(b.text.replace(/\n/g, " ")));
    })
    .filter((s) => s.trim() !== "")
    .join("\n\n");
  return `${body}\n\n${attribution(row)}`;
}

/** Portable markdown: body verbatim (images + quotes kept) as a `>` quote + attribution. */
export function toMarkdown(row: SearchResult): string {
  return markdownQuote(row);
}

/** HTML for the clipboard `writeHtml` path — bold/italic/links, blockquotes,
 * headings, real <img>, and a trailing attribution line. */
export function toHtml(row: SearchResult): string {
  const blocks = splitBlocks((row.text || "").trim())
    .map((b) => {
      if (b.t === "heading") return `<h${b.level}>${inlineHtml(tokenize(b.text))}</h${b.level}>`;
      if (b.t === "quote")
        return `<blockquote>${b.lines.map((l) => inlineHtml(tokenize(l))).join("<br>")}</blockquote>`;
      return `<p>${inlineHtml(tokenize(b.text.replace(/\n/g, " ")))}</p>`;
    })
    .join("\n");
  return `${blocks}\n<p>${escapeHtml(attribution(row))}</p>`;
}

/** Image sources for copy-as-image, in document order. Zotero image annotations
 * expose their local file; tweets expose embedded remote URLs. Index 0 is primary. */
export function imageSources(row: SearchResult): Array<{ url?: string; path?: string }> {
  if (row.format === "image" && row.asset_path) return [{ path: row.asset_path }];
  return tokenize(row.text || "")
    .filter((tk): tk is Extract<InlineToken, { t: "image" }> => tk.t === "image")
    .map((tk) => ({ url: tk.url }));
}
```

- [ ] **Step 4: Run to verify pass**

Run: `bun run test src/lib/copyFormats.test.ts`
Expected: PASS (6 tests).

- [ ] **Step 5: Commit**

```bash
git add src/lib/copyFormats.ts src/lib/copyFormats.test.ts
git commit -m "feat: plain/markdown/html copy serializers + image source extraction"
```

---

### Task 6: Rust `copy_image` command

Download (http/https) or read (local path) an image, decode to RGBA, write to the clipboard. The decode is a testable pure helper.

**Files:**
- Create: `src-tauri/src/commands/clipboard.rs`
- Modify: `src-tauri/src/commands/mod.rs`, `src-tauri/src/lib.rs`, `src-tauri/Cargo.toml`, `src-tauri/capabilities/default.json`

- [ ] **Step 1: Add the `image` dependency**

Append to `src-tauri/Cargo.toml` under `[dependencies]`:

```toml
image = "0.25"
```

- [ ] **Step 2: Write `clipboard.rs` with the command + a tested decode helper**

Create `src-tauri/src/commands/clipboard.rs`:

```rust
use tauri::image::Image;
use tauri_plugin_clipboard_manager::ClipboardExt;

/// Decode encoded image bytes (PNG/JPEG/GIF/WebP) into (rgba, width, height).
fn decode_rgba(bytes: &[u8]) -> Result<(Vec<u8>, u32, u32), String> {
    let img = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    Ok((rgba.into_raw(), w, h))
}

/// Copy an image to the clipboard as a bitmap. `source` is an http(s) URL
/// (downloaded) or a local file path (read from disk).
#[tauri::command]
pub async fn copy_image(app: tauri::AppHandle, source: String) -> Result<(), String> {
    let bytes: Vec<u8> = if source.starts_with("http://") || source.starts_with("https://") {
        let resp = reqwest::get(&source).await.map_err(|e| e.to_string())?;
        resp.bytes().await.map_err(|e| e.to_string())?.to_vec()
    } else {
        std::fs::read(&source).map_err(|e| e.to_string())?
    };
    let (rgba, w, h) = decode_rgba(&bytes)?;
    let image = Image::new_owned(rgba, w, h);
    app.clipboard().write_image(&image).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_png_to_rgba() {
        let mut buf = std::io::Cursor::new(Vec::new());
        let img = image::RgbaImage::from_pixel(2, 3, image::Rgba([10, 20, 30, 255]));
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, image::ImageFormat::Png)
            .unwrap();
        let (rgba, w, h) = decode_rgba(&buf.into_inner()).unwrap();
        assert_eq!((w, h), (2, 3));
        assert_eq!(rgba.len(), (2 * 3 * 4) as usize);
        assert_eq!(&rgba[0..4], &[10, 20, 30, 255]);
    }

    #[test]
    fn rejects_non_image_bytes() {
        assert!(decode_rgba(b"not an image").is_err());
    }
}
```

- [ ] **Step 3: Register the module**

In `src-tauri/src/commands/mod.rs` add:

```rust
pub mod clipboard;
```

- [ ] **Step 4: Register the command**

In `src-tauri/src/lib.rs`, in the `tauri::generate_handler![ ... ]` list (after `commands::settings::set_autostart,` on line 118), add:

```rust
            commands::clipboard::copy_image,
```

- [ ] **Step 5: Grant clipboard permissions**

In `src-tauri/capabilities/default.json`, replace the `"clipboard-manager:allow-write-text"` line with:

```json
    "clipboard-manager:allow-write-text",
    "clipboard-manager:allow-write-image",
    "clipboard-manager:allow-write-html",
```

- [ ] **Step 6: Run the Rust tests**

Run: `cd src-tauri && cargo test decode`
Expected: PASS (`decodes_png_to_rgba`, `rejects_non_image_bytes`).

- [ ] **Step 7: Build the backend**

Run: `cd src-tauri && cargo build`
Expected: compiles (0 errors). `reqwest` is already a dependency; `image` is newly added.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/commands/clipboard.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/capabilities/default.json
git commit -m "feat: copy_image command (download/read, decode, clipboard bitmap)"
```

---

### Task 7: Clipboard frontend helpers

**Files:**
- Modify: `src/lib/clipboard.ts`

- [ ] **Step 1: Add `copyHtml` and `copyImage`**

Replace `src/lib/clipboard.ts` with:

```ts
import { writeHtml, writeText } from "@tauri-apps/plugin-clipboard-manager";
import { invoke } from "@tauri-apps/api/core";

/** Copy plain text to the clipboard. */
export async function copyText(text: string): Promise<void> {
  await writeText(text);
}

/** Copy rich text (HTML) — pastes as formatted content into Word/Pages/Gmail. */
export async function copyHtml(html: string): Promise<void> {
  await writeHtml(html);
}

/** Copy an image (by remote URL or local path) to the clipboard as a bitmap. */
export async function copyImage(source: string): Promise<void> {
  await invoke("copy_image", { source });
}
```

- [ ] **Step 2: Typecheck**

Run: `bunx tsc --noEmit`
Expected: no errors (`writeHtml` is exported by the clipboard plugin; `invoke` by `@tauri-apps/api/core`).

- [ ] **Step 3: Commit**

```bash
git add src/lib/clipboard.ts
git commit -m "feat: copyHtml + copyImage clipboard helpers"
```

---

### Task 8: `CopyMenu` component

A small dropdown exposing the four formats (+ citation). Closes on outside click and after a choice.

**Files:**
- Create: `src/components/CopyMenu.tsx`
- Reference: `src/lib/copyFormats.ts`, `src/lib/clipboard.ts`

- [ ] **Step 1: Implement `CopyMenu`**

Create `src/components/CopyMenu.tsx`:

```tsx
import { useEffect, useRef, useState } from "react";
import type { SearchResult } from "../types";
import { copyHtml, copyImage, copyText } from "../lib/clipboard";
import { imageSources, toHtml, toMarkdown, toPlainText } from "../lib/copyFormats";

interface Props {
  row: SearchResult;
  onToast: (msg: string) => void;
}

export function CopyMenu({ row, onToast }: Props) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const imgs = imageSources(row);

  useEffect(() => {
    if (!open) return;
    const onDoc = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onDoc);
    return () => document.removeEventListener("mousedown", onDoc);
  }, [open]);

  const run = (fn: () => Promise<void>, ok: string, fail: string) => {
    setOpen(false);
    fn()
      .then(() => onToast(ok))
      .catch(() => onToast(fail));
  };

  const copyRich = () =>
    copyHtml(toHtml(row)).catch(() => copyText(toPlainText(row)));

  const copyImg = () => {
    const src = imgs[0]?.path ?? imgs[0]?.url;
    if (!src) return Promise.reject(new Error("no image"));
    return copyImage(src);
  };

  const imageLabel = imgs.length > 1 ? `Image (1 of ${imgs.length})` : "Image";

  return (
    <div ref={ref} className="relative inline-block">
      <button
        onClick={() => setOpen((o) => !o)}
        className="rounded bg-zinc-100 px-1.5 py-0.5 text-zinc-500 hover:bg-zinc-200"
      >
        Copy ▾
      </button>
      {open && (
        <div className="absolute z-30 mt-1 w-44 rounded border border-zinc-200 bg-white py-1 text-sm shadow-lg">
          <Item onClick={() => run(() => copyText(toPlainText(row)), "Copied as plain text", "Copy failed")}>
            Plain text
          </Item>
          <Item onClick={() => run(() => copyText(toMarkdown(row)), "Copied as Markdown", "Copy failed")}>
            Markdown
          </Item>
          <Item onClick={() => run(copyRich, "Copied as rich text", "Copy failed")}>Rich text</Item>
          <Item
            disabled={imgs.length === 0}
            onClick={() =>
              run(
                copyImg,
                imgs.length > 1 ? `Copied image 1 of ${imgs.length}` : "Copied image",
                "Couldn't copy image",
              )
            }
          >
            {imageLabel}
          </Item>
          {row.citation && (
            <Item onClick={() => run(() => copyText(row.citation!), "Citation copied", "Copy failed")}>
              Citation
            </Item>
          )}
        </div>
      )}
    </div>
  );
}

function Item({
  children,
  onClick,
  disabled,
}: {
  children: React.ReactNode;
  onClick: () => void;
  disabled?: boolean;
}) {
  return (
    <button
      disabled={disabled}
      onClick={onClick}
      className="block w-full px-3 py-1 text-left text-zinc-700 hover:bg-zinc-50 disabled:cursor-default disabled:text-zinc-300 disabled:hover:bg-white"
    >
      {children}
    </button>
  );
}
```

- [ ] **Step 2: Typecheck**

Run: `bunx tsc --noEmit`
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add src/components/CopyMenu.tsx
git commit -m "feat: CopyMenu dropdown for four copy formats"
```

---

### Task 9: Wire `CopyMenu` into the reading pane

Replace the standalone "Copy citation" button with the menu (citation becomes a menu item, shown only when present).

**Files:**
- Modify: `src/components/ReadingPane.tsx:5` (import), `:99-106` (replace the citation button)

- [ ] **Step 1: Import `CopyMenu`**

In `src/components/ReadingPane.tsx`, after line 5 (`import { copyText } from "../lib/clipboard";`) add:

```tsx
import { CopyMenu } from "./CopyMenu";
```

- [ ] **Step 2: Replace the citation button with the menu**

Replace this block (lines 99-106):

```tsx
          {row.citation && (
            <button
              onClick={() => copyText(row.citation!).then(() => onToast("Citation copied"))}
              className="rounded bg-zinc-100 px-1.5 py-0.5 text-zinc-500 hover:bg-zinc-200"
            >
              Copy citation
            </button>
          )}
```

with:

```tsx
          <CopyMenu row={row} onToast={onToast} />
```

- [ ] **Step 3: Verify `copyText` is still used (avoid unused-import error)**

`copyText` may now be unused in `ReadingPane.tsx`. If `bunx tsc --noEmit` reports it unused, remove the `import { copyText } from "../lib/clipboard";` line (line 5).

Run: `bunx tsc --noEmit`
Expected: no errors after removing the unused import if flagged.

- [ ] **Step 4: Build**

Run: `bun run build`
Expected: succeeds.

- [ ] **Step 5: Commit**

```bash
git add src/components/ReadingPane.tsx
git commit -m "feat: reading pane Copy menu replaces lone citation button"
```

---

### Task 10: Command palette + keyboard wiring

Relabel the copy commands, add `copyRichText` and `copyImage`, and route `copyHighlight` (⌘C) through the plain-text serializer.

**Files:**
- Modify: `src/lib/keybindings.ts:11-13` (ids), `:43-45` (labels), `:111-115` (`comboMap`)
- Modify: `src/App.tsx:5-15` (imports), `:287-295` (copy fns), `:401-403` (command map)

- [ ] **Step 1: Add command ids**

In `src/lib/keybindings.ts`, in the `CommandId` union, replace lines 11-13:

```ts
  | "copyHighlight"
  | "copyMarkdown"
  | "copyCitation"
```

with:

```ts
  | "copyHighlight"
  | "copyMarkdown"
  | "copyRichText"
  | "copyImage"
  | "copyCitation"
```

- [ ] **Step 2: Relabel + add palette entries**

Replace the three copy entries (lines 43-45) in the `COMMANDS` array:

```ts
  { id: "copyHighlight", label: "Copy highlight", group: "Actions", default: "Mod+C" },
  { id: "copyMarkdown", label: "Copy as Markdown quote", group: "Actions", default: "Mod+Shift+C" },
  { id: "copyCitation", label: "Copy citation", group: "Actions", default: "Mod+Shift+K" },
```

with:

```ts
  { id: "copyHighlight", label: "Copy as plain text", group: "Actions", default: "Mod+C" },
  { id: "copyMarkdown", label: "Copy as Markdown", group: "Actions", default: "Mod+Shift+C" },
  { id: "copyRichText", label: "Copy as rich text", group: "Actions", default: "" },
  { id: "copyImage", label: "Copy image", group: "Actions", default: "" },
  { id: "copyCitation", label: "Copy citation", group: "Actions", default: "Mod+Shift+K" },
```

- [ ] **Step 3: Skip empty bindings in `comboMap`**

Replace the loop in `comboMap` (lines 112-115):

```ts
  const map: Record<string, CommandId> = {};
  const bindings = resolveBindings();
  for (const c of COMMANDS) map[bindings[c.id]] = c.id;
  return map;
```

with:

```ts
  const map: Record<string, CommandId> = {};
  const bindings = resolveBindings();
  for (const c of COMMANDS) {
    const b = bindings[c.id];
    if (b) map[b] = c.id;
  }
  return map;
```

- [ ] **Step 4: Update App.tsx imports**

In `src/App.tsx`, find the import of `copyText` and `markdownQuote`. Add `copyHtml` and `copyImage` to the clipboard import, and import the serializers. The clipboard import becomes:

```tsx
import { copyHtml, copyImage, copyText } from "./lib/clipboard";
```

Add (near the other `./lib/...` imports):

```tsx
import { imageSources, toHtml, toMarkdown, toPlainText } from "./lib/copyFormats";
```

If `markdownQuote` was imported from `./lib/format` only for copy, leave the `format` import as-is (other helpers may still be used); `markdownQuote` is no longer referenced in App.tsx after Step 5, so remove it from that import if `tsc` flags it unused.

- [ ] **Step 5: Replace the copy command implementations**

Replace lines 287-295:

```tsx
  const copyHighlight = async () => {
    if (activeRow) { await copyText(activeRow.text); showToast("Copied highlight"); }
  };
  const copyMarkdown = async () => {
    if (activeRow) { await copyText(markdownQuote(activeRow)); showToast("Copied as Markdown"); }
  };
  const copyCitationCmd = async () => {
    if (activeRow?.citation) { await copyText(activeRow.citation); showToast("Citation copied"); }
  };
```

with:

```tsx
  const copyHighlight = async () => {
    if (activeRow) { await copyText(toPlainText(activeRow)); showToast("Copied as plain text"); }
  };
  const copyMarkdown = async () => {
    if (activeRow) { await copyText(toMarkdown(activeRow)); showToast("Copied as Markdown"); }
  };
  const copyRichText = async () => {
    if (!activeRow) return;
    try { await copyHtml(toHtml(activeRow)); showToast("Copied as rich text"); }
    catch { await copyText(toPlainText(activeRow)); showToast("Copied as text"); }
  };
  const copyImageCmd = async () => {
    if (!activeRow) return;
    const imgs = imageSources(activeRow);
    if (!imgs.length) { showToast("No image to copy"); return; }
    const src = imgs[0].path ?? imgs[0].url!;
    try { await copyImage(src); showToast(imgs.length > 1 ? `Copied image 1 of ${imgs.length}` : "Copied image"); }
    catch { showToast("Couldn't copy image"); }
  };
  const copyCitationCmd = async () => {
    if (activeRow?.citation) { await copyText(activeRow.citation); showToast("Citation copied"); }
  };
```

- [ ] **Step 6: Wire the new commands into the command map**

In the `commands` object (lines 401-403), replace:

```tsx
    copyHighlight,
    copyMarkdown,
    copyCitation: copyCitationCmd,
```

with:

```tsx
    copyHighlight,
    copyMarkdown,
    copyRichText,
    copyImage: copyImageCmd,
    copyCitation: copyCitationCmd,
```

- [ ] **Step 7: Typecheck + build**

Run: `bunx tsc --noEmit && bun run build`
Expected: no errors; build succeeds. (If `markdownQuote` is flagged unused, remove it from the `./lib/format` import.)

- [ ] **Step 8: Commit**

```bash
git add src/lib/keybindings.ts src/App.tsx
git commit -m "feat: wire four-way copy into palette + keyboard"
```

---

### Task 11: Version bump + release notes + manual QA

**Files:**
- Modify: `src/version.ts:1` (+ prepend notes), `src-tauri/tauri.conf.json:4`

- [ ] **Step 1: Bump `version.ts`**

Set line 1 to:

```ts
export const APP_VERSION = "0.5.2";
```

Prepend a new entry as the first element of `RELEASE_NOTES`:

```ts
  {
    version: "0.5.2",
    notes: [
      "Tweets now render properly: inline images, quoted/reply context as blockquotes, and clickable article links.",
      "Copy any highlight four ways (reading pane Copy menu, or the command palette): plain text, Markdown, rich text (formatted with images, pastable into Word/Pages/Gmail), or the image itself.",
      "⌘C copies plain text, ⌘⇧C copies Markdown; rich text and image copy are in the Copy menu and command palette.",
    ],
  },
```

- [ ] **Step 2: Bump `tauri.conf.json`**

Set line 4 to:

```json
  "version": "0.5.2",
```

- [ ] **Step 3: Full test sweep**

Run: `bun run test && cd src-tauri && cargo test && cd ..`
Expected: all JS tests pass; all Rust tests pass.

- [ ] **Step 4: Build the app**

Run: `cargo tauri build`
Expected: produces `Highlight Scout_0.5.2_aarch64.dmg` and the `.app`.

- [ ] **Step 5: Manual QA (install + verify)**

```bash
osascript -e 'quit app "Highlight Scout"' 2>/dev/null; sleep 1
rm -rf "/Applications/Highlight Scout.app"
cp -R "src-tauri/target/release/bundle/macos/Highlight Scout.app" "/Applications/"
open "/Applications/Highlight Scout.app"
```

Verify:
- Search a tweet with an image → the image renders inline in the reading pane (not literal `![image](url)`); list row shows a `🖼` marker and stays one line.
- A tweet with a quoted/reply tweet → renders as an indented blockquote.
- A tweet with an article link (`🔗 …`) → the URL is clickable.
- Reading pane `Copy ▾` → Plain text, Markdown, Rich text, Image (disabled when no image), Citation (only when present).
- Copy as **rich text** → paste into Pages/TextEdit (rich mode) → formatted text with the image visible.
- Copy as **image** → paste into Preview/an image editor → the bitmap appears; toast says "Copied image 1 of N" for multi-image tweets.
- A Zotero image annotation still renders (regression check).

- [ ] **Step 6: Commit**

```bash
git add src/version.ts src-tauri/tauri.conf.json
git commit -m "chore: bump to 0.5.2 (formatting + four-way copy)"
```

---

## Self-Review

**1. Spec coverage:**
- Renderer: inline images (Task 4), blockquotes (Tasks 3-4), bare-URL autolink (Tasks 2,4), headings (Tasks 3-4), shared tokenizer (Task 2). ✓
- List rows stay single-line with 🖼 marker (Task 4). ✓
- Four copy formats — plain/markdown/rich/image (Tasks 5-10). ✓
- First-image-only bitmap + "1 of N" toast (Tasks 8,10). ✓
- Rich text via `writeHtml`, image via Rust `copy_image` using `image` crate + `write_image` (Tasks 6-7). ✓
- Capabilities `write-image`/`write-html` + Cargo `image` dep (Task 6). ✓
- UI: ReadingPane Copy menu + command palette entries; ⌘C plain, ⌘⇧C markdown (Tasks 8-10). ✓
- Error handling: image fail → toast; no image → disabled/`No image`; `writeHtml` fallback to text (Tasks 8,10). ✓
- Remote twimg URLs as-is; webp/OCR out of scope. ✓ (no task touches storage)

**2. Placeholder scan:** No TBD/TODO; every code step has full code. ✓

**3. Type consistency:** `InlineToken`/`Block` defined in Task 2-3, consumed identically in Tasks 4-5. `tokenize`/`splitBlocks`/`toPlainText`/`toMarkdown`/`toHtml`/`imageSources` names consistent across copyFormats, CopyMenu, App. `copy_image` command name matches `invoke("copy_image", { source })` and the `source` arg. `copyHtml`/`copyImage`/`copyText` consistent across clipboard.ts, CopyMenu, App. ✓
