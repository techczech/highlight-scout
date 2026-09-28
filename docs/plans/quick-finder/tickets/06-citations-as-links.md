# 06: Citations carry their links as links, never raw URLs

**What to build:** Dominik's preview verdict (DTC quick-finder-preview-2, 2026-09-28): "the references should be under links". Pasted, the citation showed "archive: writeflex://open?path=%2FUsers…" and "public: https://medium.com/…" as raw text.
- In every copy format the words **archive** and **public** are the link text, and the URLs sit behind them.
- **Markdown** (WriteFlex): `— Dominik Lukeš, *Full Title*, 23 June 2016 · [archive](writeflex://…) · [public](https://…)`.
- **Rich HTML** (Word, Slack, Mail): `<a href>` anchors with those words.
- **Plain text** (a target that takes no formatting): only the public URL in angle brackets, or nothing if there is none. Never the local writeflex:// path.
- Fix the "Auto" choice so that pasting into WriteFlex actually gets Markdown. His paste looked like the plain fallback, so first find out why Auto picked plain (frontmost-app detection, or the clipboard carrying only text/plain).
- Put BOTH text/html and text/plain on the clipboard, so each target app picks its best format.

**Feature / journey:** `citation-copy` (J1).
**Design:** `~/gitrepos/06_apps-utilities/01_desktop-apps/highlight-scout/docs/design/2026-09-27-archive-search-and-corpus-tools/round-1/picks.md` ("the citation link carries BOTH links"); the verdict screenshot is at `_DTC/highlight-scout/2026-09-28-quick-finder-preview-2.shots/copy-a-quote-with-its-citation-1.png`.
**Blocked by:** None: dispatchable now (branch feat/quick-finder-01).
**Seams under test:** the citation formatter (engine cite → markdown, html and plain, golden tests), and the clipboard writer (both flavours present).
**Status:** ready

- [ ] Pasting into WriteFlex gives Markdown links; into Word or Mail, clickable "archive" and "public" words; into TextEdit plain text, no local path.
- [ ] A test fails if any format prints a raw writeflex:// URL as visible text.
