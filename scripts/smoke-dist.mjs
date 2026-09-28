// Launch smoke for the built frontend (dist/): loads it in headless WebKit
// (the engine the macOS app runs in) with the Tauri IPC mocked, first with
// empty local settings (the default scope, the three-corpus quick finder),
// then in classic search, and fails if the app does not mount: an uncaught
// page error, or no search box. Also plays the launch events (index keeper,
// sync finished) so a crash on the first data update is caught too.
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
function tauriMock(scope) {
  localStorage.clear();
  if (scope) localStorage.setItem("searchScope", scope);
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
    search_query: { rows: [], has_more: false },
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

const CASES = [
  { name: "first launch (default scope: quick finder)", scope: null, box: 'input[aria-label="Search writing, tweets and highlights"]', also: '[data-testid="rail"]' },
  { name: "classic search remembered", scope: "highlights", box: "input", also: null },
];

const browser = await webkit.launch();
let failed = 0;
try {
  for (const c of CASES) {
    const page = await browser.newPage({ viewport: { width: 1200, height: 780 } });
    const errors = [];
    page.on("pageerror", (e) => errors.push(e.message));
    await page.addInitScript(tauriMock, c.scope);
    await page.goto(url);
    const mounted = await page.locator(c.box).first().waitFor({ state: "visible", timeout: 5000 }).then(() => true, () => false);
    for (const [event, payload] of LAUNCH_EVENTS) {
      await page.evaluate(([e, p]) => window.__smokeEmit(e, p), [event, payload]);
      await page.waitForTimeout(100);
    }
    const still = await page.locator(c.box).first().isVisible();
    const extra = c.also ? await page.locator(c.also).isVisible() : true;
    const ok = mounted && still && extra && errors.length === 0;
    console.log(`smoke: ${ok ? "ok  " : "FAIL"} ${c.name}`);
    if (!ok) {
      failed++;
      if (!mounted) console.log("       the search box never appeared");
      if (mounted && !still) console.log("       the window emptied after the launch events");
      if (!extra) console.log(`       ${c.also} missing`);
      errors.forEach((e) => console.log(`       page error: ${e}`));
    }
    await page.close();
  }
} finally {
  await browser.close();
  server.close();
}
process.exit(failed ? 1 : 0);
