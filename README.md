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
- **Instant keyword search** with a full query grammar (see below), sub-second
  on tens of thousands of highlights.
- **Reading pane** with matched terms highlighted, full metadata, and inline
  rendering of Zotero image annotations.
- **Sort, group, filter.** Sort by matches/recent/oldest; group by work, author,
  year, tag, or none; scope by favourites, Zotero, time, or type; filter by
  Zotero colour.
- **Work view.** Open any work as a scrollable document of all its highlights.
- **Global hotkey.** `⌘⇧H` toggles the window from anywhere.

Semantic search (find by meaning, not just keywords) is the next addition — the
mode toggle is already in the interface.

## Search syntax

Matching adapts to how many words you type: **one word** matches as-is, **two
words** require both, **three or more** match any (ranked by how many hit).

- `cat OR dog` / `cat | dog` — force either · `book AND chapter` — force all
- `-novice` exclude · `"deep practice"` phrase · `comput*` prefix · `/\bAI\b/` regex (`/x/c` = case-sensitive)
- Fields: `au:`/`author:`, `ti:`/`title:`, `ty:`/`type:`, `tag:`, `co:`/`color:`,
  `zo:` (Zotero only), `y:2023` / `y:2022-2024` / `y:2024-` / `y:-2022`,
  `after:` / `before:`, and type shortcuts `book:` `art:` `tw:` `pdf:` `pod:`

## Keyboard

| Key | Action |
| --- | --- |
| `⌘⇧H` | Show / hide the window |
| `↑` `↓` | Move through results |
| `↵` | Open the source in your browser |
| `⌘C` | Copy the highlight |
| `⌘⇧C` | Copy as a Markdown quote |
| `⌘⇧L` | Open the work's highlights |
| `⌘⇧O` | Open the work Markdown file |
| `⌘⇧P` | Toggle the reading pane |
| `⌘⇧T` | Filter by tag |
| `⌘,` | Settings |
| `esc` | Clear the query, then hide |

## Settings

Open with `⌘,` (or the ⚙ button) to set your Readwise API key, archive path,
Zotero database path, global shortcut, and result limit. Changes apply
immediately; the shortcut applies after a restart.

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
