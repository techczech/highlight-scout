from lib import *

CITE_TXT = CITE_SPEC
SENT_M = REPAVED_SENT.replace("new paths", "new <mark>paths</mark>")


def draft_editor():
    return (f'<h1>Metaphor talk</h1><p>{DRAFT_P1}</p><p>As I wrote in 2016:<span class="wcaret"></span></p>')


def under_wf(did):
    return f'<div class="under">{wf_win("", did, draft_editor(), "", side=True, pane_on=False)}</div><div class="scrim"></div>'


def pane_a(toast=False):
    return (f'<div class="hs-pane"><div class="p-title">Repaved paths and generative metaphors: Expressing human purposes with technology</div>'
            f'<div class="p-sub"><span class="src w">Writing</span><span>Dominik Lukeš · 23 June 2016 · essay · medium.com/metaphor-hacker</span></div>'
            f'<div class="p-quote"><p class="dim">{REPAVED_PREV}</p><p><span class="sent">{SENT_M}</span></p></div>'
            f'<div style="margin-top:auto"><div class="lab">Citation</div><div class="cite-box">{CITE_TXT} · <span class="link">open piece</span></div>'
            f'<div class="actbar" style="margin-top:12px">'
            f'<span class="act">{icon("quote")}Copy quote <kbd>⌘C</kbd></span>'
            f'<span class="act pri">{icon("cite")}Copy quote + citation <kbd>⌘⇧C</kbd></span>'
            f'<span class="act">{icon("link")}Copy link</span>'
            f'<span class="act soft">{icon("plus")}Set <kbd>⌘S</kbd></span>'
            f'<span class="link" style="margin-left:auto;font-size:12.5px">Open piece {icon("ext", "sm")}</span></div>'
            f'<p class="small" style="margin-top:8px">Pastes as Markdown into WriteFlex and as rich text into Word or Slack.</p></div></div>')


def list_a():
    rows = [hs_row(I["repaved"], on=True)] + [hs_row(I[k]) for k in ["sword", "tgen", "hack", "fork", "backrow", "edtechie", "weller"]]
    return (f'<div class="hs-list"><div class="listhead"><b>Best matches</b><span>·</span>{ph()}<span>results</span>'
            f'<span style="margin-left:auto">Writing {ph()} · Tweets {ph()} · Highlights {ph()}</span></div>{"".join(rows)}</div>')


def win_a(fid, toast=""):
    right = f'<span class="hs-btn sm">Filters</span><span class="hs-btn sm">{icon("gear")}</span>'
    body = (hs_search("paths metaphor", right) + hs_bar() + hs_chips() +
            f'<div class="hs-main">{list_a()}{pane_a()}</div>' + toast +
            hs_foot("Writing 1,506 pieces · Tweets 14,892 · Highlights 14,724 works"))
    return hs_win(fid, "HS-1A", body)


def a1():
    return f'<div class="desk" id="hs1a-1">{under_wf("")}{win_a("hs1a-1w")}</div>'


def a2():
    toast = (f'<div class="hs-toast">{icon("check")} Copied quote + citation · Markdown'
             f'<span class="x">·</span><span><kbd style="background:transparent;color:inherit;border-color:rgba(127,127,127,.5)">esc</kbd> back to WriteFlex</span></div>')
    return win_a("hs1a-2", toast)


def pasted_strip():
    q = REPAVED_SENT
    wfc = (f'<div style="font-family:\'Source Serif 4\',Georgia,serif;border-left:3px solid var(--pg-line);padding-left:14px;font-size:14.5px;line-height:1.55;color:var(--pg-ink2)">'
           f'<i>{q}</i><div style="margin-top:4px;font-size:13px">{CITE_SPEC} · <u style="color:#2e6be6">open piece</u></div></div>')
    word = (f'<div style="font-family:Calibri,Helvetica,sans-serif;font-size:14px;line-height:1.5;padding-left:28px;color:var(--pg-ink)">'
            f'“{q}”<div style="margin-top:4px">— Dominik Lukeš, <i>Repaved paths and generative metaphors: Expressing human purpose…</i>, 23 June 2016. '
            f'<u style="color:#2e6be6">open piece</u></div></div>')
    slack = (f'<div style="font-size:14px;line-height:1.5;border-left:4px solid var(--pg-line);padding-left:10px;color:var(--pg-ink)">{q}</div>'
             f'<div style="font-size:14px;margin-top:4px;color:var(--pg-ink)">— Dominik Lukeš, <i>Repaved paths and generative metaphors: Expressing human purpose…</i>, 23 June 2016 · <span style="color:#1d9bd1">open piece</span></div>')
    def card(t, c):
        return (f'<div style="flex:1;background:var(--pg-card);border:1px solid var(--pg-line);border-radius:10px;padding:14px 16px">'
                f'<div style="font-size:11px;font-weight:700;letter-spacing:.07em;text-transform:uppercase;color:var(--pg-ink3);margin-bottom:8px">{t}</div>{c}</div>')
    return (f'<div class="pg-row" id="hs1a-3" style="width:1440px;margin-top:16px">{card("Pasted into WriteFlex (Markdown)", wfc)}'
            f'{card("Pasted into Word (rich text)", word)}{card("Pasted into Slack", slack)}</div>')


