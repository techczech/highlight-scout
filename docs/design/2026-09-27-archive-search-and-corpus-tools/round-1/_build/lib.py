"""Shared helpers for the round-1 mockups. Output is plain static HTML."""
import os

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.expanduser(
    "~/gitrepos/06_apps-utilities/01_desktop-apps/highlight-scout/docs/design/"
    "2026-09-27-archive-search-and-corpus-tools/round-1")

DARK = """
  --pg-bg:#141416;--pg-ink:#ececef;--pg-ink2:#a6a6ae;--pg-ink3:#7c7c84;--pg-line:rgba(255,255,255,.13);--pg-card:#1d1d20;--badge:#23a059;
  --desk:linear-gradient(135deg,#1f2a36 0%,#2c2822 100%);
  --hs-s0:#1b1b1e;--hs-s1:#202024;--hs-s2:#27272b;--hs-s3:#34343a;
  --hs-l1:#28282c;--hs-l2:#36363c;
  --hs-t1:#f4f4f5;--hs-t2:#d4d4d8;--hs-t3:#a1a1aa;--hs-t4:#7a7a83;--hs-t5:#52525b;
  --hs-tb:#2a2a2e;--hs-active:rgba(251,191,36,.10);--hs-amber:#fbbf24;--hs-amber-ink:#fcd34d;--hs-mark:rgba(250,204,21,.32);--hs-sent:rgba(250,204,21,.18);
  --hs-blue:#60a5fa;--hs-violet:#c4b5fd;--hs-violet-bg:rgba(139,92,246,.12);--hs-violet-line:rgba(167,139,250,.35);
  --hs-red:#fca5a5;--hs-red-bg:rgba(239,68,68,.13);--hs-red-line:rgba(248,113,113,.35);--hs-green:#6ee7b7;--hs-green-bg:rgba(16,185,129,.14);
  --src-w:#6ee7b7;--src-w-bg:rgba(16,185,129,.16);--src-t:#7dd3fc;--src-t-bg:rgba(14,165,233,.16);--src-h:#fcd34d;--src-h-bg:rgba(245,158,11,.16);
  --hs-toast:#f4f4f5;--hs-toast-ink:#18181b;--hs-ph:#3f3f46;--hs-shadow:0 0 0 .5px rgba(255,255,255,.14),0 22px 60px rgba(0,0,0,.6);
  --wf-ink:#e7e7ea;--wf-ink-2:#a6a6ae;--wf-ink-3:#92929b;--wf-ink-4:#5f5f68;
  --wf-line:rgba(255,255,255,.13);--wf-line-soft:rgba(255,255,255,.07);
  --wf-bg-toolbar:#242427;--wf-bg-side:#212124;--wf-bg-pane:#1a1a1e;--wf-bg-sunk:#202024;
  --wf-pill:#333338;--wf-hover:rgba(255,255,255,.06);
  --wf-accent:#5f8ef2;--wf-accent-soft:rgba(95,142,242,.18);
  --wf-review:#55b6a2;--wf-review-bg:rgba(85,182,162,.17);
  --wf-amber:#d2a04a;--wf-amber-soft:rgba(210,160,74,.16);
  --wf-note:#a58cf5;--wf-note-bg:rgba(165,140,245,.17);
  --wf-bg-elev:#242428;--wf-prose-ink:#d9d9de;--wf-mark:rgba(250,204,21,.28);
  --as-bg:#15171c;--as-panel:#1c1f26;--as-panel-2:#191c22;--as-panel-3:#22262e;--as-border:#2b2f38;--as-border-2:#333844;
  --as-ink:#e7e9ee;--as-ink-2:#a6adb8;--as-ink-3:#7b828e;--as-ink-4:#5c626d;--as-accent:#7f92ee;--as-accent-soft:#232838;--as-accent-line:#3a4468;
  --as-ok:#5fbd88;--as-ok-bg:#1c2c24;--as-mark:#4a4118;--as-mark-ink:#f4e6a8;
"""


