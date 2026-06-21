# Tweet Structure-Aware Import (v0.5.3-A) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Parse Readwise Reader tweet `html_content` structure so threads render with `---` separators and quoted/reply tweets render as `>` blockquotes, with images inline.

**Architecture:** Replace the flattening `parse_html` in `readwise_tweets.rs` with a `tl`-based DOM walk (`parse_tweet_html`) that emits a complete markdown body. The Reader importer passes that prebuilt body through `tweet_common` via a new `body_markdown` field (the file/birdclaw path is untouched). The frontend renderer + copy serializers gain horizontal-rule (`---`) support. Re-running the import overwrites existing tweets (upsert by tweet id) — no migration code.

**Tech Stack:** Rust (Tauri backend), `tl` 0.7.8 (pure-Rust HTML parser), TypeScript/React (Vite), Vitest.

**Spec:** `docs/superpowers/specs/2026-06-21-tweet-structure-import-v053a-design.md`

---

## File Structure

**Modified:**
- `src-tauri/Cargo.toml` — add `tl = "0.7.8"`.
- `src-tauri/src/import/tweet_common.rs` — add `body_markdown: Option<String>` to `TweetInput`; `body()` returns it verbatim when set.
- `src-tauri/src/import/readwise_tweets.rs` — replace `parse_html` with `parse_tweet_html` (tl DOM walk → markdown); `import()` sets `body_markdown`; still extracts `/media/` image URLs for `source_data.images`.
- `src/lib/markdown.tsx` — `splitBlocks` recognises `---`/`***`/`___` as an `hr` block; `renderMarkdown` renders `<hr>`; list render drops it.
- `src/lib/copyFormats.ts` — handle the `hr` block (`---` in markdown/plain, `<hr>` in HTML).
- `src/version.ts`, `src-tauri/tauri.conf.json` — bump to 0.5.3.

**Tests:** Rust unit tests in `readwise_tweets.rs` (two real fixtures); Vitest additions in `markdown.test.ts` and `copyFormats.test.ts`.

---

### Task 1: `tl` dependency + `body_markdown` passthrough

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/import/tweet_common.rs`

- [ ] **Step 1: Add the `tl` dependency**

Append under `[dependencies]` in `src-tauri/Cargo.toml`:

```toml
tl = "0.7.8"
```

- [ ] **Step 2: Add a failing test for the `body_markdown` passthrough**

In `src-tauri/src/import/tweet_common.rs`, inside the existing `#[cfg(test)] mod tests`, add:

```rust
    #[test]
    fn prebuilt_body_markdown_is_used_verbatim() {
        let t = TweetInput {
            tweet_id: "1".into(),
            text: "ignored when body_markdown is set".into(),
            images: vec!["https://pbs.twimg.com/media/AAA.jpg".into()],
            body_markdown: Some("PREBUILT\n\n---\n\n> quote".into()),
            ..Default::default()
        };
        let (_w, h, _title) = make_records(&t, "2026-06-21T00:00:00Z");
        assert_eq!(h.text, "PREBUILT\n\n---\n\n> quote");
        // images are NOT re-appended when a prebuilt body is supplied
        assert!(!h.text.contains("![image]"));
    }
```

- [ ] **Step 3: Run it to verify it fails**

Run: `cd src-tauri && cargo test prebuilt_body_markdown`
Expected: FAIL to compile — `TweetInput` has no field `body_markdown`.

- [ ] **Step 4: Add the field**

In `src-tauri/src/import/tweet_common.rs`, in the `TweetInput` struct (after `pub quoted_handle: Option<String>,`), add:

```rust
    /// When set, used as the highlight body verbatim (the Reader importer
    /// pre-builds structure-aware markdown). The file/birdclaw path leaves
    /// this `None` and `body()` assembles the body from the fields.
    pub body_markdown: Option<String>,
```

- [ ] **Step 5: Make `body()` honour it**

