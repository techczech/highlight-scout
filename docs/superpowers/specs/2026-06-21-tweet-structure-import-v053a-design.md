# Highlight Scout v0.5.3-A — Structure-Aware Tweet Import

**Goal:** Parse the structure in Readwise Reader's tweet `html_content` so threads render with `---` separators between tweets and quoted/reply tweets render as `>` blockquotes, with images positioned inline.

**Status:** Approved (2026-06-21). First of three v0.5.3 sub-projects (A = structure; B = local webp images; C = OCR).

**Spec lineage:** items 3 + 4 of "Open follow-ups (v0.5.3)" in `2026-06-21-formatting-and-copy-v052-design.md`.

---

## Background

`src-tauri/src/import/readwise_tweets.rs::parse_html` currently **flattens** Reader's `html_content` to plain text — it emits `\n` on block tags and discards everything else, collecting images and links into separate lists that `tweet_common` appends at the end. This loses two structures that Reader marks explicitly:

**Thread** — each tweet is a `<p data-rw-toc-level="1" …>` block, separated by a literal delimiter:
```html
<hr class="twitter-thread-delimiter"/>
```
A thread's final block is a self-link only (`<a href="https://x.com/<user>/status/…">x.com/…</a>`).

**Quoted / reply tweet** — wrapped in:
```html
<article class="rw-embedded-tweet" data-rw-tweet-id="…">
  <header …><a href="…/claudeai">Claude</a> <a href="…/claudeai">@claudeai</a> …<svg/>… </header>
  <main> <p>quoted text…</p> <figure><img src="…/media/…"/></figure> </main>
  <footer data-rw-created-timestamp="1781024894000"><a>Posted Jun 9, 2026 at 5:08PM</a></footer>
</article>
```

Both are deterministic markers, so parsing is reliable rather than heuristic.

## Scope

**In scope (A):**
- Replace `parse_html` with a structure-aware parser (using the `tl` HTML-parser crate) that emits a complete markdown body.
- Reader importer passes that prebuilt body through `tweet_common` without re-appending images/links.
- Renderer + copy gain horizontal-rule (`---`) support.

**Out of scope:** local webp image download/migration (B), OCR (C). Images remain remote `https://pbs.twimg.com/...` URLs, now positioned inline.

**Migration:** none required as code. The Reader importer upserts by tweet id (dedup invariant `x-{id}`), so re-running **Settings → Import → "Readwise saved tweets"** after this ships overwrites the ~8,054 existing tweet highlights with the new structure-aware body.

---

## Output format (approved)

For a quoted tweet:
```markdown
<main tweet text, paragraphs preserved>

> **Claude** @claudeai
>
> <quoted text, paragraphs preserved>
>
> ![image](https://pbs.twimg.com/media/HKYwNlEWMAAJanX.png?name=orig)
>
> _Posted Jun 9, 2026 at 5:08PM_
```

For a thread:
```markdown
<tweet 1>

---

<tweet 2>

---

<tweet 3>
```

## Parsing rules

The parser walks the parsed DOM and emits markdown:

- **Thread delimiter** `<hr class="twitter-thread-delimiter">` → `\n\n---\n\n` between tweet blocks. Trailing empty segments (the self-link-only final block) are dropped, and no dangling `---` is left.
- **Embedded tweet** `<article class="rw-embedded-tweet">` → a blockquote (every emitted line prefixed `> `):
  - **Header**: the two `<a>` texts become `**<name>** <@handle>`. The decorative `<svg>` (X logo) is ignored.
  - **Main**: the quoted text (paragraphs preserved as blank `>` lines) followed by its images as `> ![image](url)`.
  - **Footer**: `> _<footer link text>_` (e.g. `_Posted Jun 9, 2026 at 5:08PM_`).
  - Reply-parent tweets use the same `rw-embedded-tweet` structure and get the same blockquote (no attempt to distinguish "quoting" from "replying to").
