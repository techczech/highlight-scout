from lib import *

S = {
    "hall": dict(s="w", meta="2025-12-07 · essay", title="How Humans Hallucinate and How We Deal with It",
                 snip="…the ZPD is not any task achieved with help, a fixed quantity of hidden potential or simply another word for <mark>scaffolding</mark>. Vygotsky’s broader theory of mediated cognition is related but distinct…"),
    "pron": dict(s="w", meta="2023-12-01 · essay", title="Pronominal Paragraphs",
                 snip="There should be visual elements of <mark>scaffolding</mark> or a bulkhead, to represent the structural support provided by these linguistic features."),
    "trans": dict(s="w", meta="2019-06-15 · essay", title="Writing as translation and translation as commitment: Why is (academic) writing so hard?",
                  snip="…regardless of whether we provide lots of ladders and <mark>scaffolding</mark> or just put a trampoline next to the edifice of their skill."),
    "fill": dict(s="w", meta="2014-02-15 · essay", title="Linguistics according to Fillmore",
                 snip="…as ‘schema’, ‘script’, ‘scenario’, ‘ideational <mark>scaffolding</mark>’, ‘cognitive model’, or ‘folk theory’."),
    "step3": dict(s="w", meta="2020-09 · guide", title="Step 3: Use good practice strategies",
                  snip="…memorizing words is not language learning, it is necessary to build up the <mark>scaffolding</mark> you need to get there."),
    "step2": dict(s="w", meta="2020-09 · guide", title="Step 2: Set the right goals", snip="<mark>Scaffolding</mark>: Necessary but not sufficient"),
    "remade": dict(s="w", meta="2026-04-19 · report", title="How I (Claude Code) remade this site",
                   snip="…a reader browsing the content collection sees essays, not plugin <mark>scaffolding</mark>."),
}
ORDER = ["hall", "pron", "trans", "fill", "step3", "step2", "remade"]


def pane():
    it = S["hall"]
    return (f'<div class="hs-pane"><div class="p-title">{it["title"]}</div>'
            f'<div class="p-sub"><span class="src w">Writing</span>Dominik Lukeš · 7 December 2025 · essay</div>'
            f'<div class="p-quote"><p>Chaiklin is useful for resisting the common equation of ZPD with any assisted task or with <mark>scaffolding</mark> in general.</p>'
            f'<p class="dim">…the ZPD is not any task achieved with help, a fixed quantity of hidden potential or simply another word for <mark>scaffolding</mark>.</p></div>'
            f'<div style="margin-top:auto" class="actbar"><span class="act">{icon("quote")}Copy quote <kbd>⌘C</kbd></span>'
            f'<span class="act pri">{icon("cite")}Copy quote + citation <kbd>⌘⇧C</kbd></span></div></div>')


def rows():
    return "".join(hs_row(S[k], on=(k == "hall")) for k in ORDER)


def a1():
    head = (f'<div class="listhead" style="padding:7px 12px 7px 16px"><b>30 pieces · 64 matches</b><span>in Writing</span>'
            f'<span class="hs-btn sm labb" style="margin-left:auto">{icon("lab", "sm")} Analyse in ArchiveScout {icon("ext", "sm")}</span></div>')
    tip = ('<div class="tip" style="left:90px;top:160px">Opens the lab on “scaffolding” in Writing · concordance, collocates, n-grams</div>')
    body = (hs_search("scaffolding", '<span class="hs-btn sm">Filters</span>') + hs_bar() + hs_chips(active=("w",))
            + f'<div class="hs-main"><div class="hs-list">{head}{rows()}</div>{pane()}</div>' + tip
            + hs_foot("Writing 1,506 pieces · Tweets 14,892 · Highlights 14,724 works"))
    return hs_win("hs5a-1", "HS-5A", body)


def b_body(palette=""):
    head = '<div class="listhead"><b>30 pieces · 64 matches</b><span>in Writing</span></div>'
    foot = hs_foot(f'scaffolding · Writing · 30 pieces · 64 matches <span class="link" style="margin-left:6px">{icon("lab", "sm")} Analyse ↗</span>',
                   '<span><kbd>⌘⇧P</kbd> commands</span><span><kbd>⌘C</kbd> quote</span><span><kbd>⌘⇧C</kbd> quote + citation</span><span><kbd>esc</kbd> hide</span>')
    return (hs_search("scaffolding", '<span class="hs-btn sm">Filters</span>') + hs_bar() + hs_chips(active=("w",))
            + f'<div class="hs-main"><div class="hs-list">{head}{rows()}</div>{pane()}</div>' + foot + palette)


