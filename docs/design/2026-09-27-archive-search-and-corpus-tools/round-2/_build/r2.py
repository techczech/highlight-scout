"""Round 2: one merged search (HS-M1A, HS-M1B). Run: python3 r2.py"""
import os
import lib
from lib import I, REPAVED_PREV, REPAVED_SENT, icon, ph, hs_row, hs_win, state, direction, page

_css = lib.css
R2 = open(os.path.join(lib.HERE, "r2.css")).read()
lib.css = lambda with_fonts=False: _css(with_fonts) + "\n" + R2

SENT_M = REPAVED_SENT.replace("new paths", "new <mark>paths</mark>")
COUNTS = "Writing 1,506 pieces · Tweets 14,892 · Highlights 14,724 works"
Q2 = "computer as a metaphor for the mind"

# The 14 colours the colour row shows today (facets.colors.slice(0,14)), in the
# order of their counts in index.sqlite; named ones resolved through COLOR_MAP.
COLOURS = [("yellow", "#eab308"), ("red", "#ef4444"), ("green", "#22c55e"), ("purple", "#a855f7"),
           ("blue", "#3b82f6"), ("gray", "#9ca3af"), ("magenta", "#d946ef"), ("#33ff33", "#33ff33"),
           ("orange", "#f97316"), ("#ffff00", "#ffff00"), ("#32ff32", "#32ff32"), ("#ff32ff", "#ff32ff"),
           ("#ff3232", "#ff3232"), ("#ff33ff", "#ff33ff")]


def cdots(on="red"):
    return '<span class="m-cdots">' + "".join(
        f'<span class="m-cd{" on" if n == on else ""}" title="{n}" style="background:{c}"></span>' for n, c in COLOURS) + "</span>"


# ---------- real highlight rows for frame 2 (index.sqlite, Zotero, colour red) ----------
NEWELL_1 = ("The developments now taking place in psychology involve much more, however, than just a borrowing of new terms "
            "and new metaphors from other sciences. They involve the use of the digital computer as a tool both for "
            "constructing theories and for testing them.")
NEWELL_2 = ("We see that a computer is not merely a number-manipulating device; it is a symbol-manipulating device, and the "
            "symbols it manipulates may represent numbers, letters, words, or even nonnumerical, nonverbal patterns.")
SEARLE = ("strong AI has little to tell us about thinking, since it is not about machines but about programs, and no program "
          "by itself is sufficient for thinking.")
MILLS = ("metaphor of hallucination reinforces the misconception that AI is conscious; it implies that AI experiences reality "
         "and sometimes becomes delirious.")
FTT = ("During the last year or two most people must have heard of the remarkable devices often called “Electronic "
       "Brains”; every schoolboy knows that there are in existence some very complicated machines which are capable of "
       "astounding feats of arithmetic.")
WORKS = [
    ("Computer Simulation of Human Thinking", "Newell", [(NEWELL_1, "2011"), (NEWELL_2, "2012")]),
    ("Minds, brains, and programs", "Searle", [(SEARLE, "417")]),
    ("Are We Tripping? The Mirage of AI Hallucinations", "Mills", [(MILLS, "1")]),
    ("Faster Than Thought: A Symposium on Digital Computing Machines", None, [(FTT, "13")]),
]


def hl_row(text, loc, on=False, acts=""):
    return (f'<div class="row{" on" if on else ""}"><div class="body"><div class="meta1">'
            f'<span class="m-cd sm" style="background:#ef4444"></span><span>Zotero</span><span>·</span><span>location {loc}</span></div>'
            f'<div class="snip">{text}</div>{acts}</div></div>')


def acts_row():
    return (f'<div class="rowacts"><span class="act">{icon("quote", "sm")}Quote <kbd>⌘C</kbd></span>'
            f'<span class="act pri">{icon("cite", "sm")}Quote + citation <kbd>⌘⇧C</kbd></span>'
            f'<span class="act">{icon("link", "sm")}Link</span><span class="act">+ Set</span></div>')


def work_list(head):
    out = [f'<div class="hs-list" style="width:430px">{head}']
    first = True
    for title, by, rows in WORKS:
        byh = f'<span class="m-by">{by}</span>' if by else '<span class="m-by">no author</span>'
        out.append(f'<div class="m-whead"><b>{title}</b>{byh}<span>·</span><span>{ph()}</span></div>')
        for text, loc in rows:
            out.append(hl_row(text, loc, on=first, acts=acts_row() if first else ""))
            first = False
    out.append("</div>")
    return "".join(out)


