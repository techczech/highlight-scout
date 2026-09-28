// Launch smoke for the built frontend (dist/): loads it in headless WebKit
// (the engine the macOS app runs in) with the Tauri IPC mocked, and fails if
// the one search does not mount: an uncaught page error, or no search box.
// Cases: first launch (every corpus ticked: the corpus engine answers), a
// search with Highlights alone ticked (the highlight index answers, in the
// same window), and the Filters · Group popover opening. Also plays the
// launch events (index keeper, sync finished) so a crash on the first data
// update is caught too.
//
//   node scripts/smoke-dist.mjs            (after `bun run build`)
//
// Without a Playwright WebKit on the machine it prints SKIPPED and exits 0,
// unless SMOKE_REQUIRED=1 (then it fails). Install: bunx playwright install webkit
import fs from "node:fs";
import http from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const dist = path.join(root, "dist");
const required = process.env.SMOKE_REQUIRED === "1";

function skip(why) {
  if (required) {
    console.error(`smoke: FAILED to run: ${why}`);
    process.exit(1);
  }
  console.warn(`smoke: SKIPPED (${why})`);
  process.exit(0);
}

let webkit;
try {
  ({ webkit } = await import("playwright"));
} catch {
  skip("playwright is not installed");
}
if (!fs.existsSync(webkit.executablePath())) skip("no Playwright WebKit; run `bunx playwright install webkit`");
if (!fs.existsSync(path.join(dist, "index.html"))) {
  console.error("smoke: dist/index.html missing; run `bun run build` first");
  process.exit(1);
}

const TYPES = { ".js": "text/javascript", ".css": "text/css", ".html": "text/html", ".svg": "image/svg+xml", ".png": "image/png" };
const server = http.createServer((req, res) => {
  let p = path.join(dist, decodeURIComponent(req.url.split("?")[0]));
  if (!p.startsWith(dist) || !fs.existsSync(p) || fs.statSync(p).isDirectory()) p = path.join(dist, "index.html");
  res.setHeader("content-type", TYPES[path.extname(p)] ?? "application/octet-stream");
  res.end(fs.readFileSync(p));
});
await new Promise((r) => server.listen(0, "127.0.0.1", r));
const url = `http://127.0.0.1:${server.address().port}/`;

// The commands the window calls at launch, answered in the shapes the backend returns.
function tauriMock(view) {
  localStorage.clear();
  if (view) localStorage.setItem("search.view", JSON.stringify(view));
  const job = { phase: "current", corpora: [], builds: [], message: "Archive indexes are up to date" };
  const settings = {
    readwise_api_key: "", archive_path: "/tmp/archive", zotero_db_path: "", shortcut: "CmdOrCtrl+Alt+Shift+H",
    result_limit: 80, import_reminder_days: 0, sync_on_launch: true, sync_interval_hours: 6, autostart_enabled: false,
    ocr_on_import: false, r2_enabled: false, r2_account_id: "", r2_endpoint: "", r2_bucket: "", r2_prefix: "", r2_has_credentials: false,
  };
  const answers = {
    corpus_counts: [
      { corpus: "writing", docs: 1742, indexed: true, sources: {} },
      { corpus: "tweets", docs: 14892, indexed: true, sources: {} },
      { corpus: "highlights", docs: 15010, indexed: true, sources: { readwise: 4794, x: 9864, zotero: 352 } },
    ],
    corpus_index_refresh: job,
    corpus_index_status: { status: { schema_version: 1, corpora: [] }, job, running: false },
    get_sync_status: { running: false, last_report: null, sources: [] },
    get_stats: { highlights: 51935, works: 15010 },
    get_facets: { sources: ["readwise", "x", "zotero"], colors: [] },
    get_config: { archive_path: "/tmp/archive", has_api_key: true, shortcut: settings.shortcut, zotero_db_path: "" },
    get_settings: settings,
    get_import_log: [],
    qmd_available: false,
    search_query: {
      rows: [{
        highlight_id: "h1", work_id: "w1", slug: "newell", text: "The digital computer as a tool for constructing theories.", note: null,
        title: "Computer Simulation of Human Thinking", author: "Newell", authors: [], work_type: "article", source_system: "zotero",
        source_id: null, url: "https://www.jstor.org/stable/1708447", highlighted_at: "2021-05-01T00:00:00Z", tags: [], location: "2011",
        annotation_color: "red", annotation_type: null, format: "text", asset_path: null, citation: null, collections: [],
        zotero_link: null, relevance: null, snippet: "", ocr_text: null,
      }],
      has_more: false,
    },
    search_counts: { total: 1, sources: { zotero: 1 } },
    highlight_position: { pos: 1, total: 2, max_loc: 4400 },
    list_tags: [{ tag: "epistemology", count: 3 }],
    corpus_search: {
      body: {
        schema_version: 1, query: "metaphor", corpora: ["writing"], total_documents: 1, total_passages: 1,
        results: [{
          corpus: "writing", rel_path: "a.md", path: "/tmp/a.md", title: "Repaved paths", author: "Dominik Lukeš", date: "2016-06-23",
          date_display: "23 June 2016", public_url: null, score: 1, rank: 1, title_match: false, passage_count: 1,
          hits: [{ passage_id: "writing:a.md:3", line_start: 3, line_end: 3, line: 3, quote: "generative metaphor", score: 1, link: null }],
          citation: { markdown: "", plain: "" },
        }],
      },
      notes: [],
    },
    corpus_passage: {
      body: {
        cited: {
          schema_version: 1, passage_id: "writing:a.md:3", corpus: "writing", rel_path: "a.md", path: "/tmp/a.md", line_start: 3, line_end: 3,
          quote: "A generative metaphor reframes the problem.", title: "Repaved paths", author: "Dominik Lukeš", date: "2016-06-23",
          date_display: "23 June 2016", link: null, public_url: null, citation: { markdown: "> A generative metaphor\n>\n> — Dominik Lukeš", plain: "" },
        },
        html: "<p>A generative metaphor</p>", plain: "A generative metaphor", context_before: "The paragraph before.",
      },
      notes: [],
    },
    frontmost_other_app: null,
  };
  let next = 0;
  const listeners = {};
  window.__smokeEmit = (event, payload) =>
    (listeners[event] ?? []).forEach((h) => window[`_${h}`]?.({ event, id: 0, payload }));
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } },
    transformCallback: (cb) => { const id = ++next; window[`_${id}`] = cb; return id; },
    unregisterCallback: () => {},
    convertFileSrc: (p) => p,
    invoke: async (cmd, args) => {
      if (cmd === "plugin:event|listen") (listeners[args.event] ??= []).push(args.handler);
      if (cmd.startsWith("plugin:")) return ++next;
      return cmd in answers ? answers[cmd] : null;
    },
  };
}