def b1():
    return hs_win("hs5b-1", "HS-5B", b_body())


def b2():
    pal = (f'<div class="overlay" style="background:rgba(0,0,0,.18)"></div><div class="palette"><div class="pin">analy<span class="caret"></span></div>'
           f'<div class="pg">This search</div>'
           f'<div class="pi on">{icon("lab")}Analyse “scaffolding” in ArchiveScout<span class="small" style="margin-left:6px">Writing · 64 matches</span><kbd>⏎</kbd></div>'
           f'<div class="pi">{icon("lab")}Analyse in ArchiveScout: Collocates</div>'
           f'<div class="pi">{icon("lab")}Analyse in ArchiveScout: N-grams</div>'
           f'<div class="pg">Selected result</div>'
           f'<div class="pi">{icon("lab")}Analyse this piece in ArchiveScout</div></div>')
    return hs_win("hs5b-2", "HS-5B", b_body(pal))


KWIC = [
    ("2025-12-07 · essay · How Humans Hallucinate…", "…resisting the common equation of ZPD with any assisted task or with", "in general. Sources: Vygotsky…"),
    ("2025-12-07 · essay · How Humans Hallucinate…", "…a fixed quantity of hidden potential or simply another word for", ". Vygotsky’s broader theory of mediated cognition…"),
    ("2023-12-01 · essay · Pronominal Paragraphs", "…referential function of language. There should be visual elements of", "or a bulkhead, to represent the structural support…"),
    ("2019-06-15 · essay · Writing as translation…", "…regardless of whether we provide lots of ladders and", "or just put a trampoline next to the edifice…"),
    ("2014-02-15 · essay · Linguistics according to Fillmore", "…as ‘schema’, ‘script’, ‘scenario’, ‘ideational", "’, ‘cognitive model’, or ‘folk theory’."),
    ("2020-09 · guide · Step 3: Use good practice strategies", "…it is necessary to build up the", "you need to get there. So you will be doing a lot…"),
    ("2026-04-19 · report · How I (Claude Code) remade this site", "…the content collection sees essays, not plugin", "."),
]


def archivescout():
    trs = ""
    for i, (src, l, r) in enumerate(KWIC):
        trs += (f'<tr class="{"on" if i == 0 else ""}"><td class="src2">{src}</td><td class="L"><span>{l}</span></td>'
                f'<td class="K">scaffolding</td><td class="R">{r}</td></tr>')
    for _ in range(8):
        trs += (f'<tr><td>{ph("l")}</td><td style="text-align:right">{ph("l")} {ph("w")}</td><td class="K">scaffolding</td><td>{ph("l")} {ph("w")}</td></tr>')
    bars = "".join(f'<i style="height:{h}px;{"background:var(--as-ink-4)" if h == 3 else ""}"></i>' for h in [3] * 17)
    return (f'<div class="aswin" id="as5-1"><div class="tb"><div class="tl"><i></i><i></i><i></i></div><div class="tt">ArchiveScout<span class="did">HS-5A · HS-5B</span></div></div>'
            f'<div class="as-top"><span class="crumb">My writing › <b>Concordance</b></span><div class="as-views"><span>Survey</span><span class="on">Concordance</span><span>Collocates</span><span>N-grams</span><span>Reports</span></div>'
            f'</div>'
            f'<div class="as-main"><div class="as-rail"><div class="rl">Corpora</div><div class="ri on">My writing<span class="n">1,506</span></div>'
            f'<div class="ri">Tweets<span class="n">14,892</span></div><div class="ri">Highlights<span class="n">14,724</span></div>'
            f'<div class="rl">Filters</div><div class="ri">Language: English</div><div class="ri">Years: all</div><div class="ri">Genre: all</div></div>'
            f'<div class="as-center"><div class="as-from">{icon("check")} <b>From Highlight Scout</b> · same query and corpus · <b>30 pieces · 64 lines</b> = what Highlight Scout showed'
            f'<span style="margin-left:auto">↩ Back to Highlight Scout</span></div>'
            f'<div class="as-q"><span class="qq">scaffolding</span><span class="as-chip">My writing</span><span class="as-chip">Sort: R1</span>'
            f'<span style="margin-left:auto;color:var(--as-ink-3)">30 pieces · 64 lines</span></div>'
            f'<div class="strip" title="hits per year: not computed yet">{bars}<span style="margin-left:10px;font-size:11.5px;color:var(--as-ink-3);align-self:center">hits per year · not computed yet</span></div>'
            f'<table class="kwic"><colgroup><col style="width:210px"><col><col style="width:96px"><col></colgroup><tr><th>Source</th><th style="text-align:right">Left</th><th style="text-align:center">Key</th><th>Right</th></tr>{trs}</table></div>'
            f'<div class="as-ctx"><div class="lab2" style="margin-top:0">Passage</div><div style="font-family:var(--serif, Georgia);font-size:14px;line-height:1.55;color:var(--as-ink-2)">'
            f'Chaiklin is useful for resisting the common equation of ZPD with any assisted task or with <mark style="background:var(--as-mark);color:var(--as-mark-ink)">scaffolding</mark> in general.</div>'
            f'<div class="lab2">Source</div><div style="font-size:12.5px">How Humans Hallucinate and How We Deal with It · 7 December 2025 · essay</div></div></div>'
            f'<div class="as-status"><span>My writing · English · 1.7M words</span><span style="margin-left:auto">Grey rows: the other lines, not drawn</span></div></div>')


