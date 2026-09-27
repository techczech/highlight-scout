from lib import *
import re
WS = {'w': 'w', 't': 'tw', 'h': 'hl'}

SENT_M = REPAVED_SENT.replace("new paths", "new <mark>paths</mark>")
CITE_LINK = f'{CITE_SPEC} · <a>open piece</a>'


def ed_before():
    return f'<h1>Metaphor talk</h1><p>{DRAFT_P1}</p><p>As I wrote in 2016:<span class="wcaret"></span></p>'


def ed_after():
    return (f'<h1>Metaphor talk</h1><p>{DRAFT_P1}</p><p>As I wrote in 2016:</p>'
            f'<blockquote class="flash"><p>{REPAVED_SENT}</p><cite>{CITE_LINK}</cite></blockquote>'
            f'<p><span class="wcaret"></span></p>')


def ed_link():
    return (f'<h1>Metaphor talk</h1><p>{DRAFT_P1}</p>'
            f'<p>As I wrote in 2016 in <a class="flash">Repaved paths and generative metaphors</a><span class="wcaret"></span></p>')


def wr(it, on=False, ins="", two=False):
    s = it["s"]
    m = [f'<span class="wsrc {WS[s]}">{SRC_LABEL[s]}</span>']
    if it.get("sub"):
        m.append(f'<span>{it["sub"]} ·</span>')
    if it.get("by"):
        m.append(f'<span style="color:var(--wf-ink-2);font-weight:500">{it["by"]} ·</span>')
    m.append(f'<span style="overflow:hidden;text-overflow:ellipsis">{it["meta"]}</span>')
    t = f'<div class="ti">{it["title"]}</div>' if it.get("title") else ""
    return (f'<div class="wr{" on" if on else ""}"><div class="m">{"".join(m)}</div>{t}'
            f'<div class="s{" two" if two else ""}">{it["snip"]}</div>{ins}</div>')


def ptabs():
    return ('<div class="ptabs">'
            f'<span class="ptab cut">Review</span><span class="ptab">{icon("doc", "sm")} Agent notes</span>'
            f'<span class="ptab">{icon("clock", "sm")} Versions</span>'
            f'<span class="ptab on">{icon("search", "sm")} Search</span></div>')


def scope():
    return ('<div class="wf-scope"><span class="on">All</span><span>Writing</span><span>Tweets</span><span>Highlights</span>'
            '<span style="background:transparent;color:var(--wf-ink-3)">Best matches ⌄</span></div>')


def pane_a(inserted=False):
    if inserted:
        ins = ('<div class="ins"><span class="wbtn" style="color:var(--wf-review);border-color:var(--wf-review)">'
               f'{icon("check", "sm")} Inserted into the draft</span></div>')
        inp = (f'<div class="wf-in" style="border-color:var(--wf-line);box-shadow:none">{icon("search", "sm")}paths metaphor</div>')
    else:
        ins = (f'<div class="ins"><span class="wbtn pri"><kbd>⏎</kbd> Quote + citation</span>'
               f'<span class="wbtn"><kbd>⌘⏎</kbd> Link only</span><span class="wbtn">{icon("copy", "sm")}</span></div>')
        inp = f'<div class="wf-in">{icon("search", "sm")}paths metaphor<span class="c"></span></div>'
    res = (wr(I["repaved"], on=True, ins=ins) + wr(I["sword"]) + wr(I["tgen"], two=True) + wr(I["hack"], two=True)
           + wr(I["backrow"], two=True) + wr(I["fork"]))
    return (f'<div class="rpane">{ptabs()}{inp}{scope()}<div class="wf-res">{res}</div>'
            '<div class="pfoot"><span><kbd>⏎</kbd> quote + citation</span><span><kbd>⌘⏎</kbd> link only</span>'
            '<span><kbd>↑↓</kbd> move</span><span><kbd>esc</kbd> back to draft</span></div></div>')


def a1():
    return wf_win("wf4a-1", "WF-4A", ed_before(), pane_a())


def a2():
    toast = f'<div class="wf-status">{icon("check", "sm")} Quote + citation inserted <span style="opacity:.6">·</span> ⌘Z undoes</div>'
    return wf_win("wf4a-2", "WF-4A", ed_after(), pane_a(True), extra=toast)


def a3():
    return wf_win("wf4a-3", "WF-4A", ed_link(), pane_a(True), cls="crop")


# ---------- B ----------

def wrow(it, on=False):
    s = it["s"]
    label = it.get("title") or re.sub('<[^>]+>', '', it["snip"]) if s == "t" else it.get("title") or (it.get("by", "") + " · " + it["meta"] if it.get("by") else it["snip"])
    d = it["meta"].split(" · ")[0] if s != "h" else it.get("sub", "")
    return (f'<div class="wrow{" on" if on else ""}"><span class="wsrc {WS[s]}">{SRC_LABEL[s]}</span>'
            f'<span class="tt2">{label}</span><span class="d">{d}</span></div>')


