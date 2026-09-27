from lib import *

SET7 = ["repaved", "sword", "hack", "tgen", "weller", "backrow", "mitchell"]
NOTES = {"mitchell": "Generative MEtaphor or metaphor hacking"}

EXTRA_CSS = """
<style>
.tray{flex:none;display:flex;align-items:center;gap:10px;padding:7px 16px;border-top:1px solid var(--hs-l2);background:var(--hs-s2);white-space:nowrap;overflow:hidden}
.tray .nm{font-weight:650;font-size:13px;color:var(--hs-t1)}
.tray .mini{display:inline-block;white-space:nowrap;text-overflow:ellipsis;align-items:center;gap:5px;font-size:11.5px;color:var(--hs-t3);background:var(--hs-s0);border:1px solid var(--hs-l2);border-radius:6px;padding:2px 7px;max-width:170px;overflow:hidden;text-overflow:ellipsis}
.inset{font-size:10.5px;font-weight:650;color:var(--hs-green);background:var(--hs-green-bg);border-radius:4px;padding:0 6px;line-height:15px}
.addset{font-size:11px;color:var(--hs-t3);border:1px dashed var(--hs-t5);border-radius:5px;padding:0 6px;line-height:15px}
.setv{flex:1;display:flex;min-height:0}
.setl{width:70%;flex:none;display:flex;flex-direction:column;border-right:1px solid var(--hs-l2);min-height:0;overflow:hidden}
.setr{flex:1;min-width:0;padding:16px 18px;background:var(--hs-s1);display:flex;flex-direction:column;gap:10px;overflow:hidden}
.seth{display:flex;align-items:center;gap:10px;padding:12px 18px 10px;border-bottom:1px solid var(--hs-l2);white-space:nowrap}
.seth .nm{font-size:18px;font-weight:700}
.seth .sub{font-size:12.5px;color:var(--hs-t3)}
.si{display:flex;gap:10px;padding:10px 18px 10px 10px;border-bottom:1px solid var(--hs-l1);background:var(--hs-s0);position:relative}
.si .num{width:18px;text-align:right;color:var(--hs-t4);font-size:12px;padding-top:2px;flex:none;font-variant-numeric:tabular-nums}
.si .g{padding-top:1px;flex:none}
.si .note{margin-top:6px;font-size:12.5px;color:var(--hs-t3);display:flex;gap:6px;align-items:center}
.si .note.has{color:var(--hs-t2)}
.si .note .nf{border:1px solid var(--hs-l2);border-radius:6px;padding:2px 8px;flex:1;background:var(--hs-s1)}
.si .note .nf.edit{border-color:var(--hs-blue);box-shadow:0 0 0 3px rgba(59,130,246,.15);background:var(--hs-s0);color:var(--hs-t1)}
.si.lift{box-shadow:0 12px 30px rgba(0,0,0,.22),0 0 0 1px var(--hs-l2);transform:translate(6px,-6px) rotate(-.3deg);z-index:3;border-radius:8px}
.dropline{height:0;border-top:2px solid var(--hs-blue);margin:0 18px 0 38px;position:relative;z-index:2}
.dropline::before{content:"";position:absolute;left:-5px;top:-5px;width:8px;height:8px;border-radius:50%;border:2px solid var(--hs-blue);background:var(--hs-s0)}
.si.gap{height:52px;background:var(--hs-s2);border:1px dashed var(--hs-t5)}
.xcard{border:1px solid var(--hs-l2);border-radius:9px;background:var(--hs-s0);padding:11px 12px;display:flex;gap:10px}
.xcard .ic{width:30px;height:30px;border-radius:7px;background:var(--hs-s2);display:flex;align-items:center;justify-content:center;flex:none;color:var(--hs-t2)}
.xcard b{font-size:13px;display:block}
.xcard p{font-size:12px;color:var(--hs-t3);margin-top:1px;line-height:1.4}
.pinb{flex:none;width:26px;height:26px;border-radius:6px;display:flex;align-items:center;justify-content:center;color:var(--hs-t4);border:1px solid transparent}
.pinb.on{color:var(--hs-amber-ink);background:var(--hs-active);border-color:var(--hs-amber)}
.newtag{font-size:10px;font-weight:700;letter-spacing:.04em;text-transform:uppercase;color:var(--hs-blue);border:1px solid currentColor;border-radius:4px;padding:0 5px;line-height:14px}
.gh{padding:6px 18px;font-size:11px;font-weight:650;letter-spacing:.05em;text-transform:uppercase;color:var(--hs-t4);background:var(--hs-s2);border-bottom:1px solid var(--hs-l1);display:flex;gap:8px;align-items:center}
.gh span.r{margin-left:auto;text-transform:none;letter-spacing:0;font-weight:400}
.struck .snip,.struck .rtitle{text-decoration:line-through;text-decoration-color:var(--hs-t5);color:var(--hs-t4)}
.sugg{margin:0;border-bottom:1px solid var(--hs-violet-line);background:var(--hs-violet-bg);padding:11px 18px 12px 38px}
.sugg .why{margin-top:7px;font-size:12.5px;line-height:1.5;color:var(--hs-t2);border-left:2px solid var(--hs-violet);padding-left:10px}
.sugg .why u{text-decoration:none;box-shadow:inset 0 -2px 0 var(--hs-violet)}
.sugg .like{font-size:11.5px;color:var(--hs-t3);margin-top:6px}
.sugg .btns{display:flex;gap:6px;margin-top:8px}
.sugg.done{background:var(--hs-green-bg);border-color:transparent}
.sugg.no{opacity:.5;background:var(--hs-s1)}
.vbtn{display:inline-flex;align-items:center;gap:5px;border-radius:6px;padding:3px 9px;font-size:12px;border:1px solid var(--hs-l2);background:var(--hs-s0);color:var(--hs-t2)}
.vbtn.ok{background:var(--hs-green);border-color:var(--hs-green);color:var(--hs-s0)}
.drawer{flex:1;min-width:0;display:flex;flex-direction:column;background:var(--hs-s1);min-height:0;position:relative}
.drawer .dh{display:flex;align-items:center;gap:8px;padding:10px 14px;border-bottom:1px solid var(--hs-l2);white-space:nowrap;background:var(--hs-s0)}
.drawer .dh .nm{font-weight:700;font-size:15px}
.dc{margin:6px 10px 0;padding:9px 11px;border:1px solid var(--hs-l2);border-radius:9px;background:var(--hs-s0);display:flex;gap:8px;position:relative}
.dc .body{min-width:0;flex:1}
.dc.new{border-color:var(--hs-amber);box-shadow:0 0 0 3px var(--hs-active)}
.dc.lift{box-shadow:0 14px 30px rgba(0,0,0,.25);transform:translate(10px,-4px) rotate(-.5deg);z-index:3}
.dfoot{margin-top:auto;display:flex;align-items:center;gap:8px;padding:10px 14px;border-top:1px solid var(--hs-l2);background:var(--hs-s0);white-space:nowrap}
.pop{position:absolute;right:14px;bottom:52px;width:330px;background:var(--hs-s0);border:1px solid var(--hs-l2);border-radius:11px;box-shadow:0 18px 44px rgba(0,0,0,.28);padding:6px;z-index:6}
.pop .pi{display:flex;gap:10px;padding:8px 10px;border-radius:7px;align-items:flex-start}
.pop .pi.on{background:var(--hs-active)}
.pop .pi b{display:block;font-size:13px}.pop .pi p{font-size:11.5px;color:var(--hs-t3)}
.rev{flex:1;display:flex;min-height:0}
.revl{width:340px;flex:none;border-right:1px solid var(--hs-l2);background:var(--hs-s1);overflow:hidden}
.revr{flex:1;min-width:0;padding:22px 28px;display:flex;flex-direction:column;overflow:hidden}
.bigq{font-size:17px;line-height:1.6;color:var(--hs-t1);border-left:3px solid var(--hs-violet);padding-left:16px;margin-top:14px}
.bigq u{text-decoration:none;box-shadow:inset 0 -3px 0 var(--hs-violet-line)}
.seed{margin-top:18px;border:1px solid var(--hs-l2);border-radius:10px;padding:12px 14px;background:var(--hs-s1)}
.seed .snip u{text-decoration:none;box-shadow:inset 0 -2px 0 var(--hs-violet-line)}
.rv{display:flex;gap:8px;padding:10px 14px;border-bottom:1px solid var(--hs-l2);align-items:flex-start}
.rv.on{background:var(--hs-s0);box-shadow:inset 3px 0 0 var(--hs-violet)}
.rv .st{flex:none;width:18px;height:18px;border-radius:50%;border:1.5px solid var(--hs-t5);display:flex;align-items:center;justify-content:center;margin-top:2px;color:var(--hs-s0)}
.rv .st.ok{background:var(--hs-green);border-color:var(--hs-green)}
.rv .st.no{background:var(--hs-t4);border-color:var(--hs-t4)}
</style>
"""