A = direction("HS-5A", "A button beside the hit count",
              "The count line at the head of the results carries “Analyse in ArchiveScout ↗” in the lab’s own blue, so it reads as a door to another app. "
              "Hovering says what will open.",
              state(1, "<b><code>scaffolding</code> in Writing: 30 pieces · 64 matches.</b> The button sits on the count it will carry across.", a1()))

B = direction("HS-5B", "A quiet link in the status bar, plus commands",
              "The finder stays a finder: the hand-off is a small “Analyse ↗” link beside the count in the status bar, and a set of commands in the palette (⌘⇧P) "
              "that can open the lab directly on Concordance, Collocates or N-grams.",
              state(1, "<b>The status bar link.</b> Nothing new in the results area.", b1())
              + state(2, "<b>⌘⇧P, “analy…”.</b> Commands for this search and for the selected piece.", b2()))

SHARED = ('<section class="shared"><span class="tag">Both directions land here</span><h2 style="font-size:22px">ArchiveScout opens on the same query and corpus</h2>'
          '<p class="dir-desc">ArchiveScout at its real window (1280 × 820), in its own tokens. The green line confirms the hand-off and the count, which must equal what Highlight Scout showed.</p>'
          + state(1, "<b>ArchiveScout, Concordance on <code>scaffolding</code> in My writing.</b> The first seven lines are real; the grey rows stand for the rest.", archivescout(),
                  "The concordance look here is a stand-in; the corpus-lab design round decides it.") + '</section>')

page("hs-5-analyse-in-archivescout.html", "Surface 5", "“Analyse in ArchiveScout ↗”",
     "Journey J7-C: Highlight Scout is the quick finder, ArchiveScout the corpus lab. The hand-off opens the lab on the same query and corpus, with the same hit count Highlight Scout showed.",
     "<span>Highlight Scout <b>1200 × 780</b> · ArchiveScout <b>1280 × 820</b></span><span>Real count: <b>scaffolding, 64 lines in 30 pieces</b> of his English writing</span>"
     "<span>Every line and title is real</span>",
     A + B + SHARED,
     ["Is the hand-off prominent (HS-5A) or quiet (HS-5B)? It decides how much Highlight Scout advertises the lab.",
      "Should the button show only when the query is one word or phrase, where a concordance makes sense?",
      "What happens when ArchiveScout is not installed or not running: offer to install, or hide the button?",
      "Highlight Scout says “matches” and ArchiveScout “lines” for the same 64. Use one word in both apps?",
      "Should the lab open on Concordance every time, or on the view he last used?"])