# ---------- B ----------

def rail_b():
    return ('<div class="rail"><div class="rl">Search in</div>'
            f'<div class="ri on">{icon("search", "sm")}All corpora</div>'
            '<div class="ri"><span class="dot w" style="width:8px;height:8px;border-radius:50%"></span>Writing<span class="n">1,506</span></div>'
            '<div class="ri"><span class="dot t" style="width:8px;height:8px;border-radius:50%"></span>Tweets<span class="n">14,892</span></div>'
            '<div class="ri"><span class="dot h" style="width:8px;height:8px;border-radius:50%"></span>Highlights<span class="n">14,724</span></div>'
            '<div class="ri sub">X<span class="n">9,683</span></div><div class="ri sub">Readwise<span class="n">4,698</span></div>'
            '<div class="ri sub">Zotero<span class="n">343</span></div>'
            f'<div class="rl">Sets</div><div class="ri">{icon("set", "sm")}Metaphor talk<span class="n">7</span></div>'
            f'<div class="ri">{icon("saved", "sm")}metaphor -tweets…<span class="n">{ph()}</span></div>'
            f'<div class="rl">Recent</div><div class="ri" style="color:var(--hs-t3)">{icon("clock", "sm")}scaffolding</div>'
            f'<div class="ri" style="color:var(--hs-t3)">{icon("clock", "sm")}paths metaphor</div>'
            '<div style="margin-top:auto;padding:10px 16px;font-size:11.5px;color:var(--hs-t4)">Counts are pieces, tweets and works.</div></div>')


def acts_b(copied=False):
    if copied:
        return (f'<div class="rowacts"><span class="act" style="border-color:var(--hs-green);color:var(--hs-green);background:var(--hs-green-bg)">'
                f'{icon("check", "sm")}Copied quote + citation</span><span class="act soft">esc back to WriteFlex</span></div>')
    return (f'<div class="rowacts"><span class="act">{icon("quote", "sm")}Quote <kbd>⌘C</kbd></span>'
            f'<span class="act pri">{icon("cite", "sm")}Quote + citation <kbd>⌘⇧C</kbd></span>'
            f'<span class="act">{icon("link", "sm")}Link</span><span class="act">+ Set</span></div>')


def list_b(copied=False):
    def sec(name, dot):
        return f'<div class="sechead"><span class="dot {dot}" style="width:7px;height:7px;border-radius:50%"></span>{name}{ph()}<span class="more">show all</span></div>'
    return ('<div class="hs-list" style="width:450px">'
            '<div class="listhead"><b>paths metaphor</b><span>·</span><span>grouped by corpus</span><span class="sel" style="margin-left:auto">Best matches</span></div>'
            + sec("Writing", "w") + hs_row(I["repaved"], on=True, acts=acts_b(copied)) + hs_row(I["hack"]) + hs_row(I["backrow"])
            + sec("Tweets", "t") + hs_row(I["tgen"])
            + sec("Highlights", "h") + hs_row(I["sword"]) + hs_row(I["fork"]) + '</div>')


def pane_b():
    md = ("&gt; But being open to repaving once new paths are trodden alongside those you outlined with your technology "
          "(be it critical pedagogy or iPads) is perhaps the most important thing we can do in our ed tech efforts.<br>&gt;<br>"
          "&gt; — Dominik Lukeš, *Repaved paths and generative metaphors: Expressing human purpose…*, 23 June 2016 · "
          "[open piece](https://medium.com/metaphor-hacker/repaved-paths-…)")
    return (f'<div class="hs-pane" style="padding:18px 22px 14px"><div class="p-title" style="font-size:16px">Repaved paths and generative metaphors: Expressing human purposes with technology</div>'
            f'<div class="p-sub"><span>Dominik Lukeš · 23 June 2016 · essay</span><span class="link">Open piece {icon("ext", "sm")}</span></div>'
            f'<div class="p-quote" style="font-size:14.5px"><p class="dim">{REPAVED_PREV}</p><p><span class="sent">{SENT_M}</span></p></div>'
            f'<div style="margin-top:auto"><div style="display:flex;align-items:center;gap:8px;margin-bottom:6px"><span class="lab" style="margin:0">⌘⇧C copies</span>'
            f'<span class="sel" style="margin-left:auto;font-size:12px">Auto: Markdown in WriteFlex, rich text elsewhere</span></div>'
            f'<div class="cite-box" style="font:11.5px/1.5 ui-monospace,Menlo,monospace;color:var(--hs-t3)">{md}</div>'
            f'<div class="actbar" style="margin-top:10px"><span class="act">{icon("spark")}Find related</span><span class="act">Show work highlights →</span></div></div></div>')