def set_item(k, n, note=None, edit=False, lift=False, lead=""):
    it = I[k]
    nt = note if note is not None else NOTES.get(k)
    if edit:
        nhtml = f'<div class="note">{icon("pencil", "sm")}<span class="nf edit">Open the talk with this<span class="caret" style="height:13px"></span></span></div>'
    elif nt:
        nhtml = f'<div class="note has">{icon("pencil", "sm")}<span class="nf">{nt}</span></div>'
    else:
        nhtml = f'<div class="note">{icon("pencil", "sm")}<span style="color:var(--hs-t5)">Add a note…</span></div>'
    r = hs_row(it)
    inner = r[r.index('<div class="body">'):r.rindex('</div>')]
    inner = inner[:inner.rindex('</div>')] + nhtml + '</div>'
    return (f'<div class="si{" lift" if lift else ""}"><span class="num">{n}</span><span class="g grip">{icon("grip")}</span>'
            f'{inner}{lead}</div>')


# ---------- A ----------

def tray():
    minis = "".join(f'<span class="mini"><span class="dot {I[k]["s"]}" style="width:6px;height:6px;border-radius:50%;flex:none"></span>'
                    f'{(I[k].get("title") or I[k].get("by") or "Tweet")[:26]}</span>' for k in ["repaved", "sword", "hack", "tgen", "weller"])
    return (f'<div class="tray">{icon("set")}<span class="nm">Metaphor talk · 7 items</span>{minis}<span class="small">+2</span>'
            f'<span style="margin-left:auto" class="hs-btn sm">Switch set ⌄</span><span class="hs-btn sm pri">Open set</span></div>')


