# Highlight Scout v0.5.2 — Formatting & Copy Design

**Goal:** Make tweet highlights render correctly (inline images, blockquotes, links, headings) and let the user copy any highlight as plain text, markdown, rich text, or image.

**Status:** Approved (2026-06-21). Implementation target: v0.5.2.

---

## Background

Tweet highlights imported via `tweet_common.rs` store everything in the highlight's `text` field as markdown:

- Body text.
- Reply/quote context as `> ` blockquote lines (`— Replying to @x:`, `— Quoting @y:`).
- Article links as `🔗 https://…` bare URLs.
- Images as `\n\n![image](https://pbs.twimg.com/media/…)` — remote twimg URLs.

Scout's renderer (`src/lib/markdown.tsx`) only handles `**bold**`, `*italic*`, `` `code` ``, and `[text](url)`. It has **no image, blockquote, bare-URL, or heading support**, so tweets render with literal `![image](url)` text, flattened quote context, and unclickable links. Zotero image annotations (`format="image"`, local PNG at `asset_path`) already render correctly and are out of scope.

Copy is plain-text-only today (`writeText`): `⌘C` copies raw text, `⌘⇧C` copies a markdown quote, plus a citation copy. There is no rich-text or image copy.

## Scope

**In scope (v0.5.2):**
1. Renderer fix: inline images, blockquotes, bare-URL autolinking, headings.
2. Four-way copy: plain text, markdown, rich text (HTML), image (bitmap).

**Out of scope — deferred to v0.5.3:**
- Downloading tweet images and storing them locally as **webp** (Option B pipeline) + migrating the ~8,054 already-imported tweets.
- **OCR** of images so text inside them is searchable and copyable.

In v0.5.2 images are referenced by their remote twimg URLs as-is. Rich-text and image copy work against those remote URLs (HTML `<img src>` for rich text; on-demand download for bitmap copy).

---

## Part 1 — Renderer fix

**File:** `src/lib/markdown.tsx`

The renderer is refactored so a single inline tokenizer feeds multiple output serializers (React for display, HTML and plain-text for copy — see Part 2). This prevents display and copy formats from drifting apart.

### Inline-level additions (all output paths)

Extend the inline grammar, in this precedence order, with images matched before links:

- **Image** `![alt](url)` → React `<img>` (display) / `<img>` (HTML) / dropped (plain text). `url` matches `https?://[^)\s]+`.
- **Bold / italic / code / link** — unchanged.
- **Bare URL autolink** — a bare `https?://…` run not already part of a markdown link becomes a clickable link (display/HTML) / kept verbatim (plain text). The `🔗 ` prefix from article links is preserved as literal text before the link.

Images render as `<img>` with `max-h-80 w-auto rounded border` styling (matching the existing Zotero-image classes in `ReadingPane`). In one-line/list contexts (`renderInlineMarkdown`) images are **not** rendered inline — they are replaced with a `🖼` marker so rows stay single-line; full images appear in the reading pane / work view block render.

### Block-level additions (`renderMarkdown` only)

- **Blockquote**: consecutive lines beginning with `> ` group into one `<blockquote>` with left border + muted text styling. Nested `> ` content keeps the inner text.
- **Heading**: a line of `#`, `##`, or `###` followed by space renders as `<h1>`/`<h2>`/`<h3>` with descending size/weight. Levels deeper than 3 clamp to `<h3>`.
- Existing paragraph splitting on blank lines is retained for non-blockquote, non-heading lines.

`renderInlineMarkdown` (list rows) stays single-line: blockquote markers and heading markers are stripped to plain text, images become the `🖼` marker.

### Components consuming the renderer (no behavior change required)

`ReadingPane`, `WorkView`, `RelatedWindow`, `ResultsList` already call `renderMarkdown` / `renderInlineMarkdown`; they pick up the new formatting automatically. The `format === "image"` Zotero branch in those components is untouched.

### Remote image loading

CSP is `null` and `assetProtocol` covers `$HOME/**` only; remote `https` images load directly via the network with no config change. A broken/oversized image must not break layout: `<img>` keeps `max-h-80 max-w-full` and a neutral border; a failed load shows the browser default (acceptable for v0.5.2 — local caching lands in v0.5.3).

---

## Part 2 — Four-way copy

### Format definitions

For a highlight `row` (`SearchResult`), each format produces:

| Format | Contents | Mechanism |
|---|---|---|
| **Plain text** | Markdown stripped to readable prose: body text; reply/quote context as plain `Replying to @x: …` lines (no `>`); article URLs kept; images dropped; link text kept (URL kept only if it differs from the link text). Trailing `— {author}, {title}`. | `writeText` |
| **Markdown** | Portable markdown: body kept verbatim incl. `![](url)`, `> ` quotes, `[links]`; wrapped as a `> ` blockquote with trailing `— {author}, {title}`. (Extends the existing `markdownQuote`, but does **not** strip images.) | `writeText` |
| **Rich text** | HTML built from the shared tokenizer: rendered bold/italic/code/links, `<blockquote>`, headings, real `<img src="{remote url}">`, and a trailing attribution line. Pastes into Word/Pages/Gmail as formatted text with images. | `writeHtml` |
| **Image** | The first/primary image as a clipboard **bitmap**. Tweets: downloaded on demand from the twimg URL. Zotero: read from `asset_path`. Toast reports `Copied image 1 of N` when N > 1. Rich text and markdown still include *all* images. | Rust command + `write_image` |

Image URLs for plain/markdown/rich/image are extracted by running the inline tokenizer over `row.text` and collecting image tokens in document order; index 0 is primary. For a Zotero highlight (`format === "image"`), the single image is `row.asset_path` (a local file), and there is no embedded-image extraction.

### Components

- **`src/lib/copyFormats.ts`** (new): pure functions `toPlainText(row)`, `toMarkdown(row)`, `toHtml(row)`, and `imageSources(row): {url?: string; path?: string}[]`. Built on the shared tokenizer from `markdown.tsx` (extracted into a small `tokenize(text)` helper exported for reuse). No React imports — testable in isolation.
- **`src/lib/clipboard.ts`**: add `copyHtml(html, altText)` (calls plugin `writeHtml`) and `copyImage(src)` which invokes the Rust command. Keep existing `copyText`.
- **`src-tauri/src/commands/clipboard.rs`** (new): `#[tauri::command] async fn copy_image(app, source: String)`. If `source` is an `http(s)` URL, download with `reqwest`; otherwise read the local file. Decode to RGBA with the `image` crate and write to the clipboard via `tauri_plugin_clipboard_manager` `write_image`. Returns `Result<(), String>`.
- **`src-tauri/src/lib.rs`**: register `commands::clipboard::copy_image` in `invoke_handler`.
- **UI — Reading pane** (`ReadingPane.tsx`): replace the lone "Copy citation" button with a `Copy ▾` menu whose items are: Plain text, Markdown, Rich text, Image, and (when present) Citation. Image item is disabled when the row has no image.
- **UI — Command palette** (`src/lib/keybindings.ts` + `App.tsx`): add four commands — `copyPlainText`, `copyMarkdown` (existing, relabelled), `copyRichText`, `copyImage`. Keep `⌘C` → plain text and `⌘⇧C` → markdown; rich text and image are palette/menu only (no default chord). The existing `copyHighlight` command maps to plain text.

### Dependencies & permissions

- **Cargo**: add `image` (pure-Rust decode; PNG/JPEG/GIF/webp-decode — no libwebp encoder needed in v0.5.2).
- **`src-tauri/capabilities/default.json`**: add `clipboard-manager:allow-write-image` and `clipboard-manager:allow-write-html` to the existing `allow-write-text`.

### Error handling

- `copy_image` download failure / decode failure → returns `Err(msg)`; the frontend shows a toast `Couldn't copy image` and does nothing else.
- A highlight with no image: the Image copy item is disabled (reading pane) and the palette command shows `No image to copy`.
- `writeHtml` is best-effort; if it throws, fall back to `writeText` of the plain-text form and toast `Copied as text`.

### Testing

- **Renderer (`markdown` tests, TS/Vitest if present, else manual):** image token renders `<img>`; bare URL autolinks; `> ` lines group into one blockquote; `#`/`##`/`###` map to `h1`/`h2`/`h3`; list render stays single-line with `🖼` marker.
- **`copyFormats` (TS unit):** for a tweet fixture with body + quote + article link + two images:
  - `toPlainText` has no `![`, no `>`, keeps the article URL, ends with `— author, title`.
  - `toMarkdown` keeps both `![](url)` and the `> ` quote.
  - `toHtml` contains two `<img`, a `<blockquote`, and the attribution.
  - `imageSources` returns the two URLs in order.
- **`copy_image` (Rust):** local-file path decodes and writes without error (use a fixture PNG); an obviously bad path returns `Err`.
- **Manual:** copy rich text → paste into TextEdit (RTF mode)/Pages and confirm images + formatting; copy image → paste into Preview/an image editor.

---

## Open follow-ups (v0.5.3)

These are recorded so they are not lost; they are **not** part of v0.5.2:

1. Download tweet images, convert to **webp**, store under `archive/readings/assets/`, rewrite highlight text to local paths, and migrate the ~8,054 existing tweets (Option B). This is a hard-to-reverse storage decision and likely warrants an ADR at that time.
2. **OCR** images on import so embedded text is FTS-searchable and copyable.
3. Consider converting existing Zotero PNG assets to webp for storage consistency.