def pane_hl():
    md = (f"&gt; {NEWELL_1}<br>&gt;<br>&gt; — Newell, *Computer Simulation of Human Thinking* · "
          "[jstor.org/stable/1708447](https://www.jstor.org/stable/1708447)")
    return (f'<div class="hs-pane" style="padding:18px 22px 14px"><div class="p-title" style="font-size:16px">Computer Simulation of Human Thinking</div>'
            f'<div class="p-sub"><span>Newell · article · Zotero</span><span class="link">Open PDF in Zotero {icon("ext", "sm")}</span>'
            f'<span class="link">jstor.org/stable/1708447</span></div>'
            f'<div class="p-quote" style="font-size:14.5px;border-left-color:#ef4444"><p>{NEWELL_1}</p></div>'
            f'<div class="m-pmeta"><span class="m-cd sm" style="background:#ef4444"></span><span>red</span><span>·</span>'
            f'<span>location 2011</span><span>·</span><span class="m-b">Copy ▾</span></div>'
            f'<div style="margin-top:auto"><div style="display:flex;align-items:center;gap:8px;margin-bottom:6px"><span class="lab" style="margin:0">⌘⇧C copies</span>'
            f'<span class="sel" style="margin-left:auto;font-size:12px">Auto: Markdown in WriteFlex, rich text elsewhere</span></div>'
            f'<div class="cite-box" style="font:11.5px/1.5 ui-monospace,Menlo,monospace;color:var(--hs-t3)">{md}</div>'
            f'<div class="actbar" style="margin-top:10px"><span class="act v">{icon("spark")}Find related <kbd>⌘⇧F</kbd></span>'
            f'<span class="act">Show work highlights → <kbd>⌘⇧L</kbd></span><span class="act">⧉ New window</span></div></div></div>')


def pane_w():
    md = ("&gt; But being open to repaving once new paths are trodden alongside those you outlined with your technology "
          "(be it critical pedagogy or iPads) is perhaps the most important thing we can do in our ed tech efforts.<br>&gt;<br>"
          "&gt; — Dominik Lukeš, *Repaved paths and generative metaphors: Expressing human purposes with technology*, 23 June 2016 · "
          "[open piece](https://medium.com/metaphor-hacker/repaved-paths-…)")
    return (f'<div class="hs-pane" style="padding:18px 22px 14px"><div class="p-title" style="font-size:16px">Repaved paths and generative metaphors: Expressing human purposes with technology</div>'
            f'<div class="p-sub"><span>Dominik Lukeš · 23 June 2016 · essay</span><span class="link">Open piece {icon("ext", "sm")}</span></div>'
            f'<div class="p-quote" style="font-size:14.5px"><p class="dim">{REPAVED_PREV}</p><p><span class="sent">{SENT_M}</span></p></div>'
            f'<div style="margin-top:auto"><div style="display:flex;align-items:center;gap:8px;margin-bottom:6px"><span class="lab" style="margin:0">⌘⇧C copies</span>'
            f'<span class="sel" style="margin-left:auto;font-size:12px">Auto: Markdown in WriteFlex, rich text elsewhere</span></div>'
            f'<div class="cite-box" style="font:11.5px/1.5 ui-monospace,Menlo,monospace;color:var(--hs-t3)">{md}</div>'
            f'<div class="actbar" style="margin-top:10px"><span class="act">{icon("spark")}Find related</span><span class="act">Show work highlights →</span></div></div></div>')


def corpus_list(head_right='<span class="sel" style="margin-left:auto">Best matches</span>', group_lbl="grouped by corpus"):
    def sec(name, dot):
        return f'<div class="sechead"><span class="dot {dot}" style="width:7px;height:7px;border-radius:50%"></span>{name}{ph()}<span class="more">show all</span></div>'
    return ('<div class="hs-list" style="width:450px">'
            f'<div class="listhead"><b>paths metaphor</b><span>·</span><span>{group_lbl}</span>{head_right}</div>'
            + sec("Writing", "w") + hs_row(I["repaved"], on=True, acts=acts_row()) + hs_row(I["hack"]) + hs_row(I["backrow"])
            + sec("Tweets", "t") + hs_row(I["tgen"])
            + sec("Highlights", "h") + hs_row(I["sword"]) + hs_row(I["fork"]) + '</div>')