def a1():
    def mark_in(it):
        d = dict(it); d["tag"] = '<span class="inset">✓ In set</span>'; return d
    def mark_add(it):
        d = dict(it); d["tag"] = '<span class="addset">+ Set ⌘S</span>'; return d
    rows = (hs_row(mark_in(I["repaved"])) + hs_row(mark_in(I["sword"])) + hs_row(mark_add(I["ships"]), on=True)
            + hs_row(mark_in(I["tgen"])) + hs_row(I["fork"]) + hs_row(mark_in(I["backrow"])) + hs_row(I["edtechie"]))
    pane = (f'<div class="hs-pane"><div class="p-title">Not ships in the night: Metaphor and simile as process</div>'
            f'<div class="p-sub"><span class="src w">Writing</span>Dominik Lukeš · 22 May 2018 · essay</div>'
            f'<div class="p-quote"><p>If you study <mark>metaphor</mark> in context, this will not surprise you. The blend is projected into another domain that is in a complex relationship to what precedes and what follows.</p></div>'
            f'<div style="margin-top:auto" class="actbar"><span class="act pri">{icon("plus")}Add to Metaphor talk <kbd>⌘S</kbd></span>'
            f'<span class="act">{icon("cite")}Copy quote + citation <kbd>⌘⇧C</kbd></span></div></div>')
    body = (hs_search("paths metaphor", f'<span class="hs-btn sm">Filters</span>') + hs_chips() +
            f'<div class="hs-main"><div class="hs-list">{rows}</div>{pane}</div>' + tray() + hs_foot("⌘S adds the selected result to Metaphor talk"))
    return hs_win("hs2a-1", "HS-2A", body)


