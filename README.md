# Highlight Scout

Fast local search across your reading highlights. A small desktop app that pulls
highlights from multiple sources into one searchable archive, with a global
hotkey for instant access.

## What it does

- **Multi-source import.** Readwise (books, articles, tweets, podcasts) and
  Zotero (PDF annotations, read directly from the local database). Readwise is
  one source among several, not the hub.
- **Full-text article bodies.** Pulls the complete text of Reader articles so
  you can refer back to the surrounding context, not just the highlight.
- **First-class Zotero colours and types.** Filter by annotation colour (your
  own meaning — e.g. red = important, green = methods) and by type (highlight,
  underline, comment, image).
- **Instant keyword search.** SQLite full-text search with stemming, sub-second
  on tens of thousands of highlights.
- **Global hotkey.** `⌘⇧H` toggles the window from anywhere. Type, navigate with
  the arrow keys, open with return.

Semantic search (find by meaning, not just keywords) is the next addition — the
mode toggle is already in the interface.

## Setup

Requires [Bun](https://bun.sh) and the Rust toolchain.

```bash
bun install
bun run tauri dev      # run in development
bun run tauri build    # build a release app
```

On first launch the app writes a config file to
`~/.config/highlight-scout/config.toml`:

```toml
readwise_api_key = ""                                   # from readwise.io/access_token
archive_path = "~/gitrepos/.../highlights-archive-v2"   # where Markdown is written
shortcut = "CmdOrCtrl+Shift+H"                           # global hotkey
zotero_db_path = "~/Zotero/zotero.sqlite"               # local Zotero database
```

Add your Readwise key, then click **Import Readwise** or **Import Zotero**.
Zotero import needs no API key and works whether or not Zotero is running.

## How it stores things

- **Archive** — Markdown, one file per work, committed to git. Human-readable
  and portable. Full article bodies live alongside in `readings/fulltext/`.
- **Index** — a generated SQLite file kept locally, never committed; rebuilt
  from the Archive at any time.

If you stop using a reader or switch tools, the Archive is yours and stays
readable.

## Keyboard

| Key | Action |
| --- | --- |
| `⌘⇧H` | Show / hide the window |
| `↑` `↓` | Move through results |
| `↵` | Open the highlight detail |
| `⌘↵` | Open the source in your browser |
| `esc` | Clear the query, then hide the window |