In `src-tauri/src/import/tweet_common.rs`, at the very top of `fn body(t: &TweetInput) -> String {`, add:

```rust
    if let Some(b) = &t.body_markdown {
        return b.clone();
    }
```

- [ ] **Step 6: Run the test to verify it passes**

Run: `cd src-tauri && cargo test prebuilt_body_markdown`
Expected: PASS. Also run `cargo test --lib` — all existing tests still pass (31 + this one).

- [ ] **Step 7: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/import/tweet_common.rs
git commit -m "feat: tl dep + body_markdown passthrough on TweetInput"
```

---

### Task 2: `parse_tweet_html` — the structure-aware parser

This is the core task. The exact `tl` API calls must be discovered against the real crate, so this is **strongly test-driven**: write the fixture assertions first, then implement `parse_tweet_html` and iterate until green. Do **not** assert whole-string equality — assert the structural substrings below (robust to whitespace).

**Files:**
- Modify: `src-tauri/src/import/readwise_tweets.rs`

- [ ] **Step 1: Write the failing tests with the two real fixtures**

In `src-tauri/src/import/readwise_tweets.rs`, replace the existing `parses_html_text_media_and_links` test with the following (keep `extracts_tweet_id_and_handle`). These fixtures are the real Reader `html_content` captured from the user's library.

```rust
    const THREAD_HTML: &str = r##"<div><p data-rw-toc-level="1" data-rw-toc-title="It's hard to overstate how disappointing academia's reaction to LLMs..."><p>It's hard to overstate how disappointing academia's reaction to LLMs have been. We got scifi level automation in less than a decade and the response has been almost entirely protectionist politics. Do you have any idea how embarrassing that is? Institutions fighting for relevance  </p></p><hr class="twitter-thread-delimiter"/><p data-rw-toc-level="1" data-rw-toc-title="It's now possible to provide every student of any income..."><p>It's now possible to provide every student of any income level and individual learning capabilities a nearly free always on personal tutor in any subject from birth. Something that would have been considered a miracle in any previous decade. Remember one laptop per child?  </p></p><hr class="twitter-thread-delimiter"/><p data-rw-toc-level="1" data-rw-toc-title="The response has been catastrophically embarrassing"><p>The response has been catastrophically embarrassing. It's so profoundly obvious that the mission isn't actually education and that who are supposed to be the state of the art are actually several decades behind luddites. It really and truly is sorting people and institutions.  </p></p><hr class="twitter-thread-delimiter"/><p data-rw-toc-level="1" data-rw-toc-title="Let me put it this way"><p>Let me put it this way</p><p>-You can be the cutting edge and not care about teaching</p><p>-Or you can care only about teaching</p><p>But you can't both be out of date and also not excited about more ways to teach. There's no self referential luddites box.  </p></p><hr class="twitter-thread-delimiter"/><p data-rw-toc-level="1" data-rw-toc-title="Also it's deeply funny watching an industry that spent a..."><p>Also it's deeply funny watching an industry that spent a decade trashing protectionism do a 180 the second their industry was threatened. Globalization is obviously good unless it's against clankers then suddenly it's lifetime bans for a single hallucinated cite.  </p></p><hr class="twitter-thread-delimiter"/><p data-rw-toc-level="1" data-rw-toc-title="Tweet by RexDouglass"><p><a href="https://x.com/RexDouglass/status/2067695321740640530" rel="nofollow">x.com/RexDouglass/st…</a> </p></p></div>"##;

    const QUOTE_HTML: &str = r##"<div><p data-rw-toc-level="1" data-rw-toc-title="This is a super exciting release..."><p>This is a super exciting release - Claude Fable 5 is the same underlying model as Mythos but with added safeguards. The benchmarks are great and it's SOTA on everything by a margin but I'll add that <em>qualitatively</em> also, this is a major-version-bump-deserving step change forward. Really looking forward to all the things people build!  </p></p>