def foot(left, scope="all"):
    if scope == "all":
        right = ('<span>↑↓ nav</span><span><kbd>⌥↓</kbd> next corpus</span><span><kbd>⌘C</kbd> quote</span>'
                 '<span><kbd>⌘⇧C</kbd> + citation</span><span><kbd>esc</kbd> hide</span>')
    else:
        right = ('<span>↑↓ nav</span><span><kbd>⌥↓</kbd> next work</span><span><kbd>⏎</kbd> source</span><span><kbd>⌘C</kbd> quote</span>'
                 '<span><kbd>⌘⇧C</kbd> + citation</span><span><kbd>⌘⇧L</kbd> work</span><span><kbd>esc</kbd> hide</span>')
    ref = f'<span class="m-ico" title="Refresh: re-run the search and reload counts">{icon("refresh", "sm")}</span>'
    return f'<div class="hs-foot">{ref}<span style="display:flex;align-items:center;gap:6px;min-width:0">{left}</span><span class="r">{right}</span></div>'


def tail_icons():
    return (f'<span class="m-ico on" title="Reading pane (⌘\\)">{icon("panel", "sm")}</span>'
            f'<span class="m-ico" title="Settings (⌘,)">{icon("gear", "sm")}</span>')


# =====================================================================
# HS-M1A: the rail is the scope
# =====================================================================

def rail_a(on="all", glow=False):
    def ri(key, inner, cls=""):
        c = cls + (" on" if key == on else "") + (" m-glow" if (glow and key == "h") else "")
        return f'<div class="ri {c}">{inner}</div>'
    d = lambda k: f'<span class="dot {k}" style="width:8px;height:8px;border-radius:50%"></span>'
    return ('<div class="rail"><div class="rl">Search in</div>'
            + ri("all", f'{icon("search", "sm")}All corpora')
            + ri("w", f'{d("w")}Writing<span class="n">1,506</span>')
            + ri("t", f'{d("t")}Tweets<span class="n">14,892</span>')
            + ri("h", f'{d("h")}Highlights<span class="n">14,724</span>')
            + ri("x", 'X<span class="n">9,683</span>', "sub") + ri("rw", 'Readwise<span class="n">4,698</span>', "sub")
            + ri("zo", 'Zotero<span class="n">343</span>', "sub")
            + f'<div class="rl">Sets</div><div class="ri">{icon("set", "sm")}Metaphor talk<span class="n">7</span></div>'
            f'<div class="ri">{icon("saved", "sm")}metaphor -tweets…<span class="n">{ph()}</span></div>'
            f'<div class="rl">Recent</div><div class="ri" style="color:var(--hs-t3)">{icon("clock", "sm")}{Q2}</div>'
            f'<div class="ri" style="color:var(--hs-t3)">{icon("clock", "sm")}paths metaphor</div>'
            f'<div class="ri" style="color:var(--hs-t3)">{icon("clock", "sm")}scaffolding</div>'
            '<div style="margin-top:auto;padding:10px 16px;font-size:11.5px;color:var(--hs-t4)">Counts are pieces, tweets and works.</div></div>')


def search_a(q, extra=""):
    return (f'<div class="hs-search">{icon("search", "lg")}<div class="q">{q}<span class="caret"></span></div>{extra}'
            f'<span class="small">actions on selection</span><kbd>⌘K</kbd>{tail_icons()}</div>')


def bar_a(semantic=True, filters_open=False, colour="red"):
    seg = ('<span class="seg"><span>Keyword</span><span class="on m-sem">Semantic</span></span>' if semantic else
           '<span class="seg"><span class="on">Keyword</span><span>Semantic</span></span>')
    match = "" if semantic else '<span class="m-b">Match: whole word</span>'
    fcls = "m-b" + (" open" if filters_open else "")
    return (f'<div class="m-bar">{seg}{match}<span class="m-sep"></span>'
            f'<span class="m-lbl">Colour</span>{cdots(colour)}<span class="m-clear">clear</span><span class="m-sep"></span>'
            f'<span class="m-b">Tags <span class="k">⌘⇧T</span></span><span class="{fcls}">⚲ Filters <span class="k">⌘⇧I</span></span>'
            '<span class="m-r"><span>Rows</span><span class="sel">Compact</span></span></div>')