def export_col(extra=""):
    return (f'<div class="setr"><div class="lab">Export this set</div>'
            f'<div class="xcard"><span class="ic">{icon("doc")}</span><div><b>WriteFlex piece</b><p>A Markdown piece: each quote with its citation line, notes as the paragraphs between.</p></div></div>'
            f'<div class="xcard"><span class="ic">{icon("slides")}</span><div><b>TalkWeaver quote slides</b><p>One quote slide per item, in this order, with the source line on each slide.</p></div></div>'
            f'<div class="xcard"><span class="ic">{icon("cite")}</span><div><b>Citation list</b><p>One line per item, in this order, each with its link.</p></div></div>'
            f'{extra}<div style="margin-top:auto" class="small">Saved as a file in the archive. Reopens with the same order and notes; every item still opens its source.</div></div>')


def set_head(name, sub, right=""):
    return (f'<div class="seth"><span class="small">← Search</span><span class="nm">{name}</span><span class="sub">{sub}</span>'
            f'<span style="margin-left:auto;display:flex;gap:6px">{right}</span></div>')


def a2():
    items = (set_item("repaved", 1, edit=True) + set_item("sword", 2) + set_item("hack", 3)
             + '<div class="dropline"></div>' + set_item("weller", 4, lift=True) + set_item("tgen", 5)
             + set_item("backrow", 6) + set_item("mitchell", 7))
    right = f'<span class="hs-btn sm">{icon("spark", "sm")} More like these…</span><span class="hs-btn sm">Export ⌄</span>'
    body = (f'<div class="setv"><div class="setl">{set_head("Metaphor talk", "7 items · 3 writing · 1 tweet · 3 highlights", right)}{items}</div>{export_col()}</div>'
            + hs_foot("Drag or ⌥↑ ⌥↓ to reorder · ⏎ opens the source · N edits the note",
                      "<span><kbd>⌥↑</kbd><kbd>⌥↓</kbd> move</span><span><kbd>N</kbd> note</span><span><kbd>⌫</kbd> remove</span><span><kbd>esc</kbd> back to search</span>"))
    return hs_win("hs2a-2", "HS-2A", body)


def pinrow(k, pinned, new=False, struck=False):
    it = dict(I[k])
    if new:
        it["tag"] = '<span class="newtag">New since sync</span>'
    pb = f'<span class="pinb{" on" if pinned else ""}">{icon("pin")}</span>'
    return hs_row(it, lead="", extra_cls="struck" if struck else "", one=True).replace('<div class="body">', pb + '<div class="body">', 1)


def a3():
    q = '<span class="chip" style="font-family:ui-monospace,Menlo,monospace;font-size:12px">metaphor -tweets after:2015</span>'
    head = set_head("Metaphor since 2015", q, '<span class="small" style="align-self:center">updates after every sync</span>')
    body_items = (f'<div class="gh">{icon("pin", "sm")} Pinned · 4<span class="r">always in the set</span></div>'
                  + pinrow("repaved", True) + pinrow("ships", True) + pinrow("backrow", True) + pinrow("sword", True)
                  + f'<div class="gh">Matching · not pinned · {ph()}<span class="r">in the set while they match</span></div>'
                  + pinrow("weller", False, new=True) + pinrow("anthro", False) + pinrow("oer", False)
                  + f'<div class="gh">Unpinned · 1<span class="r">kept out, even when it matches after an update</span></div>'
                  + pinrow("actors", False, struck=True))
    side = export_col('<div class="cite-box" style="margin-top:4px"><b>Last update</b><br>After this morning’s sync: '
                      f'{ph()} new matches. Pins and unpins are kept.</div>')
    body = (f'<div class="setv"><div class="setl">{head}{body_items}</div>{side}</div>'
            + hs_foot("P pins or unpins the selected item", "<span><kbd>P</kbd> pin / unpin</span><span><kbd>esc</kbd> back to search</span>"))
    return hs_win("hs2a-3", "HS-2A", body)