def css(with_fonts=False):
    base = open(os.path.join(HERE, "common.css")).read()
    dark = ("@media (prefers-color-scheme: dark){:root:not([data-theme=\"light\"]){" + DARK + "}}\n"
            ":root[data-theme=\"dark\"]{" + DARK + "}\n")
    base = base.replace("/*DARK*/", dark)
    if with_fonts:
        faces = []
        ranges = {
            "latin": "U+0000-00FF,U+0131,U+0152-0153,U+02BB-02BC,U+02C6,U+02DA,U+02DC,U+0304,U+0308,U+0329,U+2000-206F,U+20AC,U+2122,U+2191,U+2193,U+2212,U+2215,U+FEFF,U+FFFD",
            "latin-ext": "U+0100-02BA,U+02BD-02C5,U+02C7-02CC,U+02CE-02D7,U+02DD-02FF,U+1D00-1DBF,U+1E00-1E9F,U+1EF2-1EFF,U+2020,U+20A0-20AB,U+20AD-20C0,U+2113,U+2C60-2C7F,U+A720-A7FF",
        }
        for sub in ("latin", "latin-ext"):
            for style in ("normal", "italic"):
                b64 = open(os.path.join(HERE, f"{sub}-opsz-{style}.b64")).read().strip()
                faces.append("@font-face{font-family:'Source Serif 4';font-style:%s;font-weight:200 900;"
                             "src:url(data:font/woff2;base64,%s) format('woff2-variations');unicode-range:%s}"
                             % (style, b64, ranges[sub]))
        base = "\n".join(faces) + "\n" + base
    return base


ICONS = {
    "search": '<circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/>',
    "link": '<path d="M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1"/><path d="M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1"/>',
    "copy": '<rect x="9" y="9" width="11" height="11" rx="2"/><path d="M5 15V5a2 2 0 0 1 2-2h8"/>',
    "quote": '<path d="M7 7h4v4c0 3-1.5 5-4 6"/><path d="M14 7h4v4c0 3-1.5 5-4 6"/>',
    "grip": '<circle cx="9" cy="6" r="1"/><circle cx="15" cy="6" r="1"/><circle cx="9" cy="12" r="1"/><circle cx="15" cy="12" r="1"/><circle cx="9" cy="18" r="1"/><circle cx="15" cy="18" r="1"/>',
    "pin": '<path d="M12 17v5"/><path d="M9 3h6l-1 6 3 3v2H7v-2l3-3z"/>',
    "check": '<path d="m5 12 5 5 9-10"/>',
    "x": '<path d="M6 6l12 12M18 6 6 18"/>',
    "ext": '<path d="M14 4h6v6"/><path d="M20 4 10 14"/><path d="M19 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1h5"/>',
    "spark": '<path d="M12 3l1.8 5.2L19 10l-5.2 1.8L12 17l-1.8-5.2L5 10l5.2-1.8z"/>',
    "plus": '<path d="M12 5v14M5 12h14"/>',
    "gear": '<circle cx="12" cy="12" r="3"/><path d="M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9l2.1 2.1M17 17l2.1 2.1M4.9 19.1 7 17M17 7l2.1-2.1"/>',
    "sidebar": '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M9 4v16"/>',
    "panel": '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M15 4v16"/>',
    "book": '<path d="M4 5a2 2 0 0 1 2-2h13v16H6a2 2 0 0 0-2 2z"/><path d="M4 19V5"/>',
    "pencil": '<path d="M4 20h4L19 9l-4-4L4 16z"/>',
    "refresh": '<path d="M20 11a8 8 0 1 0-2.3 5.7"/><path d="M20 4v7h-7"/>',
    "alert": '<circle cx="12" cy="12" r="9"/><path d="M12 8v5M12 16h.01"/>',
    "down": '<path d="m6 9 6 6 6-6"/>',
    "up": '<path d="m6 15 6-6 6 6"/>',
    "list": '<path d="M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01"/>',
    "doc": '<path d="M14 3H6a1 1 0 0 0-1 1v16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V8z"/><path d="M14 3v5h5"/>',
    "slides": '<rect x="3" y="4" width="18" height="12" rx="1.5"/><path d="M12 16v4M8 20h8"/>',
    "cite": '<path d="M5 6h14M5 10h14M5 14h9M5 18h6"/>',
    "folder": '<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>',
    "clock": '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>',
    "lab": '<path d="M9 3h6M10 3v6l-5 9a2 2 0 0 0 1.7 3h10.6a2 2 0 0 0 1.7-3l-5-9V3"/><path d="M7.5 15h9"/>',
    "set": '<rect x="4" y="4" width="16" height="5" rx="1.5"/><rect x="4" y="11" width="16" height="4" rx="1.5"/><rect x="4" y="17" width="16" height="3" rx="1.5"/>',
    "agent": '<rect x="5" y="8" width="14" height="11" rx="3"/><path d="M12 4v4M9 13h.01M15 13h.01"/>',
    "saved": '<path d="M6 3h12v18l-6-4-6 4z"/>',
}