def win_b(fid, copied=False):
    right = f'<span class="small">actions on selection</span><kbd>⌘K</kbd>'
    foot = ("Copied · esc returns to WriteFlex" if copied else "Writing 1,506 pieces · Tweets 14,892 · Highlights 14,724 works")
    body = (hs_search("paths metaphor", right) + f'<div class="hs-main">{rail_b()}{list_b(copied)}{pane_b()}</div>'
            + hs_foot(foot, '<span>↑↓ nav</span><span><kbd>⌥↓</kbd> next corpus</span><span><kbd>⌘C</kbd> quote</span><span><kbd>⌘⇧C</kbd> + citation</span><span><kbd>esc</kbd> hide</span>'))
    return hs_win(fid, "HS-1B", body)


def b1():
    return f'<div class="desk" id="hs1b-1">{under_wf("")}{win_b("hs1b-1w")}</div>'


def b2():
    return win_b("hs1b-2", copied=True)


A = direction("HS-1A", "The finder you know, with a corpus row",
              "Today’s Highlight Scout layout kept whole: search line, the view toolbar, results on the left (46%) and the reading pane on the right. "
              "A new row of corpus chips replaces the colour row. Every result starts with its source badge. The copy actions sit at the foot of the reading pane, "
              "with ⌘⇧C as the one dark button.",
              state(1, "<b>⌘⌥⇧H over WriteFlex.</b> Highlight Scout opens centred at its real size (1200 × 780) over WriteFlex (1440 × 900). He types <code>paths metaphor</code>.", a1(),
                    "The window is the normal app window, not a separate mini window. Per-corpus hit counts are grey because no count for this query exists yet.")
              + state(2, "<b>⌘⇧C copies the quote with its citation.</b> A toast confirms the format; Esc hides the window and returns focus to WriteFlex.", a2())
              + state(3, "<b>What he gets when he pastes.</b> One copy, three renderings: Markdown blockquote in WriteFlex, rich text in Word, a quote block in Slack.", pasted_strip(),
                      "Word and Slack renderings are drawn to show the formatting, not captured from those apps."))

B = direction("HS-1B", "Corpus rail, results grouped by corpus",
              "The corpora move into a quiet left rail (210 px) with their real sizes, sets and recent searches, so the left edge holds the breathing room. "
              "Results are grouped Writing / Tweets / Highlights, so his own words always come first. "
              "The copy actions ride on the selected row itself; the reading pane shows exactly what ⌘⇧C will put on the clipboard.",
              state(1, "<b>⌘⌥⇧H over WriteFlex.</b> Same window size, three columns: rail, grouped results, reading pane.", b1(),
                    "⌥↓ jumps to the next corpus group. Sort and group move into the list header, so the toolbar row is gone.")
              + state(2, "<b>After ⌘⇧C.</b> The row confirms in place; the footer tells him Esc returns to WriteFlex.", b2()))

page("hs-1-search-three-corpora.html", "Surface 1", "One search over three corpora",
     "Highlight Scout summoned with ⌘⌥⇧H over another app (journey J1-B): corpus chips or rail, results with the source badge first, the reading pane, and copy with citation (⌘C quote, ⌘⇧C quote + citation, link).",
     "<span>Drawn at <b>1200 × 780</b> (tauri.conf.json main window) over WriteFlex at <b>1440 × 900</b></span>"
     "<span>Writing <b>1,506</b> pieces · Tweets <b>14,892</b> · Highlights <b>14,724</b> works (X 9,683 · Readwise 4,698 · Zotero 343)</span>"
     "<span>Grey pills = counts not known yet</span>",
     A + B,
     ["Should “All” interleave the corpora by rank (HS-1A) or always group his own writing first (HS-1B)?",
      "Copy link has no shortcut: ⌘L is already “Focus search box” in Highlight Scout. Leave it unbound, or give it one?",
      "The citation line truncates the title (“…Expressing human purpose…”) as the journey text does. Should the pasted citation carry the full title instead?",
      "Does “link” mean the public URL (medium.com/metaphor-hacker/…) or a link that opens the piece in the archive (WriteFlex or Highlight Scout)? Drawn as the public URL.",
      "⌘⇧C is today’s “Copy as Markdown”; this round makes it “quote + citation”. Existing ⌘⇧K “Copy citation” also collides with the estate’s ⌘⇧K navigation switcher."],
     fonts=True)