def sugg(k, like, why, state=""):
    it = I[k]
    r = hs_row(it)
    meta = r[r.index('<div class="meta1">'):r.index('</div>', r.index('<div class="meta1">')) + 6]
    title = f'<div class="rtitle">{it["title"]}</div>' if it.get("title") else ""
    if state == "done":
        btns = f'<div class="btns"><span class="vbtn ok">{icon("check", "sm")} Added as item 8</span></div>'
    elif state == "no":
        btns = '<div class="btns"><span class="vbtn">Rejected · will not be suggested again</span></div>'
    else:
        btns = f'<div class="btns"><span class="vbtn ok">{icon("check", "sm")} Add <kbd style="background:transparent;color:inherit;border-color:rgba(255,255,255,.4)">Y</kbd></span><span class="vbtn">{icon("x", "sm")} Not this <kbd>N</kbd></span></div>'
    return (f'<div class="sugg {state}">{meta}{title}<div class="why">{why}</div>'
            f'<div class="like">{icon("spark", "sm")} Similar to <b>{like}</b></div>{btns}</div>')


def a4():
    head = set_head("Metaphor talk", "8 items", f'<span class="hs-btn sm">Export ⌄</span>')
    items = ('<div class="gh">Seeds<span class="r">items 1–3 of 8</span></div>'
             + ''.join(f'<div class="row" style="padding:6px 18px"><div class="body"><div class="meta1"><span style="width:14px;color:var(--hs-t4)">{n}</span><span class="src {I[k]["s"]}">{SRC_LABEL[I[k]["s"]]}</span><span class="by">{I[k].get("title") or I[k].get("by")}</span></div></div></div>'
                       for n, k in ((1, "repaved"), (2, "sword"), (3, "hack"))))
    sg = (f'<div class="gh" style="background:var(--hs-violet-bg);color:var(--hs-violet)">{icon("spark", "sm")} More like items 1–3 · across writing and highlights'
          f'<span class="r">each joins only when you add it</span></div>'
          + sugg("oer", "Hacking a metaphor in five steps", "The <u>metaphors we choose to employ literally determine what we will see</u>, consider, and understand – as well as what we will not.", "done")
          + sugg("actors", "Repaved paths and generative metaphors", "There are many ways in which <u>the teacher’s experience is similar to that of an actor</u>.")
          + sugg("fruit", "Air &amp; Light &amp; Time &amp; Space (Helen Sword)", "We do not really understand one domain in terms of another through metaphor. <u>We ‘understand’ both domains in different ways.</u>")
          + sugg("gori", "Hacking a metaphor in five steps", "<u>Metaphors are a means through which human thought is structured.</u>", "no"))
    side = export_col()
    body = (f'<div class="setv"><div class="setl">{head}{items}{sg}</div>{side}</div>'
            + hs_foot("Y adds · N rejects · the underlined words are what made it similar", "<span><kbd>Y</kbd> add</span><span><kbd>N</kbd> not this</span><span><kbd>esc</kbd> stop</span>"))
    return hs_win("hs2a-4", "HS-2A", body)


# ---------- B ----------

def dcard(k, note=None, new=False, lift=False, edit=False, compact=False):
    it = I[k]
    nt = note if note is not None else NOTES.get(k)
    if edit:
        n = f'<div class="note" style="margin-top:6px"><span class="nf edit" style="display:block;border:1px solid var(--hs-blue);border-radius:6px;padding:2px 8px;font-size:12.5px">Open the talk with this<span class="caret" style="height:13px"></span></span></div>'
    elif nt:
        n = f'<div style="margin-top:5px;font-size:12.5px;color:var(--hs-t2)">{icon("pencil", "sm")} {nt}</div>'
    else:
        n = f'<div style="margin-top:5px;font-size:12px;color:var(--hs-t5)">{icon("pencil", "sm")} Add a note…</div>'
    if compact:
        n = ""
    r = hs_row(it, one=compact)
    inner = r[r.index('<div class="meta1">'):r.rindex('</div>')]
    inner = inner[:inner.rindex('</div>')]
    return (f'<div class="dc{" new" if new else ""}{" lift" if lift else ""}"><span class="grip" style="padding-top:1px">{icon("grip")}</span>'
            f'<div class="body">{inner}{n}</div></div>')