def pane_b():
    rows = (wrow(I["repaved"], True) + wrow(I["hack"]) + wrow(I["backrow"]) + wrow(I["tgen"]) + wrow(I["sword"])
            + wrow(I["fork"]))
    ctx = (f'<div class="ctx">…They simply looked at paths trodden in the grass after a few semesters and then paved them. <span class="pick">{SENT_M}</span></div>')
    will = f'<div class="will"><em>{REPAVED_SENT}</em><div style="margin-top:4px;font-size:12.5px;color:var(--wf-ink-3)">{CITE_SPEC} · <span class="wfa">open piece</span></div></div>'
    return (f'<div class="rpane wide"><div class="phead">{icon("search")} Search the archive<span class="gap"></span><kbd>⌃⌘F</kbd>{icon("x", "sm")}</div>'
            f'<div class="wf-in">{icon("search", "sm")}paths metaphor<span class="c"></span></div>{scope()}'
            f'<div style="padding:0 8px">{rows}</div>'
            f'<div class="plab" style="display:flex">Passage<span style="margin-left:auto;text-transform:none;letter-spacing:0;font-weight:500"><kbd>⇧↑</kbd> <kbd>⇧↓</kbd> take one more sentence</span></div>{ctx}'
            f'<div class="plab">Will insert</div>{will}'
            f'<div style="margin-top:auto;display:flex;gap:8px;padding:12px 14px;border-top:1px solid var(--wf-line-soft);background:var(--wf-bg-sunk)">'
            f'<span class="wbtn pri big" style="flex:1.3">Insert quote + citation <kbd>⏎</kbd></span>'
            f'<span class="wbtn big" style="flex:1">Insert link only <kbd>⌘⏎</kbd></span></div></div>')


def b1():
    return wf_win("wf4b-1", "WF-4B", ed_before(), pane_b(), side=False)


def b2():
    toast = (f'<div class="wf-status">{icon("check", "sm")} Inserted from “Repaved paths and generative metaphors”'
             f'<span style="opacity:.6">·</span> ⌃⌘F searches again <span style="opacity:.6">·</span> ⌘Z undoes</div>')
    return wf_win("wf4b-2", "WF-4B", ed_after(), "", side=False, pane_on=False, extra=toast)


def b3():
    return wf_win("wf4b-3", "WF-4B", ed_link(), "", side=False, pane_on=False, cls="crop")


A = direction("WF-4A", "Search as a sixth right-pane tab",
              "Search joins Agent, Scratchpad, Review, Agent notes and Versions in the existing 340 px right pane. ⌃⌘F opens the pane on the Search tab and puts the cursor in its box. "
              "Results are cards in WriteFlex’s own type: source badge first, title, the matching sentence in the prose face. "
              "The pane stays open after an insert, so he can take a second quote without searching again.",
              state(1, "<b>⌃⌘F, then <code>paths metaphor</code>.</b> The tab row scrolls (as built today) so Search is in full view; the selected card shows both insert actions.", a1(),
                    "WriteFlex at 1440 × 900 (windows.ts), sidebar 242 px, right pane 340 px, paper 660 px, all from app.css.")
              + state(2, "<b>⏎ inserts the quote with its citation line and link.</b> The block flashes as an applied edit does today; the cursor is on the next empty line in the draft.", a2())
              + state(3, "<b>⌘⏎ inserts only the link</b>, with the piece title as its text, at the cursor.", a3()))

B = direction("WF-4B", "A wider search dock that previews the insert",
              "Search is its own 460 px dock, not a tab, and the sidebar folds away while it is open. The top half is a one-line-per-result list for scanning; "
              "the bottom half shows the passage in context and exactly what will be inserted. ⇧↑ and ⇧↓ take one more sentence before or after. "
              "After an insert the dock closes and he is back in the draft.",
              state(1, "<b>⌃⌘F, then <code>paths metaphor</code>.</b> The list scans fast; the preview below shows the sentence in its paragraph and the block that ⏎ will insert.", b1())
              + state(2, "<b>⏎ inserts and closes the dock.</b> The cursor is on the next line; the status line says where the quote came from.", b2())
              + state(3, "<b>⌘⏎ inserts only the link.</b>", b3()))

page("wf-4-search-panel.html", "Surface 4", "The WriteFlex search panel",
     "Journey J1-A: ⌃⌘F opens a search panel on the right of the editor. ⏎ inserts the quote as a blockquote with its citation line and a link back to the piece; ⌘⏎ inserts only the link.",
     "<span>Drawn at <b>1440 × 900</b> (WriteFlex default window)</span><span>Tokens, toolbar, tabs, pane and prose styles ported from <b>tokens.css</b> and <b>app.css</b>; prose in Source Serif 4</span>"
     "<span>The draft’s first paragraph is from “Hacking a metaphor in five steps”; “As I wrote in 2016:” is placeholder draft text</span>",
     A + B,
     ["A sixth tab in a pane that already needs to scroll for five (WF-4A), or a dock of its own that replaces the pane while it is open (WF-4B)?",
      "After an insert, should the panel stay open for the next quote (WF-4A) or close and return him to writing (WF-4B)?",
      "⌃⌘F sits in WriteFlex’s ⌃⌘ structure layer; the estate keymap reserves ⌘⇧F for “search across everything”. Should ⌘⇧F open this panel too?",
      "Should the inserted citation line keep the truncated title from the journey (“…Expressing human purpose…”) or carry the full title?",
      "What should the link open: the public post, or the piece in the WriteFlex archive?",
      "The blockquote is drawn rendered. In the Markdown source it is a <code>&gt;</code> block with the citation as its last line; confirm that is the format he wants saved."],
     fonts=True)