def icon(name, cls=""):
    return f'<svg class="icon {cls}" viewBox="0 0 24 24" aria-hidden="true">{ICONS[name]}</svg>'


def ph(size=""):
    return f'<span class="ph {size}" title="number not known yet"></span>'


SRC_LABEL = {"w": "Writing", "t": "Tweet", "h": "Highlight"}


def hs_row(it, on=False, acts="", lead="", extra_cls="", show_title=True, one=False):
    s = it["s"]
    parts = [f'<span class="src {s}">{SRC_LABEL[s]}</span>']
    if it.get("sub"):
        parts.append(f'<span>{it["sub"]}</span><span>·</span>')
    if it.get("by"):
        parts.append(f'<span class="by">{it["by"]}</span><span>·</span>')
    parts.append(f'<span style="overflow:hidden;text-overflow:ellipsis">{it["meta"]}</span>')
    if it.get("tag"):
        parts.append(it["tag"])
    title = f'<div class="rtitle">{it["title"]}</div>' if (it.get("title") and show_title) else ""
    snip = f'<div class="snip{" one" if one else ""}">{it["snip"]}</div>'
    return (f'<div class="row{" on" if on else ""} {extra_cls}">{lead}<div class="body">'
            f'<div class="meta1">{"".join(parts)}</div>{title}{snip}{acts}</div></div>')


def hs_win(fid, did, body, title="Highlight Scout", cls="", style=""):
    return (f'<div class="win {cls}" id="{fid}" style="{style}"><div class="tb"><div class="tl"><i></i><i></i><i></i></div>'
            f'<div class="tt">{title}<span class="did">{did}</span></div></div>'
            f'<div class="wb">{body}</div></div>')


def hs_search(q, right="", caret=True, ghost=""):
    c = '<span class="caret"></span>' if caret else ""
    g = f'<span class="ghost">{ghost}</span>' if ghost else ""
    return (f'<div class="hs-search">{icon("search", "lg")}<div class="q">{q}{c}{g}</div>{right}</div>')


def hs_bar(extra=""):
    return ('<div class="hs-bar"><span class="seg"><span class="on">Keyword</span><span>Semantic</span></span>'
            '<span>Sort</span><span class="sel">Most matches</span><span>Group</span><span class="sel">—</span>'
            f'<span>Rows</span><span class="sel">Compact</span>{extra}</div>')


def hs_chips(active=("all",), extra=""):
    def c(key, label, dot=""):
        on = " on" if key in active else ""
        d = f'<span class="dot {dot}"></span>' if dot else ""
        return f'<span class="chip{on}">{d}{label}<span class="n">{ph()}</span></span>'
    return ('<div class="chips"><span class="lbl">Search in</span>'
            + c("all", "All")
            + c("w", "Writing", "w") + c("t", "Tweets", "t") + c("h", "Highlights", "h")
            + f'{extra}</div>')


def hs_foot(left, right=None):
    if right is None:
        right = ('<span>↑↓ nav</span><span><kbd>⏎</kbd> open</span><span><kbd>⌘C</kbd> quote</span>'
                 '<span><kbd>⌘⇧C</kbd> quote + citation</span><span><kbd>⌘S</kbd> + set</span><span><kbd>esc</kbd> hide</span>')
    return f'<div class="hs-foot"><span style="display:flex;align-items:center;gap:6px;min-width:0">{left}</span><span class="r">{right}</span></div>'


def state(n, caption, frame, note=""):
    nt = f'<p class="note">{note}</p>' if note else ""
    return f'<figure class="state"><figcaption><span class="n">{n}</span>{caption}</figcaption>{frame}{nt}</figure>'


def direction(did, name, desc, states_html):
    return (f'<section class="dir" id="{did}"><div class="dir-head"><span class="badge">{did}</span><h2>{name}</h2></div>'
            f'<p class="dir-desc">{desc}</p>{states_html}</section>')


