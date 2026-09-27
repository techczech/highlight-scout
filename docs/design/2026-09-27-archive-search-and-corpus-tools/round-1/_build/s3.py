from lib import *

TOAST = "Added 732 highlights · 181 saved tweets · 9 Zotero items"
FAIL = "Readwise: couldn’t read Readwise’s reply (nextPageCursor)"

EXTRA = """<style>
.tcard{position:absolute;right:16px;top:60px;width:330px;background:var(--hs-s0);border:1px solid var(--hs-l2);border-radius:12px;box-shadow:0 18px 44px rgba(0,0,0,.22);padding:12px 14px;z-index:5}
.tcard .h{display:flex;align-items:center;gap:8px;font-weight:650;font-size:13px}
.tcard .r{display:flex;align-items:center;gap:8px;font-size:12.5px;color:var(--hs-t2);padding:5px 0;border-top:1px solid var(--hs-l1)}
.tcard .r b{margin-left:auto;font-variant-numeric:tabular-nums}
.lights{display:flex;gap:10px;align-items:center}
.lights span{display:inline-flex;gap:5px;align-items:center}
.scard{border:1px solid var(--hs-l2);border-radius:10px;padding:10px 12px;margin-top:8px}
.scard .t{display:flex;align-items:center;gap:8px;font-weight:600;font-size:13px}
.scard .t .r{margin-left:auto;font-weight:400;font-size:12px;color:var(--hs-t3)}
.scard .hist{margin-top:6px;font-size:12px;color:var(--hs-t3);display:grid;grid-template-columns:120px 1fr;gap:2px 10px}
.scard.bad{border-color:var(--hs-red-line);background:var(--hs-red-bg)}
</style>"""


def home():
    return ('<div class="home"><div class="big">1,506 pieces · 14,892 tweets · 14,724 works</div>'
            '<div class="hint">cat OR dog · "exact phrase" · -exclude · prefix* · au:scott ty:books y:2023 · /\\bAI\\b/</div></div>')


def base(fid, did, inner, foot_left, top="", bottom="", overlay=""):
    body = (hs_search("", '<span class="hs-btn sm">Filters</span><span class="hs-btn sm">' + icon("gear") + '</span>', ghost="Search… writing, tweets and highlights")
            + top + hs_chips() + inner + bottom + hs_foot(foot_left) + overlay)
    return hs_win(fid, did, body)


# ---------- A ----------

def a1():
    t = (f'<div class="hs-toast">{icon("check")} {TOAST}<span class="x">·</span><span class="lk">Details</span></div>')
    return base("hs3a-1", "HS-3A", home() + t, "Synced at launch · this morning")


def redline(top=False):
    return (f'<div class="redline{" top" if top else ""}">{icon("alert")}<b>{FAIL}</b>'
            f'<span class="small" style="color:inherit;opacity:.8">stays here until a sync succeeds</span>'
            f'<span class="sp"><span class="b">{icon("refresh", "sm")} Retry</span><span class="b">Sync settings</span></span></div>')


def a2():
    return base("hs3a-2", "HS-3A", home(), "Last sync: Zotero and saved tweets fine, Readwise highlights failed", bottom=redline())


def settings_a():
    rows = (f'<tr><td><span class="sdot"></span> Readwise highlights</td><td>this morning, {ph()}</td><td class="ok">added 732</td></tr>'
            f'<tr><td><span class="sdot"></span> Readwise saved tweets</td><td>this morning, {ph()}</td><td class="ok">added 181</td></tr>'
            f'<tr><td><span class="sdot"></span> Zotero</td><td>this morning, {ph()}</td><td class="ok">added 9</td></tr>')
    rows_f = (f'<tr><td><span class="sdot bad"></span> Readwise highlights</td><td>this morning, {ph()}<div class="small">last try {ph()} failed</div></td>'
              f'<td class="bad">couldn’t read Readwise’s reply (nextPageCursor)</td></tr>'
              f'<tr><td><span class="sdot"></span> Readwise saved tweets</td><td>{ph()}</td><td class="ok">added {ph()}</td></tr>'
              f'<tr><td><span class="sdot"></span> Zotero</td><td>{ph()}</td><td class="ok">added {ph()}</td></tr>')
    def tbl(r):
        return f'<table class="sync"><tr><th>Source</th><th>Last synced</th><th>Last result</th></tr>{r}</table>'
    return (f'<div class="overlay"><div class="sheet"><div class="tabs"><span>Import</span><span class="on">Sync</span><span>Sources</span><span>Search &amp; view</span><span>Shortcuts</span><span>About</span></div>'
            f'<div class="chk"><i>✓</i>Sync when Highlight Scout opens</div>'
            f'<div class="chk" style="justify-content:space-between"><span>While open, sync again</span><span class="sel">Daily</span></div>'
            f'<div style="display:flex;align-items:center;margin-top:12px"><span class="lab" style="margin:0">After this morning’s sync</span><span class="hs-btn sm" style="margin-left:auto">{icon("refresh", "sm")} Sync now</span></div>{tbl(rows)}'
            f'<div class="lab" style="margin-top:16px">After a failed sync</div>{tbl(rows_f)}'
            f'<p class="small" style="margin-top:10px">A failed source keeps its red line in the main window until a later sync of that source succeeds.</p></div></div>')


def a3():
    return base("hs3a-3", "HS-3A", home(), "", overlay=settings_a())


# ---------- B ----------