def win_a_all(fid, overlay=""):
    body = (search_a("paths metaphor") +
            f'<div class="hs-main">{rail_a("all", glow=bool(overlay))}{corpus_list()}{pane_w()}</div>{overlay}'
            + foot(COUNTS))
    return hs_win(fid, "HS-M1A", body)


def win_a_hl(fid, pop=""):
    head = ('<div class="listhead"><span>Group</span><span class="sel">Work</span><span>then</span><span class="sel">—</span>'
            '<span style="margin-left:auto">Sort</span><span class="sel">Best matches</span></div>')
    body = (search_a(Q2, '<span class="m-run">Semantic · ⏎ runs</span>') +
            f'<div class="hs-main">{rail_a("h")}<div class="m-col">{bar_a(filters_open=bool(pop))}'
            f'<div class="m-body">{work_list(head)}{pane_hl()}{pop}</div></div></div>'
            + foot(f"Highlights · 14,724 works · semantic results {ph()}", scope="h"))
    return hs_win(fid, "HS-M1A", body)


def pop_a():
    def it(label, ind, act=False, sec=None, cls=""):
        s = f'<div class="m-ps">{sec}</div>' if sec else ""
        return f'{s}<div class="m-pi{" act" if act else ""} {cls}"><span class="ind">{ind}</span>{label}</div>'
    body = (it("★ Favorites", "☐", act=True, sec="Quick") + it("🔖 Zotero", "☐") + it("🖼 Has image", "☐")
            + it("Articles", "☐", sec="Type") + it("Books", "☐") + it("Tweets", "☐") + it("PDFs", "☐") + it("Podcasts", "☐")
            + it("Any", "◉", sec="Time") + it("30 days", "○") + it("6 months", "○") + it("Year", "○")
            + '<div class="m-hr"></div><div class="m-clr">Clear all</div>')
    # anchored under the Filters button in the bar (x ≈ 590 inside the column)
    return f'<div class="m-pop" style="left:606px;top:2px;width:224px">{body}</div>'


def coach_a():
    return ('<div class="m-coach" style="left:222px;top:164px">'
            '<b>Classic search is now “Highlights”</b>'
            '<p>Pick Highlights, or X, Readwise or Zotero under it, and the classic tools appear above the results: '
            'Keyword / Semantic, colours, Tags, Filters, Group, Sort and Rows.</p>'
            '<p>The old keys still work from anywhere and switch to Highlights: '
            '<kbd>⌘⇧I</kbd> filters, <kbd>⌘⇧T</kbd> tags, <kbd>⌘⇧G</kbd> group, <kbd>⌘⇧D</kbd> rows, <kbd>⌘⇧X</kbd> clear colour.</p>'
            '<div class="m-cb"><span>Got it</span><span class="pri">Show me</span></div></div>')


A = direction(
    "HS-M1A", "The rail is the scope",
    "The HS-1B window, with the “Classic search” button gone. The rail decides what you are searching, and what you are searching decides "
    "which tools you see. Under All corpora the list stays grouped Writing / Tweets / Highlights with only Sort in its header. "
    "Pick Highlights, or X, Readwise or Zotero, and a slim bar appears above the results carrying today’s Classic toolbar in one line: "
    "Keyword / Semantic, match mode, the colour row, Tags, Filters and Rows. Group (with its “then” sub-group) and Sort move into the list header, above the rows they arrange; at 990 px the Classic toolbar and the colour row do not fit on one line.",
    state(1, "<b>All corpora.</b> Exactly HS-1B: rail, results grouped by corpus, reading pane. No highlight tools, because two of the three corpora have no colours, tags or images.",
          win_a_all("m1a-1"),
          "The refresh button moves to the start of the footer; the reading-pane toggle and Settings sit at the end of the search row. The query grammar (<code>in:writing after:2020</code>, <code>-exclude</code>, <code>\"phrase\"</code>) works in every scope.")
    + state(2, f"<b>Highlights, grouped by work, red only, semantic on.</b> He picks Highlights in the rail; the tool bar appears and the list header gains Group and Sort. Semantic is violet, as Find related is today; ⏎ runs it. The colour row is today’s 14 colours; red is ringed.",
            win_a_hl("m1a-2"),
            "Rows drop the work title because the group head carries it. The reading pane keeps everything Classic’s pane had: Open PDF in Zotero, the URL, Copy ▾, colour, location, Find related, Show work highlights and New window.")
    + state(3, "<b>Filters popover open (⌘⇧I).</b> Today’s popover, unchanged: Quick, Type, Time, Clear all. Keyboard behaviour stays too: ↑↓ move, Space toggles, Esc closes.",
            win_a_hl("m1a-3", pop=pop_a()),
            "Colour and Tags are not in the popover because the bar already shows them; this is the Classic split, kept.")
    + state(4, "<b>First launch after the update: a one-time hint.</b> The Highlights row glows and a card points at it. “Show me” selects Highlights; “Got it” dismisses. It never shows again.",
            win_a_all("m1a-4", overlay=coach_a()),
            "Every Classic shortcut pressed under All corpora switches the rail to Highlights and then does its job, so muscle memory also finds the tools without the card."))