def drawer(cards, head_extra="", foot=None, pop=""):
    f = foot if foot is not None else (f'<span class="hs-btn sm">{icon("spark", "sm")} More like these</span>'
                                       f'<span style="margin-left:auto" class="hs-btn sm pri">Export ⌃</span>')
    return (f'<div class="drawer"><div class="dh">{icon("set")}<span class="nm">Metaphor talk</span><span class="small">7 items</span>'
            f'{head_extra}<span style="margin-left:auto" class="sel">Sets</span><span class="small">⌘\\ hides</span></div>'
            f'<div style="flex:1;overflow:hidden;padding-bottom:6px">{cards}</div><div class="dfoot">{f}</div>{pop}</div>')


def search_list(selected="ships", added=True):
    def tag(it, t):
        d = dict(it); d["tag"] = t; return d
    ins = '<span class="inset">✓ In set</span>'
    rows = (hs_row(tag(I["repaved"], ins)) + hs_row(tag(I["ships"], ins if added else ""), on=True)
            + hs_row(tag(I["sword"], ins)) + hs_row(tag(I["tgen"], ins)) + hs_row(I["fork"]) + hs_row(tag(I["backrow"], ins)) + hs_row(I["edtechie"]))
    return f'<div class="hs-list">{rows}</div>'


def b1():
    cards = (dcard("ships", new=True) + dcard("repaved") + dcard("sword") + dcard("hack") + dcard("tgen") + dcard("weller"))
    body = (hs_search("paths metaphor", f'<span class="hs-btn sm">Filters</span>') + hs_chips() +
            f'<div class="hs-main">{search_list()}{drawer(cards, "<span class=inset>+1 just added</span>")}</div>'
            + hs_foot("⌘S added “Not ships in the night” to Metaphor talk"))
    return hs_win("hs2b-1", "HS-2B", body)


def b2():
    cards = (dcard("repaved", edit=True) + '<div class="dropline" style="margin:4px 14px 0 24px"></div>' + dcard("weller", lift=True)
             + dcard("sword") + dcard("hack") + dcard("tgen") + dcard("backrow") + dcard("mitchell"))
    pop = (f'<div class="pop"><div class="lab" style="padding:6px 10px 2px">Export Metaphor talk</div>'
           f'<div class="pi on">{icon("doc")}<div><b>As a WriteFlex piece</b><p>Quotes with citation lines; notes become the paragraphs between.</p></div></div>'
           f'<div class="pi">{icon("slides")}<div><b>As TalkWeaver quote slides</b><p>One slide per item, source line on each.</p></div></div>'
           f'<div class="pi">{icon("cite")}<div><b>As a citation list</b><p>One line per item with its link, in this order.</p></div></div></div>')
    body = (hs_search("paths metaphor", f'<span class="hs-btn sm">Filters</span>') + hs_chips() +
            f'<div class="hs-main">{search_list()}{drawer(cards, pop=pop)}</div>'
            + hs_foot("Drag to reorder · N edits the note of the selected card", "<span><kbd>⌥↑</kbd><kbd>⌥↓</kbd> move</span><span><kbd>N</kbd> note</span><span><kbd>esc</kbd> back to results</span>"))
    return hs_win("hs2b-2", "HS-2B", body)


def dpin(k, pinned, new=False, struck=False):
    c = dcard(k, compact=True)
    pb = f'<span class="pinb{" on" if pinned else ""}" style="margin-left:auto">{icon("pin")}</span>'
    c = c.replace('<div class="body">', '<div class="body">', 1)
    c = c[:c.rindex('</div>')] + pb + '</div>'
    if new:
        c = c.replace('class="dc"', 'class="dc new"', 1)
    if struck:
        c = c.replace('class="dc"', 'class="dc struck" style="opacity:.55"', 1)
    return c