def lights(bad=False):
    r = "bad" if bad else ""
    return (f'<span class="lights"><span><span class="sdot {r}"></span>Readwise</span><span><span class="sdot"></span>Saved tweets</span>'
            f'<span><span class="sdot"></span>Zotero</span></span>')


def b1():
    card = (f'<div class="tcard"><div class="h">{icon("refresh")} Synced at launch<span class="small" style="margin-left:auto">this morning</span></div>'
            f'<div class="r" style="margin-top:6px"><span class="sdot"></span>Readwise highlights<b>+732</b></div>'
            f'<div class="r"><span class="sdot"></span>Readwise saved tweets<b>+181</b></div>'
            f'<div class="r"><span class="sdot"></span>Zotero<b>+9</b></div>'
            f'<div class="small" style="margin-top:6px">{TOAST}. Fades in 6 s; click to keep.</div></div>')
    return base("hs3b-1", "HS-3B", home() + card, lights() + '<span style="margin-left:8px">synced this morning</span>')


def b2():
    return base("hs3b-2", "HS-3B", home(), lights(True) + '<span style="margin-left:8px;color:var(--hs-red)">Readwise failed</span>', top=redline(top=True))


def b3():
    def sc(name, ok, last, added, err=""):
        cls = "" if ok else " bad"
        dot = "" if ok else " bad"
        e = f'<div class="bad" style="font-size:12.5px;margin-top:4px">{err}</div>' if err else ""
        return (f'<div class="scard{cls}"><div class="t"><span class="sdot{dot}"></span>{name}<span class="r">{last}</span></div>{e}'
                f'<div class="hist"><span>this morning, {ph()}</span><span>{added}</span><span>{ph("w")}</span><span>{ph("l")}</span></div></div>')
    sheet = (f'<div class="overlay"><div class="sheet"><div class="tabs"><span>Import</span><span class="on">Sync</span><span>Sources</span><span>Search &amp; view</span><span>Shortcuts</span><span>About</span></div>'
             f'<div style="display:flex;gap:16px;align-items:center"><div class="chk"><i>✓</i>Sync at launch</div><div class="chk"><span>then</span><span class="sel">Daily</span></div>'
             f'<span class="hs-btn sm" style="margin-left:auto">{icon("refresh", "sm")} Sync all now</span></div>'
             + sc("Readwise highlights", False, f"last good: this morning · last try {ph()}", "added 732", "couldn’t read Readwise’s reply (nextPageCursor) · Retry")
             + sc("Readwise saved tweets", True, "this morning", "added 181")
             + sc("Zotero", True, "this morning", "added 9")
             + '<p class="small" style="margin-top:10px">Each card lists recent syncs, newest first. Grey rows are earlier syncs whose times and counts are not known yet.</p></div></div>')
    return base("hs3b-3", "HS-3B", home(), lights(True), overlay=sheet)


A = direction("HS-3A", "Toast at the foot, red line above the status bar",
              "The app as it works today, finished: a dark toast at the bottom centre after the launch sync, a red line above the status bar when a source fails, "
              "and the Settings → Sync tab with a table of sources, last-synced times and the last result.",
              state(1, "<b>Launch.</b> The sync runs silently; the toast names the count added per source.", a1())
              + state(2, "<b>A later launch where Readwise fails.</b> The red line names the source and the error, with Retry. It stays until a Readwise sync succeeds.", a2(),
                      "Drawn as a separate launch: the success toast and the failure cannot both come from one sync of Readwise.")
              + state(3, "<b>Settings → Sync.</b> Last-synced time and last result per source; the failed case shown below the good one.", a3()))

B = direction("HS-3B", "Source lights in the status bar, the failure at the top",
              "Three small lights in the status bar stay visible all the time, one per source. The launch summary is a card at the top right that lists each source. "
              "A failure puts the red line at the top, under the search box, where the eye starts, and turns its light red. Settings shows one card per source with its recent syncs.",
              state(1, "<b>Launch.</b> A per-source card at the top right; the lights in the status bar are all green.", b1())
              + state(2, "<b>Readwise fails.</b> The red line sits under the search box, the Readwise light turns red.", b2())
              + state(3, "<b>Settings → Sync.</b> One card per source: last good sync, last try, and recent history.", b3()))

page("hs-3-sync.html", "Surface 3", "Sync at launch: toast, failure line, Settings → Sync",
     "Journey J3-B: sync runs silently when the app opens; a toast reports the count added per source, and a failed source keeps a red line naming the source and the error until a later sync succeeds.",
     "<span>Drawn at <b>1200 × 780</b></span><span>Counts from this morning’s sync: <b>732</b> Readwise highlights · <b>181</b> saved tweets · <b>9</b> Zotero items</span>"
     "<span>Error text is the real 0.5.6 failure</span><span>Exact sync times are grey: not known</span>",
     EXTRA + A + B,
     ["Where does the failure line belong: above the status bar, next to where the toast appeared (HS-3A), or at the top under the search box (HS-3B)?",
      "Does a permanent status-bar light per source (HS-3B) add useful reassurance or clutter?",
      "Saved tweets come through Readwise too. When Readwise highlights fail, should saved tweets be reported as failed as well? Drawn as independent.",
      "Should the toast stay until clicked, or fade (HS-3B says 6 s)? The attention rule says pull surfaces are missed, but this is a success message.",
      "Should a red line also go to Telegram (J3-C was not picked), or does the in-app line suffice?"])