- **Images** `<figure><img src>` / `<img src>` → `![image](url)` inline at their DOM position. Keep only `/media/` URLs; drop `profile_images` (avatars).
- **Links** `<a href>` → `[<text>](<href>)`, except self/nav links (`x.com`, `twitter.com`, `t.co`, `pbs.twimg`) which render as their text only (or are dropped when the text is just the truncated URL).
- **Inline emphasis**: `<em>`/`<i>` → `*…*`; `<strong>`/`<b>` → `**…**`.
- **Blocks**: `<p>` boundaries → blank line (paragraph); `<br>` → single newline. Reader double-wraps (`<p><p>…</p></p>`); collapse redundant nesting.
- **Entities** unescaped (`&amp;`,`&lt;`,`&gt;`,`&quot;`,`&#39;`,`&nbsp;`).
- Leading/trailing whitespace trimmed; runs of >2 blank lines collapsed.

The top-level Reader `image_url` field is **no longer separately prepended** — images come from the `html_content` `<img>` tags (avoids duplication).

## Components

- **`src-tauri/Cargo.toml`** — add `tl` (lightweight pure-Rust HTML parser).
- **`src-tauri/src/import/readwise_tweets.rs`** — replace `parse_html` (and its `regex_all`/`html_unescape`/`collapse_blank_lines` helpers as needed) with `parse_tweet_html(&str) -> String` returning the complete markdown body. The `import()` loop sets the new `body_markdown` field on `TweetInput` instead of passing `text`/`images`/`article_urls` for re-assembly. `source_data.images` is still populated (metadata) by extracting `/media/` URLs.
- **`src-tauri/src/import/tweet_common.rs`** — add `pub body_markdown: Option<String>` to `TweetInput`. In `body()`, if `body_markdown` is `Some`, return it verbatim (skip the text+context+image assembly). The file/birdclaw path leaves it `None` and is unchanged.
- **`src/lib/markdown.tsx`** — `splitBlocks` recognises a line matching `^(---|\*\*\*|___)$` as `{ t: "hr" }`; `renderMarkdown` renders it as `<hr className="my-3 border-zinc-200" />`; `renderInlineMarkdown` (list rows) drops it.
- **`src/lib/copyFormats.ts`** — `toMarkdown`/`toPlainText` emit `---` for an hr block; `toHtml` emits `<hr>`.

## Error handling

- Missing/empty `html_content` → fall back to the document `title` (current behaviour preserved).
- `tl` parse never panics; if the expected structure is absent, the parser emits whatever text it finds (graceful degradation to a flat tweet).
- An `rw-embedded-tweet` missing a header/footer still emits the available parts.

## Testing

Rust unit tests in `readwise_tweets.rs`, using the two real fixtures captured during design:
- **Thread fixture** (RexDouglass `html_content`): asserts the body contains exactly 4 `---` separators (5 content tweets, trailing self-link block dropped), no leading/trailing `---`, and the tweet texts in order.
- **Quoted fixture** (karpathy → @claudeai `html_content`): asserts the main text precedes a blockquote; the blockquote contains `> **Claude** @claudeai`, the quoted text lines as `> …`, `> ![image](https://pbs.twimg.com/media/HKYwNlEWMAAJanX.png?name=orig)`, and `> _Posted Jun 9, 2026 at 5:08PM_`.
- **Image/avatar filter**: a `profile_images` `<img>` is excluded; a `/media/` `<img>` is included inline.
- **Existing tests** (`extracts_tweet_id_and_handle`) retained.

Frontend (Vitest) in `markdown.test.ts` / `copyFormats.test.ts`:
- `splitBlocks("a\n\n---\n\nb")` → `[para a, hr, para b]`.
- `toHtml` of a row whose text has `---` contains `<hr>`; `toPlainText` keeps `---`.

Manual QA: re-run "Readwise saved tweets" import; confirm a thread shows divider lines between tweets and a quoted tweet shows an indented blockquote with the author line, image, and date.