<article class="rw-embedded-tweet" data-rw-tweet-id="2064394151441863006">
<header class="rw-embedded-tweet-header">
<div>
<figure><img src="https://pbs.twimg.com/profile_images/1950950107937185792/QOfEjFoJ.jpg"/></figure>
</div>
<div>
<span><a href="https://twitter.com/claudeai">Claude</a></span>
<span><a href="https://twitter.com/claudeai">@claudeai</a></span>
</div>
<div>
<a href="https://twitter.com/claudeai/status/2064394151441863006">
<svg aria-label="X" fill="none" height="24" viewbox="0 0 24 24" width="24" xmlns="http://www.w3.org/2000/svg"></svg>
</a>
</div>
</header>
<main>
<p><p>Fable 5 is state-of-the-art on nearly all tested benchmarks, with exceptional performance in software engineering, knowledge work, scientific research, and vision.</p><p>The longer and more complex the task, the larger Fable 5's lead over our other models.  </p><p><figure><img alt="Image" src="https://pbs.twimg.com/media/HKYwNlEWMAAJanX.png?name=orig"/></figure> </p></p>
</main>
<footer class="rw-embedded-tweet-footer" data-rw-created-timestamp="1781024894000">
<span>
<a href="https://twitter.com/claudeai/status/2064394151441863006">Posted Jun 9, 2026 at 5:08PM</a>
</span>
</footer>
</article></div>"##;

    #[test]
    fn thread_html_uses_hr_separators() {
        let md = parse_tweet_html(THREAD_HTML);
        // 5 content tweets → 4 separators; trailing self-link block dropped.
        assert_eq!(md.matches("\n---\n").count(), 4, "got:\n{md}");
        assert!(!md.trim_start().starts_with("---"));
        assert!(!md.trim_end().ends_with("---"));
        assert!(md.contains("It's hard to overstate how disappointing academia"));
        assert!(md.contains("Remember one laptop per child?"));
        assert!(md.contains("Also it's deeply funny watching an industry"));
        // the final self-link-only tweet is dropped
        assert!(!md.contains("x.com/RexDouglass/st"));
    }

    #[test]
    fn quoted_tweet_becomes_blockquote_with_inline_image_and_date() {
        let md = parse_tweet_html(QUOTE_HTML);
        // main tweet text is present and NOT quoted
        assert!(md.contains("This is a super exciting release - Claude Fable 5"));
        assert!(md.contains("*qualitatively*"), "em → *italic*; got:\n{md}");
        // quoted tweet is a blockquote with author + handle
        assert!(md.contains("> **Claude** @claudeai"), "got:\n{md}");
        assert!(md.contains("> Fable 5 is state-of-the-art on nearly all tested benchmarks"));
        // quoted image inline, inside the quote, /media/ only
        assert!(md.contains("> ![image](https://pbs.twimg.com/media/HKYwNlEWMAAJanX.png?name=orig)"));
        // avatar (profile_images) excluded everywhere
        assert!(!md.contains("profile_images"));
        // footer date as a muted line inside the quote
        assert!(md.contains("> _Posted Jun 9, 2026 at 5:08PM_"));
    }

    #[test]
    fn falls_back_gracefully_on_plain_html() {
        let md = parse_tweet_html("<p>just a plain tweet</p>");
        assert_eq!(md.trim(), "just a plain tweet");
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cd src-tauri && cargo test --lib readwise_tweets`
Expected: FAIL to compile — `parse_tweet_html` does not exist yet.

- [ ] **Step 3: Implement `parse_tweet_html`**

Replace the existing `parse_html` function (and remove its now-unused helpers `regex_all`, plus keep `html_unescape`/`dedup` only if your implementation uses them) in `src-tauri/src/import/readwise_tweets.rs` with a `tl`-based DOM walk.

Algorithm (emit markdown into a `String`, then normalise whitespace):
1. `let dom = tl::parse(html, tl::ParserOptions::default())` (it does not error for malformed input). Get `let parser = dom.parser();`.
2. Walk top-level nodes in document order. For each node, dispatch on tag name/class:
   - **`<hr>` with class containing `twitter-thread-delimiter`** → push a thread separator marker (e.g. `\n\n---\n\n`).
   - **`<article>` with class containing `rw-embedded-tweet`** → render a blockquote (see step 4) and push it.
   - **`<p>`** → recurse its children as inline content, then ensure a blank-line paragraph break after.
   - **`<em>`/`<i>`** inline → wrap inner text in `*…*`; **`<strong>`/`<b>`** → `**…**`.
   - **`<img>`** whose `src` contains `/media/` → `![image](<src>)`; ignore `profile_images` and any non-`/media/` src; ignore `<svg>` entirely.
   - **`<a>`** → `[<inner text>](<href>)`, except when `href` contains `x.com`/`twitter.com`/`t.co`/`pbs.twimg` emit just the inner text (and drop it if the text is empty/just a truncated URL like `x.com/…`).
   - **`<br>`** → single `\n`.
   - **raw text nodes** → append unescaped text (`tl` exposes inner text already entity-decoded via `inner_text`; if you build text manually, reuse `html_unescape`).
3. **Blockquote builder** for an `rw-embedded-tweet` article: collect the header author name + handle (the two `<a>` inner texts inside `<header>`, in order → `**<name>** <@handle>`), the `<main>` content (recurse: text paragraphs + `/media/` images), and the `<footer>` link text (→ `_<text>_`). Join these with blank lines, then prefix **every** line with `> ` (blank lines become `>`). Surround the whole blockquote with blank lines.
4. **Normalise**: split into thread segments on the `---` marker, `trim()` each segment, drop empty segments (this removes the trailing self-link-only block), then re-join non-empty segments with `\n\n---\n\n`. Collapse runs of 3+ newlines to 2 within a segment. Final `trim()`.

A starting skeleton (adjust to the real `tl` 0.7.8 API as the compiler/tests guide you — method names like `as_tag`, `name`, `attributes`, `get`/`try_get`, `children`, `inner_text` may differ slightly):

```rust
/// Parse Reader tweet html_content into a complete markdown body:
/// thread <hr> → `---`, rw-embedded-tweet <article> → `>` blockquote,
/// /media/ <img> → inline ![image](url). Pure; never panics.
pub fn parse_tweet_html(html: &str) -> String {
    let dom = tl::parse(html, tl::ParserOptions::default());
    let parser = dom.parser();
    let mut out = String::new();
    for child in dom.children() {
        if let Some(node) = child.get(parser) {
            walk(node, parser, &mut out, false);
        }
    }
    normalise(&out)
}

// `quoted` = true when inside an rw-embedded-tweet (prefix handled by caller).
fn walk(node: &tl::Node, parser: &tl::Parser, out: &mut String, _quoted: bool) {
    // match on node: tags vs raw text; dispatch per the algorithm above.
    // Use a helper `render_blockquote(article, parser) -> String` for articles
    // and `inner_markdown(node, parser) -> String` for inline content.
    // ... iterate against the tests ...
}
```

> The implementer should write the helper functions (`walk`, `render_blockquote`, `inner_markdown`, `normalise`) as needed, compiling and running the three tests repeatedly until all pass. If the `tl` traversal API is materially different from the skeleton, follow the compiler and the crate docs — the tests define the contract, not the skeleton.

- [ ] **Step 4: Run the tests to green**

Run: `cd src-tauri && cargo test --lib readwise_tweets`
Expected: `thread_html_uses_hr_separators`, `quoted_tweet_becomes_blockquote_with_inline_image_and_date`, `falls_back_gracefully_on_plain_html`, and `extracts_tweet_id_and_handle` all PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/import/readwise_tweets.rs
git commit -m "feat: tl-based structure-aware tweet html parser"
```

---

### Task 3: Wire the Reader importer to the new parser

**Files:**
- Modify: `src-tauri/src/import/readwise_tweets.rs` (the `import()` loop)

- [ ] **Step 1: Build a `/media/` image extractor for source_data**

`source_data.images` is metadata used elsewhere; keep populating it. Add a small helper near `parse_tweet_html`:

```rust
/// Collect distinct /media/ image URLs from html (for source_data metadata).
pub fn media_images(html: &str) -> Vec<String> {
    let dom = tl::parse(html, tl::ParserOptions::default());
    let parser = dom.parser();
    let mut out: Vec<String> = Vec::new();
    for img in dom.query_selector("img").into_iter().flatten() {
        if let Some(tag) = img.get(parser).and_then(|n| n.as_tag()) {
            if let Some(Some(src)) = tag.attributes().get("src") {
                let s = src.as_utf8_str().to_string();
                if s.contains("/media/") && !out.contains(&s) {
                    out.push(s);
                }
            }
        }
    }
    out
}
```

> If `query_selector`/`as_utf8_str` differ in `tl` 0.7.8, adapt — the goal is "distinct `/media/` `src` values." A unit test is optional here since `parse_tweet_html` tests already cover image handling; if you add one, assert `media_images(QUOTE_HTML) == vec!["https://pbs.twimg.com/media/HKYwNlEWMAAJanX.png?name=orig".to_string()]`.

- [ ] **Step 2: Replace the per-doc body assembly in `import()`**

In `src-tauri/src/import/readwise_tweets.rs`, the `for d in &page.results` loop currently calls `parse_html` and builds `text`/`imgs`/`arts`. Replace that body-building block with:

```rust
        for d in &page.results {
            let Some(su) = d.source_url.as_deref() else { continue };
            let Some(id) = tweet_id(su) else { continue };

            let body = match d.html_content.as_deref() {
                Some(h) if !h.trim().is_empty() => parse_tweet_html(h),
                _ => String::new(),
            };
            let body = if body.trim().is_empty() {
                d.title.clone().unwrap_or_default()
            } else {
                body
            };
            if body.trim().is_empty() { continue; }

            let imgs = match d.html_content.as_deref() {
                Some(h) => media_images(h),
                None => Vec::new(),
            };

            let t = TweetInput {
                tweet_id: id.clone(),
                body_markdown: Some(body),
                author_handle: handle(su),
                author_name: d.author.clone(),
                created_at: None,
                url: Some(format!("https://x.com/{}/status/{}", handle(su).unwrap_or_default(), id)),
                images: imgs,
                saved_as: Some("likes".into()),
                ..Default::default()
            };
            let (work, highlight, title) = make_records(&t, &now);
            let author = work.author.clone();
            if seen.insert(work.id.clone()) { works.push(work); }
            highlights.push((highlight, title, author));
        }
```

(`text`, `article_urls`, and the separate `image_url` prepend are intentionally dropped — text/images now come from `parse_tweet_html`; `images` carries only metadata.)

- [ ] **Step 3: Verify the backend compiles and all tests pass**

Run: `cd src-tauri && cargo build && cargo test --lib`
Expected: compiles (remove any now-unused imports/helpers the compiler flags — e.g. if `parse_html`'s old helpers are dead, delete them); all tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/import/readwise_tweets.rs
git commit -m "feat: Reader importer emits structure-aware markdown body"
```

---

### Task 4: Renderer `---` horizontal-rule support

**Files:**
- Modify: `src/lib/markdown.tsx`
- Test: `src/lib/markdown.test.ts`

- [ ] **Step 1: Write the failing test**

Append to `src/lib/markdown.test.ts` (inside the existing `describe("splitBlocks", ...)` or a new one):

```ts
describe("splitBlocks horizontal rule", () => {
  test("a --- line becomes an hr block between paragraphs", () => {
    expect(splitBlocks("a\n\n---\n\nb")).toEqual([
      { t: "para", text: "a" },
      { t: "hr" },
      { t: "para", text: "b" },
    ]);
  });

  test("*** and ___ are also hr", () => {
    expect(splitBlocks("***")).toEqual([{ t: "hr" }]);
    expect(splitBlocks("___")).toEqual([{ t: "hr" }]);
  });
});
```

- [ ] **Step 2: Run to verify it fails**

Run: `bun run test src/lib/markdown.test.ts`
Expected: FAIL — splitBlocks returns a `para` with text `---`, not an `hr` block.

- [ ] **Step 3: Add the `hr` block type + detection**

In `src/lib/markdown.tsx`, extend the `Block` union:

```tsx
export type Block =
  | { t: "heading"; level: 1 | 2 | 3; text: string }
  | { t: "quote"; lines: string[] }
  | { t: "hr" }
  | { t: "para"; text: string };
```

Add a constant near `HEADING_RE`/`QUOTE_RE`:

```tsx
const HR_RE = /^(---+|\*\*\*+|___+)$/;
```

In `splitBlocks`, add a check **before** the heading check inside the `while` loop (so a `---` line is never treated as paragraph text):

```tsx
    if (HR_RE.test(line.trim())) {
      blocks.push({ t: "hr" });
      i++;
      continue;
    }
```

- [ ] **Step 4: Render the `hr` block and drop it in list mode**

In `renderMarkdown` (the `blocks.map(...)`), add a branch (before the paragraph fallback):

```tsx
        if (b.t === "hr") {
          return <hr key={i} className="my-3 border-zinc-200" />;
        }
```

`renderInlineMarkdown` runs the inline tokenizer on a single collapsed line and never calls `splitBlocks`, so list rows already ignore block-level `---` (a stray `---` in a one-line preview renders as literal text, which is acceptable). No change needed there.

- [ ] **Step 5: Run the tests + typecheck + build**

Run: `bun run test && bunx tsc --noEmit && bun run build`
Expected: all pass; build succeeds. (The TS `switch`/`if` over `Block` now includes `hr`.)

- [ ] **Step 6: Commit**

```bash
git add src/lib/markdown.tsx src/lib/markdown.test.ts
git commit -m "feat: render --- as a horizontal rule"
```

---

### Task 5: `copyFormats` horizontal-rule handling

**Files:**
- Modify: `src/lib/copyFormats.ts`
- Test: `src/lib/copyFormats.test.ts`

- [ ] **Step 1: Write the failing tests**

Append to `src/lib/copyFormats.test.ts` (inside the `describe("copyFormats", ...)` block):

```ts
  test("hr renders as <hr> in HTML and --- in plain/markdown", () => {
    const row = tweet({ text: "a\n\n---\n\nb" });
    expect(toHtml(row)).toContain("<hr>");
    expect(toPlainText(row)).toContain("---");
  });
```

- [ ] **Step 2: Run to verify it fails**

Run: `bun run test src/lib/copyFormats.test.ts`
Expected: FAIL — `toHtml` has no `<hr>`; the `hr` block falls through the `splitBlocks` map (TypeScript may also error that the `hr` case is unhandled once Task 4 added it to the union — handle it here).

- [ ] **Step 3: Handle the `hr` block in the serializers**

In `src/lib/copyFormats.ts`, in **both** `toPlainText` and `toHtml`, add an `hr` branch in the `.map((b) => { ... })` over `splitBlocks(...)`:

In `toPlainText` (returns a string per block; joined by `\n\n`):

```ts
      if (b.t === "hr") return "---";
```

In `toHtml` (returns an HTML string per block; joined by `\n`):

```ts
      if (b.t === "hr") return "<hr>";
```

Place each branch alongside the existing `heading`/`quote` branches, before the paragraph fallback.

- [ ] **Step 4: Run tests + typecheck**

Run: `bun run test && bunx tsc --noEmit`
Expected: all pass (25 + new tests); no type errors (the `Block` union's `hr` case is now handled in copyFormats too).

- [ ] **Step 5: Commit**

```bash
git add src/lib/copyFormats.ts src/lib/copyFormats.test.ts
git commit -m "feat: copy serializers handle the --- horizontal rule"
```

---

### Task 6: Version bump + manual QA

**Files:**
- Modify: `src/version.ts`, `src-tauri/tauri.conf.json`

- [ ] **Step 1: Bump `version.ts`**

Set line 1 to:

```ts
export const APP_VERSION = "0.5.3";
```

Prepend a new entry as the first element of `RELEASE_NOTES`:

```ts
  {
    version: "0.5.3",
    notes: [
      "Tweets imported from Readwise now keep their structure: threads show a divider between each tweet, and quoted/replied tweets appear as an indented quote with the author, image and date.",
      "Re-run Settings → Import → \"Readwise saved tweets\" to refresh existing tweets with the new formatting.",
    ],
  },
```

- [ ] **Step 2: Bump `tauri.conf.json`**

Set the `"version"` line to:

```json
  "version": "0.5.3",
```

- [ ] **Step 3: Full test sweep**

Run: `bun run test && cd src-tauri && cargo test && cd ..`
Expected: all JS tests pass; all Rust tests pass.

- [ ] **Step 4: Build**

Run: `cargo tauri build`
Expected: produces `Highlight Scout_0.5.3_aarch64.dmg` and the `.app`.

- [ ] **Step 5: Manual QA**

```bash
osascript -e 'quit app "Highlight Scout"' 2>/dev/null; sleep 1
rm -rf "/Applications/Highlight Scout.app"
cp -R "src-tauri/target/release/bundle/macos/Highlight Scout.app" "/Applications/"
open "/Applications/Highlight Scout.app"
```

Then in the app: Settings → Import → "Readwise saved tweets" (re-import). Verify:
- A thread (search "It's hard to overstate how disappointing academia") shows divider lines (`---`) between the tweets, and the trailing bare self-link is gone.
- A quoted tweet (search "major-version-bump") shows the main text, then an indented blockquote with `**Claude** @claudeai`, the quoted text, the image inside the quote, and a muted "Posted …" line.
- Copy as rich text / markdown on those rows includes the `---` / blockquote.

- [ ] **Step 6: Commit**

```bash
git add src/version.ts src-tauri/tauri.conf.json
git commit -m "chore: bump to 0.5.3 (structure-aware tweet import)"
```

---

## Self-Review

**1. Spec coverage:**
- `tl` dependency (Task 1). ✓
- `parse_tweet_html` structure parser — thread `<hr>` → `---`, `rw-embedded-tweet` → blockquote, inline `/media/` images, emphasis, links, entities, graceful fallback (Task 2). ✓
- `body_markdown` passthrough so Reader path bypasses re-assembly; birdclaw untouched (Task 1). ✓
- Reader `import()` wired to parser; `source_data.images` still populated; `image_url` prepend dropped (Task 3). ✓
- Renderer `---` hr (Task 4); copy `---` hr (Task 5). ✓
- Migration = re-run import (Task 6 QA; no code). ✓
- Version bump (Task 6). ✓

**2. Placeholder scan:** No TBD/TODO. The parser task intentionally defers exact `tl` call syntax to implementation (API-uncertain) but pins the contract via three concrete fixture tests and a full algorithm — this is a deliberate TDD task, not a placeholder.

**3. Type consistency:** `body_markdown: Option<String>` consistent across Task 1 (definition) and Task 3 (use). `Block` union gains `{ t: "hr" }` in Task 4 and is handled in Task 4 (render) + Task 5 (copy) — both reference the same shape. `parse_tweet_html(&str) -> String` and `media_images(&str) -> Vec<String>` names consistent between Tasks 2 and 3.