def page(fname, surf, title, intro, facts, body, questions, fonts=False):
    qs = "".join(f"<li>{q}</li>" for q in questions)
    html = f"""<!doctype html>
<html lang="en-GB"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>{title}</title>
<style>{css(fonts)}</style></head>
<body><div class="page">
<header class="page-head"><span class="surf">{surf}</span><div><h1>{title}</h1><p>{intro}</p></div>
<div class="theme-tg" role="group" aria-label="Theme"><button data-t="light">Light</button><button data-t="dark">Dark</button><button data-t="auto">Auto</button></div></header>
<div class="facts">{facts}</div>
{body}
<section class="questions"><h2>Open design questions</h2><ol>{qs}</ol></section>
</div>
<script>
(function(){{var r=document.documentElement,p=new URLSearchParams(location.search).get('theme');
function set(t){{if(t==='auto')r.removeAttribute('data-theme');else r.setAttribute('data-theme',t);
document.querySelectorAll('.theme-tg button').forEach(function(b){{b.classList.toggle('on',b.dataset.t===t)}});}}
set(p||'auto');document.querySelectorAll('.theme-tg button').forEach(function(b){{b.onclick=function(){{set(b.dataset.t)}}}});}})();
</script></body></html>"""
    os.makedirs(OUT, exist_ok=True)
    with open(os.path.join(OUT, fname), "w") as f:
        f.write(html)
    print("wrote", fname, len(html))


# ---------- real data ----------
CITE_SPEC = "— Dominik Lukeš, <em>Repaved paths and generative metaphors: Expressing human purpose…</em>, 23 June 2016"
REPAVED_SENT = ("But being open to repaving once new paths are trodden alongside those you outlined with your technology "
                "(be it critical pedagogy or iPads) is perhaps the most important thing we can do in our ed tech efforts.")
REPAVED_PREV = ("An oft-quoted (possibly apocryphal) example is of Stanford not building any paths on their new campus. "
                "They simply looked at paths trodden in the grass after a few semesters and then paved them. This is a "
                "technique that would be hard to replicate in most contexts (although the Agile development philosophy "
                "tries this with programming and tech start ups try this with pivots).")

I = {
    "repaved": dict(s="w", meta="2016-06-23 · essay", title="Repaved paths and generative metaphors: Expressing human purposes with technology",
                    snip="But being open to repaving once new <mark>paths</mark> are trodden alongside those you outlined with your technology (be it critical pedagogy or iPads) is perhaps the most important thing we can do in our ed tech efforts."),
    "sword": dict(s="h", sub="Readwise", by="Helen Sword", meta="Air &amp; Light &amp; Time &amp; Space",
                  snip="In his book The Courage to Teach, Palmer invokes the <mark>metaphor</mark> of a sheepdog… to demonstrate how generative <mark>metaphors</mark> can reveal complex truths about a teacher’s practice."),
    "tgen": dict(s="t", meta="2025-07-26 · @techczech",
                 snip="Hey, this is actually a very useful generative <mark>metaphor</mark> to bring up when people say the problem with LLMs is that they “don’t really think”. Just takes a bit of unpacking."),
    "hack": dict(s="w", meta="2010-07-18 · guide", title="Hacking a metaphor in five steps",
                 snip="Find an example of a <mark>metaphor</mark> being used in a way that limits your ability to achieve something or one that constrains your thinking or actions. For example, “education is a marketplace.”"),
    "fork": dict(s="h", sub="Readwise", by="Andrew", meta="The Competing Narratives of Scientific Revolution",
                 snip="forking <mark>paths</mark> multiply as fast as p-values."),
    "backrow": dict(s="w", meta="2020-06-28 · essay", title="No back row, no corridor: Metaphors for online teaching and learning",
                    snip="The best way I found to bring the contrast between the physical and the virtual into focus are two <mark>metaphors</mark> that can be summarised as “No back row” and “No corridor”."),
    "edtechie": dict(s="h", sub="Readwise", by="blog.edtechie.net", meta="What I Learnt From Being a Student",
                     snip="ensuring there are <mark>paths</mark> through the course that don’t assume full capacity but are still rewarding is essential"),
    "weller": dict(s="h", sub="Readwise", by="mweller", meta="What’s in a Name? Early Internet Metaphors",
                   snip="<mark>Metaphors</mark> are very powerful in this respect as they provide a bridge from the familiar to the unfamiliar."),
    "mitchell": dict(s="h", sub="Readwise", by="Melanie Mitchell", meta="Complexity",
                     snip="the key to analogy-making in this microworld (as well as in the real world) is what I am calling conceptual slippage."),
    "treply": dict(s="t", meta="2020-08-07 · reply to @OnlineCrsLady",
                   snip="This is so fundamental but it goes against the frame teaching=&gt;learning. I’ve argued against explanations as central to the process here…"),
    "ships": dict(s="w", meta="2018-05-22 · essay", title="Not ships in the night: Metaphor and simile as process",
                  snip="If you study <mark>metaphor</mark> in context, this will not surprise you. The blend is projected into another domain that is in a complex relationship to what precedes and what follows."),
    "anthro": dict(s="w", meta="2025-08 · essay", title="How metaphors work: Case of anthropomorphism extrapolation",
                   snip="This started as a short preamble to a different post."),
    "actors": dict(s="w", meta="2016-02-16 · essay", title="If teachers are actors, is instructional design stage directions? Exploration of a metaphor.",
                   snip="There are many ways in which the teacher’s experience is similar to that of an actor."),
    "oer": dict(s="h", sub="Readwise", by="david", meta="Questioning the OER Orthodoxy: Is the Commons the Right Metaphor…",
                snip="The <mark>metaphors</mark> we choose to employ literally determine what we will see, consider, and understand – as well as what we will not."),
    "fruit": dict(s="h", sub="Readwise", by="Dominik Lukeš", meta="Fruit Loops and Metaphors",
                  snip="We do not really understand one domain in terms of another through <mark>metaphor</mark>. We ‘understand’ both domains in different ways."),
    "gori": dict(s="h", sub="Readwise", by="Tim Gorichanaz", meta="Three Perfections: A Metaphor for Document Theory",
                 snip="<mark>Metaphors</mark> are a means through which human thought is structured."),
    "preece": dict(s="h", sub="Readwise", by="Preece Rogers", meta="Interaction Design",
                   snip="<mark>metaphors</mark> are considered to be a central component of a conceptual model."),
}