# =====================================================================
# HS-M1B: one list, one Filters control
# =====================================================================

def rail_b(ticked=("w", "t", "h")):
    d = lambda k: f'<span class="dot {k}" style="width:8px;height:8px;border-radius:50%"></span>'
    def ri(key, label, n, sub=False):
        on = key in ticked or (key in ("x", "rw", "zo") and "h" in ticked)
        cls = ("sub " if sub else "") + ("" if on else "m-off")
        ck = f'<span class="m-ck{" on" if on else ""}"></span>'
        dot = "" if sub else d(key)
        return f'<div class="ri {cls}">{ck}{dot}{label}<span class="n">{n}</span></div>'
    return ('<div class="rail"><div class="rl">Search in</div>'
            + ri("w", "Writing", "1,506") + ri("t", "Tweets", "14,892") + ri("h", "Highlights", "14,724")
            + ri("x", "X", "9,683", True) + ri("rw", "Readwise", "4,698", True) + ri("zo", "Zotero", "343", True)
            + f'<div class="rl">Sets</div><div class="ri">{icon("set", "sm")}Metaphor talk<span class="n">7</span></div>'
            f'<div class="ri">{icon("saved", "sm")}metaphor -tweets…<span class="n">{ph()}</span></div>'
            f'<div class="rl">Recent</div><div class="ri" style="color:var(--hs-t3)">{icon("clock", "sm")}{Q2}</div>'
            f'<div class="ri" style="color:var(--hs-t3)">{icon("clock", "sm")}paths metaphor</div>'
            f'<div class="ri" style="color:var(--hs-t3)">{icon("clock", "sm")}scaffolding</div>'
            '<div style="margin-top:auto;padding:10px 16px;font-size:11.5px;color:var(--hs-t4)">Tick one or more. Counts are pieces, tweets and works.</div></div>')


def search_b(q, semantic=False, fcount=0, group="Corpus", open_=False, placeholder=False, run=""):
    qs = ('<span class="m-qseg"><span>Keyword</span><span class="on m-sem">Semantic</span></span>' if semantic else
          '<span class="m-qseg"><span class="on">Keyword</span><span>Semantic</span></span>')
    qhtml = (f'<span class="caret"></span><span class="ghost">Search writing, tweets and highlights…</span>' if placeholder
             else f'{q}<span class="caret"></span>')
    lbl = f'⚲ Filters{f" ({fcount})" if fcount else ""} <span style="color:var(--hs-t4)">·</span> Group: {group}'
    cls = "hs-btn sm" + (" m-b on" if fcount else "") + (" m-b open" if open_ else "")
    return (f'<div class="hs-search">{icon("search", "lg")}<div class="q">{qhtml}</div>{run}{qs}'
            f'<span class="{cls}" style="border-radius:7px">{lbl}</span>{tail_icons()}</div>')


def chips_b():
    return ('<div class="m-chips"><span>Showing</span>'
            '<span class="m-fc"><span class="m-cd sm" style="background:#ef4444"></span>Colour: red<span class="x">×</span></span>'
            '<span class="m-fc">Group: Work<span class="x">×</span></span>'
            '<span class="m-fc v">Semantic<span class="x">×</span></span>'
            '<span class="m-clear" style="margin-left:4px">clear all</span>'
            '<span style="margin-left:auto">Highlights only · ⌘⇧I edits</span></div>')