const LAUNCH_EVENTS = [
  ["corpus:index", { phase: "building", corpora: ["writing"], builds: [], message: "Updating the archive index: writing (engine or registry changed)…" }],
  ["corpus:index", { phase: "built", corpora: ["writing"], builds: [{ corpus: "writing", reason: "engine or registry changed", report: { full: true, indexed: 1742 }, error: null }], message: "Archive index updated: writing rebuilt" }],
  ["sync:finished", {
    running: false,
    last_report: { seq: 1, trigger: "launch", started_at: "2026-01-01T00:00:00Z", finished_at: "2026-01-01T00:00:05Z", summary: "Added 2 highlights", results: [{ source: "readwise", label: "Readwise", added: 2, error: null, finished_at: "2026-01-01T00:00:05Z" }] },
    sources: [{ key: "readwise", label: "Readwise", configured: true, last_synced_at: "2026-01-01T00:00:05Z", last_error: null }],
  }],
];

const BOX = 'input[aria-label="Search writing, tweets and highlights"]';
const CASES = [
  { name: "first launch (every corpus ticked)", view: null, also: ['[data-testid="rail"]', '[data-testid="mode-switch"]', '[data-testid="filters-button"]', '[data-testid="welcome"]'] },
  { name: "a search over every corpus", view: null, type: "metaphor", also: ['[data-testid="archive-row"]', '[data-testid="pane-quote"]', '[data-testid="rail"] .n.hits', '[data-testid="copy-preview-toggle"]'] },
  { name: "highlights alone: the highlight index in the same window", view: { corpora: ["highlights"] }, type: "computer", also: ['[data-testid="highlight-row"]', '[data-testid="highlight-pane"]'] },
  { name: "the Filters · Group popover opens", view: null, click: '[data-testid="filters-button"]', also: ['[data-testid="filters-popover"]'] },
];

const browser = await webkit.launch();
let failed = 0;
try {
  for (const c of CASES) {
    const page = await browser.newPage({ viewport: { width: 1200, height: 780 } });
    const errors = [];
    page.on("pageerror", (e) => errors.push(e.message));
    await page.addInitScript(tauriMock, c.view);
    await page.goto(url);
    const mounted = await page.locator(BOX).first().waitFor({ state: "visible", timeout: 5000 }).then(() => true, () => false);
    for (const [event, payload] of LAUNCH_EVENTS) {
      await page.evaluate(([e, p]) => window.__smokeEmit(e, p), [event, payload]);
      await page.waitForTimeout(100);
    }
    if (mounted && c.type) await page.locator(BOX).first().fill(c.type);
    if (mounted && c.click) await page.locator(c.click).first().click();
    const missing = [];
    for (const sel of c.also) {
      const seen = await page.locator(sel).first().waitFor({ state: "visible", timeout: 3000 }).then(() => true, () => false);
      if (!seen) missing.push(sel);
    }
    const still = await page.locator(BOX).first().isVisible();
    const extra = missing.length === 0;
    const ok = mounted && still && extra && errors.length === 0;
    console.log(`smoke: ${ok ? "ok  " : "FAIL"} ${c.name}`);
    if (!ok) {
      failed++;
      if (!mounted) console.log("       the search box never appeared");
      if (mounted && !still) console.log("       the window emptied after the launch events");
      if (!extra) console.log(`       missing: ${missing.join(", ")}`);
      errors.forEach((e) => console.log(`       page error: ${e}`));
    }
    await page.close();
  }
} finally {
  await browser.close();
  server.close();
}
process.exit(failed ? 1 : 0);