def b3():
    head = ('<div class="dh" style="flex-wrap:wrap">' + icon("saved") + '<span class="nm">Metaphor since 2015</span>'
            '<span class="chip" style="font-family:ui-monospace,Menlo,monospace;font-size:11.5px">metaphor -tweets after:2015</span>'
            '<span style="margin-left:auto" class="sel">Sets</span></div>')
    cards = (f'<div class="lab" style="padding:10px 14px 0">{icon("pin", "sm")} Pinned · 2</div>'
             + dpin("repaved", True) + dpin("ships", True)
             + f'<div class="lab" style="padding:12px 14px 0">Matching, not pinned · {ph()} · <span class="newtag">{ph()} new since this morning’s sync</span></div>'
             + dpin("weller", False, new=True) + dpin("anthro", False)
             + f'<div class="lab" style="padding:12px 14px 0">Unpinned · kept out after updates</div>' + dpin("actors", False, struck=True))
    dr = (f'<div class="drawer">{head}<div style="flex:1;overflow:hidden;padding-bottom:6px">{cards}</div>'
          f'<div class="dfoot"><span class="small">P pins · updates after every sync</span><span style="margin-left:auto" class="hs-btn sm pri">Export ⌃</span></div></div>')
    body = (hs_search("metaphor -tweets after:2015", '<span class="hs-btn sm">Saved as a set ✓</span>', caret=False) + hs_chips(active=("w", "h")) +
            f'<div class="hs-main"><div class="hs-list">{"".join(hs_row(I[k], on=(k=="weller")) for k in ["repaved","ships","backrow","sword","weller","anthro","oer"])}</div>{dr}</div>' + hs_foot("A saved search is a set that updates itself"))
    return hs_win("hs2b-3", "HS-2B", body)


def b4():
    def rv(k, st, on=False):
        it = I[k]
        ic = {"ok": icon("check", "sm"), "no": icon("x", "sm"), "": ""}[st]
        label = it.get("title") or f'{it.get("by", "")} · {it["meta"]}'
        return (f'<div class="rv{" on" if on else ""}"><span class="st {st}">{ic}</span><div style="min-width:0">'
                f'<div class="meta1"><span class="src {it["s"]}">{SRC_LABEL[it["s"]]}</span></div>'
                f'<div class="rtitle" style="font-size:13px;white-space:normal">{label}</div></div></div>')
    left = (f'<div class="revl"><div class="gh" style="background:var(--hs-violet-bg);color:var(--hs-violet)">{icon("spark", "sm")} 4 suggestions<span class="r">1 added · 1 rejected</span></div>'
            + rv("oer", "ok") + rv("actors", "", on=True) + rv("fruit", "") + rv("gori", "no") + '</div>')
    right = (f'<div class="revr"><div class="meta1"><span class="src w">Writing</span><span>2016-02-16 · essay</span></div>'
             f'<div class="p-title">If teachers are actors, is instructional design stage directions? Exploration of a metaphor.</div>'
             f'<div class="lab" style="margin-top:16px">The passage that made it similar</div>'
             f'<div class="bigq">There are many ways in which <u>the teacher’s experience is similar to that of an actor</u>. They play a role of a persuasive salesman trying to convince their audience and create a memorable experience for them.</div>'
             f'<div class="seed"><div class="lab">Similar to item 1 in the set</div><div class="meta1"><span class="src w">Writing</span><span>2016-06-23 · essay</span></div>'
             f'<div class="rtitle">Repaved paths and generative metaphors: Expressing human purposes with technology</div>'
             f'<div class="snip">Often, a poet will be led by rhythm and rhyme to uncover new possibilities of meaning. <u>New analogies or comparisons will inspire</u> a scientist to formulate an innovative hypothesis…</div></div>'
             f'<div style="margin-top:auto;display:flex;gap:8px"><span class="vbtn ok" style="padding:7px 14px;font-size:13px">{icon("check")} Add to Metaphor talk <kbd style="background:transparent;color:inherit;border-color:rgba(255,255,255,.4)">Y</kbd></span>'
             f'<span class="vbtn" style="padding:7px 14px;font-size:13px">{icon("x")} Not this <kbd>N</kbd></span><span class="small" style="margin-left:auto;align-self:center">↓ next suggestion</span></div></div>')
    body = (f'<div class="seth">{icon("spark")}<span class="nm" style="font-size:16px">More like these</span><span class="sub">for Metaphor talk · seeds: items 1, 2 and 3 · writing and highlights</span>'
            f'<span style="margin-left:auto" class="hs-btn sm">Done</span></div><div class="rev">{left}{right}</div>'
            + hs_foot("Nothing joins the set until you add it", "<span><kbd>Y</kbd> add</span><span><kbd>N</kbd> not this</span><span><kbd>↓</kbd> next</span><span><kbd>esc</kbd> done</span>"))
    return hs_win("hs2b-4", "HS-2B", body)