def win_b_all(fid):
    body = (search_b("paths metaphor") +
            f'<div class="hs-main">{rail_b()}{corpus_list(group_lbl="Group: Corpus")}{pane_w()}</div>'
            + foot(COUNTS))
    return hs_win(fid, "HS-M1B", body)


def win_b_hl(fid, pop=""):
    head = (f'<div class="listhead"><b>{Q2}</b><span>·</span><span>Group: Work</span>'
            '<span style="margin-left:auto">Best matches</span></div>')
    body = (search_b(Q2, semantic=True, fcount=1, group="Work", open_=bool(pop), run='<span class="m-run">⏎ runs</span>') + chips_b() +
            f'<div class="hs-main">{rail_b(("h",))}<div class="m-body">{work_list(head)}{pane_hl()}</div></div>{pop}'
            + foot(f"Highlights · 14,724 works · semantic results {ph()}", scope="h"))
    return hs_win(fid, "HS-M1B", body)


def pop_b():
    def it(label, ind, act=False, cls="", n=""):
        nn = f'<span class="n">{n}</span>' if n else ""
        return f'<div class="m-pi{" act" if act else ""} {cls}"><span class="ind">{ind}</span>{label}{nn}</div>'
    view = ('<div class="m-ps">View</div>'
            '<div class="m-row"><span class="m-l">Group</span><span class="sel">Work</span><span class="small">then</span><span class="sel">—</span></div>'
            '<div class="m-row"><span class="m-l">Sort</span><span class="sel">Best matches</span></div>'
            '<div class="m-row"><span class="m-l">Rows</span><span class="sel">Compact</span></div>'
            '<div class="m-row" style="color:var(--hs-t5)"><span class="m-l" style="color:var(--hs-t5)">Match</span>Whole word · Partial</div>'
            '<div class="m-note">Match applies to keyword search only. Group offers Corpus only when more than one corpus is ticked.</div>')
    anyc = ('<div class="m-ps">Any corpus</div>' + it("Any", "○") + it("30 days", "○") + it("6 months", "○") + it("Year", "○"))
    hl = ('<div class="m-ps">Highlights <em>· X, Readwise, Zotero</em></div>'
          f'<div class="m-row"><span class="m-l">Colour</span>{cdots("red")}</div>'
          '<div class="m-grid"><div>' + it("★ Favorites", "☐", act=True) + it("🔖 Zotero", "☐") + it("🖼 Has image", "☐") + '</div><div>'
          + it("Articles", "☐") + it("Books", "☐") + it("Tweets", "☐") + it("PDFs", "☐") + it("Podcasts", "☐") + '</div></div>'
          '<div class="m-row"><span class="m-l">Tags</span><span class="small">⌘⇧T</span></div>'
          f'<div class="m-tagf">{icon("search", "sm")}Filter by tag…<span style="margin-left:auto;display:flex;gap:4px">'
          '<span class="m-tg">epistemology</span><span class="m-tg">philosophy</span></span></div>')
    left = f'<div>{view}<div class="m-hr"></div>{anyc}</div>'
    right = f'<div style="border-left:1px solid var(--hs-l1);padding-left:6px">{hl}</div>'
    return ('<div class="m-pop" style="right:48px;top:40px;width:640px">'
            f'<div style="display:grid;grid-template-columns:268px 1fr;gap:6px">{left}{right}</div>'
            '<div class="m-foot"><span>↑↓ move</span><span>Space toggles</span><span>Esc closes</span>'
            '<span style="margin-left:auto">Clear all</span></div></div>')