# ---------- WriteFlex window ----------

def wf_tool(did, active="Metaphor talk", side_on=True, pane_on=True):
    tabs = "".join(f'<div class="wtab{" on" if t == active else ""}">{t}</div>'
                   for t in ["Metaphor talk", "Frame negotiation", "Hypostasis"])
    return ('<div class="wf-tool"><div class="wf-tl"><i></i><i></i><i></i></div><div class="tspacer"></div>'
            f'<div class="tbtn{" on" if side_on else ""}">{icon("sidebar")}</div><div class="tbtn">{icon("book")}</div>'
            f'<div class="tabstrip">{tabs}</div>'
            + (f'<span class="did" style="align-self:center">{did}</span>' if did else '') +
            f'<div class="mode">{icon("pencil", "sm")} Drafting {icon("down", "sm")}</div>'
            f'<div class="tbtn" style="font-size:15px">⌘</div><div class="tbtn{" on" if pane_on else ""}">{icon("panel")}</div></div>')


def wf_side():
    return ('<div class="wf-side"><div style="display:flex;gap:4px;padding:0 10px 8px;font-size:12px;color:var(--wf-ink-3)">'
            '<span style="padding:3px 8px;border-radius:6px;background:var(--wf-bg-pane);color:var(--wf-ink);border:1px solid var(--wf-line)">Files</span>'
            '<span style="padding:3px 8px">Search</span><span style="padding:3px 8px">Open tabs</span></div>'
            '<div class="sl">Talks</div><div class="si on">Metaphor talk</div><div class="si">Frame negotiation</div>'
            '<div class="sl">Writing</div><div class="si">Hypostasis</div><div class="si">How Humans Hallucinate</div>'
            '<div class="si">Pronominal Paragraphs</div><div class="sl">Notebooks</div><div class="si">Reading notes</div></div>')


DRAFT_P1 = ("Before you start metaphor hacking you must first accept that you don’t have a choice but to speak in some sort "
            "of a figurative fashion. Almost nothing worth saying is entirely literal and there are many things whose "
            "“literalness” is rooted in metaphor.")


def wf_win(fid, did, editor, pane, side=True, cls="", style="", pane_on=True, extra=""):
    s = wf_side() if side else ""
    return (f'<div class="wfwin {cls}" id="{fid}" style="{style}">{wf_tool(did, side_on=side, pane_on=pane_on)}'
            f'<div class="wf-body">{s}<div class="wf-editor"><div class="doc">{editor}</div></div>{pane}</div>{extra}</div>')
