import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { openUrl, openPath } from "@tauri-apps/plugin-opener";
import { SearchBar } from "./components/SearchBar";
import { Toolbar, ScopeDropdown } from "./components/Toolbar";
import { ResultsList } from "./components/ResultsList";
import { ReadingPane } from "./components/ReadingPane";
import { TagPicker } from "./components/TagPicker";
import { WorkView } from "./components/WorkView";
import { SettingsPanel } from "./components/SettingsPanel";
import { CommandPalette } from "./components/CommandPalette";
import { ImportLogPanel } from "./components/ImportLogPanel";
import { CsvMappingPanel } from "./components/CsvMappingPanel";
import type { ImportAction } from "./components/Toolbar";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import {
  searchQuery,
  semanticSearch,
  qmdReindex,
  qmdAvailable,
  runImport,
  runReadwiseSeed,
  importReadwiseTweets,
  runZoteroImport,
  importKindle,
  importJson,
  importX,
  exportJson,
  getStats,
  getFacets,
  getConfig,
  getSettings,
  getImportLog,
  highlightPosition,
} from "./lib/api";
import { buildSearchQuery, parseSearch } from "./lib/query";
import { groupRows, flattenSections } from "./lib/grouping";
import { copyText } from "./lib/clipboard";
import { openWorkWindow, openRelatedWindow } from "./lib/window";
import { markdownQuote, workMarkdownPath } from "./lib/format";
import { comboMap, eventToCombo, type CommandId } from "./lib/keybindings";
import { resolveColor } from "./types";
import { APP_VERSION } from "./version";
import * as persist from "./lib/persist";
import type {
  SearchResult, Stats, Config, Facets, SearchMode, SortMode, GroupMode, Density, WorkPosition,
} from "./types";

const DEBOUNCE_MS = 130;