def win_b_home(fid):
    grammar = "cat OR dog · \"exact phrase\" · -exclude · prefix* · au:scott ty:books y:2023 · co:red · i: · /\\bAI\\b/"
    keys = ('<div class="m-keys"><span class="h">Classic tools, same keys</span>'
            '<span>Filters, colour, tags, group, sort, rows</span><span>⌘⇧I</span>'
            '<span>Filter by tag</span><span>⌘⇧T</span>'
            '<span>Cycle group · sort · rows</span><span>⌘⇧G · ⌘⇧S · ⌘⇧D</span>'
            '<span>Clear colour filter</span><span>⌘⇧X</span>'
            '<span>Keyword / Semantic</span><span>the switch in the search box</span></div>')
    home = (f'<div class="m-home"><div class="big" style="font-size:16px;color:var(--hs-t3)">{COUNTS}</div>'
            f'<div class="g">{grammar}</div>{keys}</div>')
    body = (search_b("", placeholder=True) +
            f'<div class="hs-main">{rail_b()}{home}</div>' + foot(COUNTS))
    return hs_win(fid, "HS-M1B", body)


B = direction(
    "HS-M1B", "One list, one Filters control",
    "There are no modes and no scope-dependent tool bars. The rail becomes a set of tick boxes (one or more corpora). "
    "Keyword / Semantic is a switch inside the search box, because it changes how the query is read. Everything else in Classic "
    "goes behind one button, “Filters · Group”, whose popover adapts to the ticked corpora. Anything non-default shows as a removable chip under the search box. "
    "“Grouped by corpus” is no longer a special view: it is just Group = Corpus, the default when more than one corpus is ticked.",
    state(1, "<b>All corpora.</b> Same results as HS-1B, but the header reads “Group: Corpus”, which is the value of the one Group control. No chips row, because nothing is filtered.",
          win_b_all("m1b-1"),
          "Semantic searches highlights only (QMD indexes the highlight library). Switching it on with Writing or Tweets ticked unticks them and says so in the chips row; switching back restores the ticks.")
    + state(2, "<b>Highlights only, grouped by work, red only, semantic on.</b> The button reads “Filters (1) · Group: Work” in amber; the chips row names every non-default choice and each × undoes one.",
            win_b_hl("m1b-2"),
            "The reading pane is the same as HS-M1A’s: every Classic pane action stays.")
    + state(3, "<b>The Filters popover open (⌘⇧I).</b> One popover holds everything Classic spread over the toolbar, the colour row and the Tags overlay: View (Group, then, Sort, Rows, Match), Any corpus (Time) and Highlights (colour, Quick, Type, Tags).",
            win_b_hl("m1b-3", pop=pop_b()),
            "The Highlights section appears only when Highlights, X, Readwise or Zotero is ticked; with All ticked it shows with a line saying its filters narrow results to highlights. Match greys out in semantic mode, as Classic hides it.")
    + state(4, "<b>No hint needed.</b> There is no one-time card. The empty window carries Classic’s grammar line and a short table of the old keys, all unchanged; the old names (Filters, Group, Sort, Rows, Tags) are the words on the one button and in its popover.",
            win_b_home("m1b-4"),
            "Every Classic shortcut still works: ⌘⇧I opens this popover, ⌘⇧T focuses its tag field, ⌘⇧G / ⌘⇧S / ⌘⇧D cycle without opening it, ⌘⇧X clears the colour chip."))

FACTS = ("<span>Drawn at <b>1200 × 780</b> (tauri.conf.json main window)</span>"
         "<span>Writing <b>1,506</b> pieces · Tweets <b>14,892</b> · Highlights <b>14,724</b> works (X 9,683 · Readwise 4,698 · Zotero 343)</span>"
         "<span>Frame 2 rows are real red Zotero highlights from index.sqlite</span><span>Grey pills = counts not known yet</span>")

QS = ["Classic’s Sort says “Most matches / Most recent / Oldest”; the quick finder says “Best matches / Newest first / Oldest first”. Both directions draw one list with the quick-finder words. Keep those, or the Classic ones?",
      "Keyword / Semantic has no shortcut today. Should it get one, and which chord (it must not take one from ADR-0011)?"]

page("hs-m1a-rail-is-the-scope.html", "HS-M1A", "One search: the rail is the scope",
     "Dominik on preview.2: “the way to switch to new search is confusing — we should probably merge the searches”. This direction merges them by letting the rail’s choice reveal the highlight tools.",
     FACTS, A, QS)
page("hs-m1b-one-list-one-filters.html", "HS-M1B", "One search: one list, one Filters control",
     "Dominik on preview.2: “the way to switch to new search is confusing — we should probably merge the searches”. This direction merges them by putting every Classic tool behind one adaptive popover, with semantic as a switch in the search box.",
     FACTS, B, QS)