A = direction("HS-2A", "A tray below the results; the set opens as its own page",
              "A slim tray sits between the results and the status bar and names the active set. “Open set” turns the window into the set page: "
              "the ordered items on the left (70%), export on the right (30%). Saved-search sets and the agent’s suggestions use the same page.",
              state(1, "<b>The tray.</b> Results already in the set carry “✓ In set”; ⌘S adds the selected one.", a1())
              + state(2, "<b>The set page.</b> Item 4 is being dragged above item 5’s old place; item 1’s note is being typed. Three export targets on the right.", a2(),
                      "The one filled note is his real Readwise note on the Mitchell highlight. “Open the talk with this” is placeholder text for the note being typed.")
              + state(3, "<b>A saved search as a set.</b> Pinned items stay; matching items come and go with the query; an unpinned item stays out after updates.", a3(),
                      "“New since sync” on the Weller highlight shows the state; which items the next sync adds is not known.")
              + state(4, "<b>“More like these”.</b> Suggestions appear under the seeds, each with the passage that made it similar (underlined) and which item it resembles. One added, one rejected, two waiting.", a4()))

B = direction("HS-2B", "The set lives where the reading pane was",
              "The set is a drawer in the reading pane’s slot, so searching and collecting happen side by side. Reorder, notes and export all happen in the drawer. "
              "Suggestions get a separate review screen: one suggestion at a time, large, with the seed it resembles underneath.",
              state(1, "<b>The drawer.</b> ⌘S adds the selected result; the new card lands on top with an amber edge.", b1())
              + state(2, "<b>Reorder, notes, export.</b> A card is being dragged; the first card’s note is being typed; the Export menu is open.", b2())
              + state(3, "<b>A saved search as a set.</b> The same drawer, now showing pinned, matching and unpinned items.", b3())
              + state(4, "<b>Review “more like these”, one at a time.</b> The underline shows the words that made each passage similar; Y adds, N rejects.", b4()))

page("hs-2-sets.html", "Surface 2", "Sets: tray, saved searches and “more like these”",
     "Journey J2 A+B+C: collect a set for a talk from search results, reorder and annotate it, export it; let a saved search grow a set; let the agent suggest more items, each approved one by one.",
     "<span>Drawn at <b>1200 × 780</b></span><span>Every item is a real piece, tweet or Readwise highlight from the archive</span>"
     "<span>Lakoff &amp; Johnson from the journey text is not drawn: no highlight from <i>Metaphors We Live By</i> turned up in Readwise</span>",
     EXTRA_CSS + A + B,
     ["Should the set take over the window (HS-2A) or sit beside the results in the reading pane’s place (HS-2B)?",
      "Suggestions inline under the seeds (HS-2A) or a one-at-a-time review screen (HS-2B)?",
      "Where is the set file saved and under what name? The journey requires “saved as a file in the archive”; the path is not drawn.",
      "Are sets and saved searches one list (drawn so) or two?",
      "⌘S is “save” everywhere else on the Mac. Adding to the active set with ⌘S follows the journey; confirm.",
      "Should rejected suggestions be remembered per set or across all sets?"])