export default function App() {
  const [query, setQuery] = useState("");
  const [scope, setScope] = useState<string>(() => persist.loadScope());
  const [color, setColor] = useState<string | null>(null);
  const [sort, setSort] = useState<SortMode>(() => persist.load("sort", "matches", ["matches", "recent", "oldest"]));
  const [group, setGroup] = useState<GroupMode>(() => persist.load("group", "work", ["work", "author", "date", "tag", "none"]));
  const [subgroup, setSubgroup] = useState<GroupMode>(() => persist.load("subgroup", "none", ["work", "author", "date", "tag", "none"]));
  const [density, setDensity] = useState<Density>(() => persist.load("density", "comfortable", ["minimal", "compact", "comfortable", "full"]));
  const [mode, setMode] = useState<SearchMode>("keyword");
  const [partial, setPartial] = useState<boolean>(() => persist.load("partial", "no", ["no", "yes"]) === "yes");
  const [showPane, setShowPane] = useState(true);

  const [rows, setRows] = useState<SearchResult[]>([]);
  const [page, setPage] = useState(0);
  const [hasMore, setHasMore] = useState(false);
  const [loading, setLoading] = useState(false);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [position, setPosition] = useState<WorkPosition | null>(null);

  const [stats, setStats] = useState<Stats | null>(null);
  const [facets, setFacets] = useState<Facets | null>(null);
  const [config, setConfig] = useState<Config | null>(null);
  const [pageSize, setPageSize] = useState(80);

  const [importing, setImporting] = useState(false);
  const [status, setStatus] = useState("");
  const [progress, setProgress] = useState<{ current: number; total: number } | null>(null);
  const [toast, setToast] = useState("");

  const [overlay, setOverlay] = useState<null | "tags" | "settings" | "palette" | "importlog">(null);
  const [dataVersion, setDataVersion] = useState(0);
  const [workView, setWorkView] = useState<SearchResult | null>(null);
  const [csvPath, setCsvPath] = useState<string | null>(null);
  const [bindingsVersion, setBindingsVersion] = useState(0);
  const [qmdOk, setQmdOk] = useState(true);

  const inputRef = useRef<HTMLInputElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const reqRef = useRef(0);

  const terms = useMemo(() => parseSearch(query).positive_terms, [query]);
  const sections = useMemo(() => groupRows(rows, group, subgroup, sort), [rows, group, subgroup, sort]);
  const visualRows = useMemo(() => flattenSections(sections, rows), [sections, rows]);
  const activeRow = useMemo(
    () => rows.find((r) => r.highlight_id === activeId) ?? null,
    [rows, activeId]
  );

  // Keep the selection on the first *visible* (grouped) row when it is empty or
  // has fallen out of the current result set — so a new search highlights the
  // top row, not a mid-list FTS match.
  useEffect(() => {
    if (visualRows.length === 0) {
      if (activeId !== null) setActiveId(null);
      return;
    }
    if (!activeId || !visualRows.some((r) => r.highlight_id === activeId)) {
      setActiveId(visualRows[0].highlight_id);
    }
  }, [visualRows, activeId]);

  const showToast = useCallback((msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(""), 1800);
  }, []);

  const refreshMeta = useCallback(() => {
    getStats().then(setStats).catch(() => {});
    getFacets().then(setFacets).catch(() => {});
  }, []);

  useEffect(() => {
    refreshMeta();
    getConfig().then(setConfig).catch(() => {});
    getSettings().then((s) => setPageSize(s.result_limit || 80)).catch(() => {});
    qmdAvailable().then(setQmdOk).catch(() => setQmdOk(false));
  }, [refreshMeta]);

  // Optional import reminder (Settings → Import): nudge on launch if it's been
  // longer than `import_reminder_days` since the last import. 0 = off.
  useEffect(() => {
    getSettings()
      .then((s) => {
        const days = s.import_reminder_days || 0;
        if (days <= 0) return;
        getImportLog()
          .then((log) => {
            const last = log.reduce((mx, e) => Math.max(mx, Date.parse(e.timestamp) || 0), 0);
            if (Date.now() - last > days * 86_400_000) {
              setToast(`📌 It's been over ${days} day${days === 1 ? "" : "s"} since your last import — time to sync.`);
              window.setTimeout(() => setToast(""), 9000);
            }
          })
          .catch(() => {});
      })
      .catch(() => {});
  }, []);

  useEffect(() => persist.save("sort", sort), [sort]);
  useEffect(() => persist.save("group", group), [group]);
  useEffect(() => persist.save("subgroup", subgroup), [subgroup]);
  useEffect(() => persist.save("density", density), [density]);
  useEffect(() => persist.save("partial", partial ? "yes" : "no"), [partial]);
  useEffect(() => persist.saveScope(scope), [scope]);

  // Refocus search box + auto-refresh counts when shown via the global hotkey.
  useEffect(() => {
    const un = getCurrentWindow().onFocusChanged(({ payload }) => {
      if (payload) {
        inputRef.current?.focus();
        inputRef.current?.select();
        refreshMeta();
      }
    });
    return () => { un.then((f) => f()); };
  }, []);

  // Import progress events (structured: message + current/total).
  useEffect(() => {
    const a = listen<{ message: string; current: number; total: number }>("import:progress", (e) => {
      setStatus(e.payload.message);
      setProgress(e.payload.total > 0 ? { current: e.payload.current, total: e.payload.total } : null);
    });
    const b = listen<{ message: string }>("import:complete", (e) => {
      setStatus(e.payload.message);
      setImporting(false);
      setProgress(null);
      refreshMeta();
      setDataVersion((v) => v + 1); // auto-refresh: re-run the current search
    });
    return () => { a.then((f) => f()); b.then((f) => f()); };
  }, [refreshMeta]);

  const runSemantic = useCallback(async () => {
    if (!query.trim()) return;
    const reqId = ++reqRef.current;
    setLoading(true);
    setStatus("Semantic search (QMD)…");
    try {
      const r = await semanticSearch(query);
      if (reqId !== reqRef.current) return;
      setRows(r);
      setHasMore(false);
      setStatus(r.length ? "" : "No semantic matches — if empty, run Import ▾ → Rebuild semantic index");
    } catch (e) {
      if (reqId === reqRef.current) {
        setStatus(`Semantic search failed: ${e instanceof Error ? e.message : String(e)}`);
        setRows([]);
      }
    } finally {
      if (reqId === reqRef.current) setLoading(false);
    }
  }, [query]);

  const runSearch = useCallback(
    async (nextPage: number, append: boolean) => {
      if (mode === "semantic") {
        // Semantic runs on demand (Enter) — it is slower; don't fire per keystroke.
        return;
      }
      if (!query.trim() && !scope && !color) {
        setRows([]);
        setHasMore(false);
        return;
      }
      const reqId = ++reqRef.current;
      setLoading(true);
      try {
        const payload = buildSearchQuery({
          raw: query, scope, source: null, color, sort, mode, partial, page: nextPage, pageSize,
        });
        const result = await searchQuery(payload);
        if (reqId !== reqRef.current) return;
        setHasMore(result.has_more);
        setRows((prev) => (append ? [...prev, ...result.rows] : result.rows));
        if (!append) {
          // Clear selection; an effect re-selects the first *visible* row once
          // grouping is applied, so the highlight + scroll land at the top.
          setActiveId(null);
          setPosition(null);
        }
      } catch (e) {
        if (reqId === reqRef.current) {
          console.error(e);
          setRows([]);
        }
      } finally {
        if (reqId === reqRef.current) setLoading(false);
      }
    },
    [query, scope, color, sort, mode, partial, pageSize]
  );

  // Re-run from page 0 when query/filters/sort change (debounced). Semantic
  // mode does not auto-run (it is slower) — it clears and waits for Enter.
  useEffect(() => {
    if (mode === "semantic") {
      setRows([]);
      setHasMore(false);
      return;
    }
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => {
      setPage(0);
      runSearch(0, false);
    }, DEBOUNCE_MS);
    return () => { if (debounceRef.current) clearTimeout(debounceRef.current); };
  }, [query, scope, color, sort, mode, partial, dataVersion, runSearch]);

  const loadMore = useCallback(() => {
    if (loading || !hasMore) return;
    const next = page + 1;
    setPage(next);
    runSearch(next, true);
  }, [loading, hasMore, page, runSearch]);

  // Lazy position-in-work for the active row.
  useEffect(() => {
    setPosition(null);
    const r = activeRow;
    if (!r || !r.location || (r.work_type || "").toLowerCase().startsWith("tweet")) return;
    let cancelled = false;
    highlightPosition(r.work_id, r.location)
      .then((p) => { if (!cancelled) setPosition(p); })
      .catch(() => {});
    return () => { cancelled = true; };
  }, [activeRow]);

  const move = (delta: number) => {
    if (visualRows.length === 0) return;
    const idx = visualRows.findIndex((r) => r.highlight_id === activeId);
    const next = Math.max(0, Math.min(visualRows.length - 1, (idx < 0 ? 0 : idx) + delta));
    setActiveId(visualRows[next].highlight_id);
  };

  const copyHighlight = async () => {
    if (activeRow) { await copyText(activeRow.text); showToast("Copied highlight"); }
  };
  const copyMarkdown = async () => {
    if (activeRow) { await copyText(markdownQuote(activeRow)); showToast("Copied as Markdown"); }
  };
  const copyCitationCmd = async () => {
    if (activeRow?.citation) { await copyText(activeRow.citation); showToast("Citation copied"); }
  };
  const openSource = () => {
    if (activeRow?.zotero_link) openUrl(activeRow.zotero_link);
    else if (activeRow?.url) openUrl(activeRow.url);
  };
  const openWorkMd = () => {
    if (activeRow && config) {
      openPath(workMarkdownPath(config.archive_path, activeRow.slug)).catch(() => showToast("Markdown not found"));
    }
  };
  const openWorkWin = () => {
    if (activeRow) openWorkWindow(activeRow.work_id, activeRow.title).catch(() => showToast("Could not open window"));
  };

  function cycle<T>(list: T[], current: T): T {
    const i = list.indexOf(current);
    return list[(i + 1) % list.length];
  }

  const pickTag = (tag: string) => {
    setQuery((c) => (c ? `${c} tag:"${tag}"` : `tag:"${tag}"`));
    setOverlay(null);
    inputRef.current?.focus();
  };

  const doImport = async (which: ImportAction) => {
    // Non-import actions and file pickers first.
    if (which === "log") { setOverlay("importlog"); return; }
    if (which === "csv") {
      const f = await openDialog({ filters: [{ name: "CSV", extensions: ["csv", "tsv", "txt"] }] });
      if (typeof f === "string") setCsvPath(f);
      return;
    }
    if (which === "kindle") {
      const f = await openDialog({ filters: [{ name: "Kindle clippings", extensions: ["txt"] }] });
      if (typeof f !== "string") return;
      await withImport("Reading Kindle clippings…", () => importKindle(f));
      return;
    }
    if (which === "json") {
      const f = await openDialog({ filters: [{ name: "JSON", extensions: ["json"] }] });
      if (typeof f !== "string") return;
      await withImport("Reading JSON…", () => importJson(f));
      return;
    }
    if (which === "x") {
      const f = await openDialog({ filters: [{ name: "Saved tweets", extensions: ["jsonl", "json"] }] });
      if (typeof f !== "string") return;
      await withImport("Reading saved tweets…", () => importX(f));
      return;
    }
    if (which === "export-json") {
      const f = await saveDialog({ defaultPath: "highlight-scout-export.json", filters: [{ name: "JSON", extensions: ["json"] }] });
      if (typeof f !== "string") return;
      try {
        const n = await exportJson(f);
        showToast(`Exported ${n.toLocaleString()} highlights`);
      } catch (e) {
        showToast(`Export failed: ${e instanceof Error ? e.message : String(e)}`);
      }
      return;
    }

    if (which === "readwise-tweets") {
      await withImport("Importing saved tweets from Readwise…", () => importReadwiseTweets());
      return;
    }

    const label =
      which === "readwise" ? "Updating from Readwise…"
      : which === "readwise-seed" ? "Seeding from Readwise archive…"
      : which === "qmd-reindex" ? "Rebuilding semantic index…"
      : "Starting Zotero import…";
    await withImport(label, () =>
      which === "readwise" ? runImport()
      : which === "readwise-seed" ? runReadwiseSeed()
      : which === "qmd-reindex" ? qmdReindex()
      : runZoteroImport()
    );
  };

  // Manual refresh: reload counts/facets and re-run the current search.
  const manualRefresh = () => {
    refreshMeta();
    if (mode === "semantic") runSemantic();
    else setDataVersion((v) => v + 1);
    showToast("Refreshed");
  };

  // Run an import call with the busy flag + status, surfacing errors.
  const withImport = async (label: string, fn: () => Promise<unknown>) => {
    setImporting(true);
    setStatus(label);
    try {
      await fn();
    } catch (e) {
      setStatus(`Failed: ${e instanceof Error ? e.message : String(e)}`);
      setImporting(false);
    }
  };

  const commands = useMemo<Record<CommandId, () => void>>(() => ({
    focusSearch: () => { inputRef.current?.focus(); inputRef.current?.select(); },
    nextResult: () => move(1),
    prevResult: () => move(-1),
    openSource,
    copyHighlight,
    copyMarkdown,
    copyCitation: copyCitationCmd,
    openWorkView: () => { if (activeRow) setWorkView(activeRow); },
    openWorkWindow: openWorkWin,
    openWorkMarkdown: openWorkMd,
    findRelated: () => { if (activeRow) openRelatedWindow(activeRow.highlight_id); },
    togglePane: () => setShowPane((s) => !s),
    cycleSort: () => setSort((s) => cycle<SortMode>(["matches", "recent", "oldest"], s)),
    cycleGroup: () => setGroup((g) => cycle<GroupMode>(["work", "author", "date", "tag", "none"], g)),
    cycleDensity: () => setDensity((d) => cycle<Density>(["minimal", "compact", "comfortable", "full"], d)),
    openTags: () => setOverlay("tags"),
    openPalette: () => setOverlay("palette"),
    openHelp: () => setOverlay("palette"),
    openSettings: () => setOverlay("settings"),
    importUpdate: () => doImport("readwise"),
    importSeed: () => doImport("readwise-seed"),
    importZotero: () => doImport("zotero"),
    clearColor: () => setColor(null),
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }), [activeRow, config, visualRows, activeId]);

  // Recomputed when the user remaps shortcuts (bindingsVersion bumps).
  const keymap = useMemo(() => comboMap(), [bindingsVersion]);

  // App-wide keyboard handling on a GLOBAL listener (a div onKeyDown only fires
  // when focus is inside it — unreliable after the window shows). A ref keeps
  // the latest closures without re-binding the listener.
  const handleKeyRef = useRef<(e: KeyboardEvent) => void>(() => {});
  handleKeyRef.current = (e: KeyboardEvent) => {
    const target = e.target as HTMLElement | null;
    const inEditable =
      !!target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable);

    if (e.key === "Escape") {
      if (workView) setWorkView(null);
      else if (overlay) setOverlay(null);
      else if (query) setQuery("");
      else getCurrentWindow().hide();
      return;
    }
    // Overlays manage their own keys (filters, capture fields, nav).
    if (overlay) return;

    // In semantic mode, Enter runs the (slower) QMD search.
    if (mode === "semantic" && e.key === "Enter" && inEditable) {
      e.preventDefault();
      runSemantic();
      return;
    }

    // "?" opens the palette unless typing into a non-empty query.
    if (e.key === "?" && !(inEditable && query)) {
      e.preventDefault();
      setOverlay("palette");
      return;
    }

    const combo = eventToCombo(e);
    if (!combo) return;
    const cmd = keymap[combo];
    if (!cmd) return;
    if (cmd === "copyHighlight" && window.getSelection()?.toString()) return;
    e.preventDefault();
    commands[cmd]?.();
  };

  useEffect(() => {
    const h = (e: KeyboardEvent) => handleKeyRef.current(e);
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, []);

  const runCommand = (id: CommandId) => {
    setOverlay(null);
    commands[id]?.();
  };

  const total = stats ? `${stats.highlights.toLocaleString()} highlights · ${stats.works.toLocaleString()} works` : "";

  return (
    <div className="relative flex h-screen flex-col overflow-hidden bg-white text-zinc-900">
      <div className="flex items-center gap-2 border-b border-zinc-200 pr-2">
        <div className="flex-1">
          <SearchBar
            ref={inputRef}
            value={query}
            onChange={setQuery}
            isSearching={loading}
            placeholder={`Search… expert -novice, "exact phrase", au:scott ty:books /regex/`}
          />
        </div>
        <ScopeDropdown value={scope} onChange={setScope} />
        <button
          onClick={manualRefresh}
          title="Refresh — re-run the search and reload counts"
          className="shrink-0 rounded-lg border border-zinc-200 px-3 py-1.5 text-sm text-zinc-600 hover:bg-zinc-100"
        >
          ⟳ Refresh
        </button>
        <button
          onClick={() => setOverlay("settings")}
          title="Settings & import (⌘,)"
          className="shrink-0 rounded-lg border border-zinc-200 px-3 py-1.5 text-sm text-zinc-600 hover:bg-zinc-100"
        >
          {importing ? "⚙ Working…" : "⚙ Settings"}
        </button>
      </div>

      <Toolbar
        sort={sort} group={group} subgroup={subgroup} mode={mode} density={density} partial={partial} showPane={showPane}
        onSort={setSort} onGroup={setGroup} onSubgroup={setSubgroup} onMode={setMode} onDensity={setDensity} onPartial={setPartial}
        onTogglePane={() => setShowPane((s) => !s)}
        onOpenTags={() => setOverlay("tags")}
      />

      {facets && facets.colors.length > 0 && (
        <div className="flex items-center gap-1.5 border-b border-zinc-100 bg-zinc-50 px-3 py-1">
          <span className="mr-1 text-xs text-zinc-400">Colour</span>
          {facets.colors.slice(0, 14).map((c) => (
            <button
              key={c}
              title={c}
              onClick={() => setColor(color === c ? null : c)}
              className={`h-4 w-4 shrink-0 rounded-full border ${color === c ? "scale-125 border-zinc-700" : "border-zinc-300"}`}
              style={{ backgroundColor: resolveColor(c) ?? "#fff" }}
            />
          ))}
          {color && (
            <button onClick={() => setColor(null)} className="ml-1 text-xs text-zinc-400 hover:text-zinc-600">clear</button>
          )}
        </div>
      )}

      {mode === "semantic" && !qmdOk && (
        <div className="border-b border-amber-200 bg-amber-50 px-4 py-2 text-xs text-amber-800">
          Semantic search needs <strong>QMD</strong> installed (a local search engine). Keyword search works without it.{" "}
          <button onClick={() => openUrl("https://www.npmjs.com/package/@tobilu/qmd")} className="underline">
            Get QMD ↗
          </button>
        </div>
      )}

      <div className="flex min-h-0 flex-1">
        <div className={`flex min-w-0 flex-col ${showPane ? "w-[46%] border-r border-zinc-100" : "flex-1"}`}>
          {visualRows.length === 0 ? (
            <div className="flex flex-1 flex-col items-center justify-center gap-3 px-6 text-center text-zinc-400">
              {!query && !scope && !color ? (
                stats?.highlights === 0 ? (
                  <>
                    <p className="text-base text-zinc-500">No highlights yet.</p>
                    <button
                      onClick={() => setOverlay("settings")}
                      className="rounded-lg bg-amber-400 px-5 py-2.5 text-sm font-semibold text-white hover:bg-amber-500"
                    >
                      Import highlights →
                    </button>
                    <p className="text-xs text-zinc-400">CSV, Kindle, JSON, Readwise or Zotero — no account required for files.</p>
                  </>
                ) : (
                  <>
                    {total && <p className="text-sm">{total}</p>}
                    <p className="text-xs text-zinc-300">
                      cat OR dog · "exact phrase" · -exclude · prefix* · au:scott ty:books y:2023 · /\bAI\b/
                    </p>
                  </>
                )
              ) : mode === "semantic" && !loading ? (
                <p className="text-sm">
                  Press <kbd className="rounded bg-zinc-100 px-1">↵</kbd> to search semantically for “{query}”
                </p>
              ) : (
                <p className="text-sm">{loading ? "Searching…" : `No results for "${query}"`}</p>
              )}
            </div>
          ) : (
            <ResultsList
              rows={rows}
              sections={sections}
              density={density}
              terms={terms}
              semantic={mode === "semantic"}
              showPane={showPane}
              activeId={activeId}
              onActivate={setActiveId}
              onOpenDetail={(id) => { const r = rows.find((x) => x.highlight_id === id); if (r) setWorkView(r); }}
              onScrollEnd={loadMore}
            />
          )}
        </div>
        {showPane && (
          <div className="min-w-0 flex-1">
            <ReadingPane row={activeRow} terms={terms} position={position} onShowWork={setWorkView} onToast={showToast} />
          </div>
        )}
      </div>

      {progress && (
        <div className="h-1 w-full bg-zinc-100">
          <div
            className="h-full bg-blue-400 transition-all"
            style={{ width: `${Math.min(100, Math.round((progress.current / Math.max(1, progress.total)) * 100))}%` }}
          />
        </div>
      )}

      <div className="flex items-center justify-between border-t border-zinc-100 bg-zinc-50 px-4 py-1.5 text-xs text-zinc-400">
        <span className="truncate">
          {rows.length > 0 ? `${rows.length} shown${hasMore ? "+" : ""}` : total}
          {status && (
            <span className={importing ? "text-blue-500" : "text-zinc-500"}>
              {" · "}{status}
              {progress && ` (${Math.round((progress.current / Math.max(1, progress.total)) * 100)}%)`}
            </span>
          )}
        </span>
        <span className="flex shrink-0 items-center gap-2 text-zinc-300">
          <span>↑↓ nav · ↵ source · ⌘C copy · ⌘⇧L work · ⌘⇧P pane · esc</span>
          <button onClick={() => setOverlay("settings")} className="text-zinc-400 hover:text-zinc-600" title="Version & release notes">
            v{APP_VERSION}
          </button>
        </span>
      </div>

      {toast && (
        <div className="absolute bottom-10 left-1/2 -translate-x-1/2 rounded bg-zinc-800 px-3 py-1.5 text-xs text-white shadow-lg">
          {toast}
        </div>
      )}

      {overlay === "palette" && <CommandPalette onRun={runCommand} onClose={() => setOverlay(null)} />}
      {overlay === "importlog" && <ImportLogPanel onClose={() => setOverlay(null)} />}
      {csvPath && (
        <CsvMappingPanel
          path={csvPath}
          onClose={() => setCsvPath(null)}
          onImported={(s) => {
            setCsvPath(null);
            setImporting(false);
            setStatus(s.message);
            refreshMeta();
          }}
        />
      )}
      {overlay === "tags" && <TagPicker onPick={pickTag} onClose={() => setOverlay(null)} />}
      {overlay === "settings" && (
        <SettingsPanel
          onClose={() => { setOverlay(null); setBindingsVersion((v) => v + 1); }}
          onImport={(a) => { setOverlay(null); doImport(a); }}
          onSaved={(shortcutChanged) => {
            setOverlay(null);
            setBindingsVersion((v) => v + 1);
            getConfig().then(setConfig).catch(() => {});
            getSettings().then((s) => setPageSize(s.result_limit || 80)).catch(() => {});
            showToast(shortcutChanged ? "Saved · restart to apply shortcut" : "Settings saved");
          }}
        />
      )}
      {workView && config && (
        <WorkView work={workView} archiveRoot={config.archive_path} onClose={() => setWorkView(null)} onToast={showToast} />
      )}
    </div>
  );
}
