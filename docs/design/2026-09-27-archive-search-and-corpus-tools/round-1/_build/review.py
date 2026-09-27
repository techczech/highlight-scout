from lib import OUT
import os
S = [
 ("Surface 1 of 5 · One search over three corpora (J1-B hotkey window)", "hs-1-search-three-corpora.html", [
  ("HS-1A","hs1a-1","The finder you know, with a corpus row","Today’s layout kept whole; corpus chips replace the colour row, source badge first, copy actions at the foot of the reading pane with ⌘⇧C as the dark button."),
  ("HS-1B","hs1b-1","Corpus rail, results grouped by corpus","A quiet left rail of corpora with real sizes, sets and recent searches; results grouped Writing / Tweets / Highlights; copy actions ride on the selected row and the pane previews the clipboard.")]),
 ("Surface 2 of 5 · Sets (J2 A+B+C)", "hs-2-sets.html", [
  ("HS-2A","hs2a-1","A tray below the results; the set opens as its own page","Tray names “Metaphor talk · 7 items”; the set page is 70% ordered items with notes, 30% export; saved searches and agent suggestions reuse the page."),
  ("HS-2B","hs2b-1","The set lives where the reading pane was","A drawer beside the results for collecting, reordering, notes and an Export menu; suggestions are reviewed one at a time on their own screen.")]),
 ("Surface 3 of 5 · Sync at launch (J3-B)", "hs-3-sync.html", [
  ("HS-3A","hs3a-1","Toast at the foot, red line above the status bar","Today’s mechanism finished: bottom toast with per-source counts, red failure line above the status bar, Settings → Sync as a table."),
  ("HS-3B","hs3b-1","Source lights in the status bar, the failure at the top","Per-source lights always visible; a summary card at top right; the red line under the search box; Settings as one card per source with history.")]),
 ("Surface 4 of 5 · The WriteFlex search panel (J1-A)", "wf-4-search-panel.html", [
  ("WF-4A","wf4a-1","Search as a sixth right-pane tab","Search joins the 340 px right pane; cards in WriteFlex type; ⏎ quote + citation, ⌘⏎ link; the pane stays open for the next quote."),
  ("WF-4B","wf4b-1","A wider search dock that previews the insert","A 460 px dock with a scan list, the passage in context and the exact block to be inserted; closes after the insert.")]),
 ("Surface 5 of 5 · Analyse in ArchiveScout ↗ (J7-C)", "hs-5-analyse-in-archivescout.html", [
  ("HS-5A","hs5a-1","A button beside the hit count","“Analyse in ArchiveScout ↗” in the lab’s blue on the count line it carries across (30 pieces · 64 matches)."),
  ("HS-5B","hs5b-1","A quiet link in the status bar, plus commands","A small “Analyse ↗” beside the count in the status bar and palette commands that open the lab on Concordance, Collocates or N-grams.")]),
]
secs = ""
for title, f, dirs in S:
    base = f[:-5]
    arts = ""
    for did, fid, name, desc in dirs:
        arts += (f'<article><a href="{f}#{did}"><img src="screens/{base}--light--{fid}.png" alt="{did} light"></a>'
                 f'<div class="m"><span class="badge">{did}</span><div><b>{name}</b><p>{desc}</p>'
                 f'<p><a href="{f}#{did}">Open light</a> · <a href="{f}?theme=dark#{did}">Open dark</a> · <a href="screens/{base}--dark--{fid}.png">dark PNG</a></p></div></div></article>')
    secs += f'<section><h2>{title}</h2>{arts}</section>'
html = f"""<!doctype html><html lang="en-GB"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Highlight Scout round 1 review</title><style>
:root{{--bg:#f4f4f5;--panel:#fff;--ink:#18181b;--ink2:#52525b;--line:#e4e4e7;--accent:#2563eb}}
@media (prefers-color-scheme: dark){{:root{{--bg:#18181b;--panel:#1f1f23;--ink:#f4f4f5;--ink2:#a1a1aa;--line:#34343a;--accent:#60a5fa}}}}
body{{margin:0;background:var(--bg);color:var(--ink);font:14px/1.5 -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}}
main{{max-width:1320px;margin:0 auto;padding:24px 16px}} h1{{font-size:22px;margin:0 0 4px}} h2{{font-size:16px;margin:28px 0 10px}}
section{{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,560px),1fr));gap:16px}} section h2{{grid-column:1/-1}}
article{{background:var(--panel);border:1px solid var(--line);border-radius:12px;overflow:hidden}} article img{{width:100%;display:block;border-bottom:1px solid var(--line)}}
.m{{display:flex;gap:12px;padding:12px 14px}} .m p{{margin:3px 0;color:var(--ink2);font-size:13px}} a{{color:var(--accent)}}
.badge{{flex:none;background:#1f8a4c;color:#fff;font-weight:800;font-size:20px;border-radius:8px;padding:6px 12px;height:fit-content;white-space:nowrap}}
.intro{{color:var(--ink2);max-width:860px}}</style></head><body><main>
<h1>Highlight Scout and the WriteFlex search panel: design round 1</h1>
<p class="intro">Five surfaces, two directions each. Highlight Scout is drawn at its real window (1200 × 780), WriteFlex at 1440 × 900, ArchiveScout at 1280 × 820. Every title, quote, tweet and count is real; grey pills stand for numbers not known yet. Each card shows the first state of a direction; the page holds every state and its open questions. Pick one direction per surface, or mix parts.</p>
{secs}</main></body></html>"""
open(os.path.join(OUT, "review.html"), "w").write(html)
print("ok")
