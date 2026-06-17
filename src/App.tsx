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
import {
  searchQuery,
  runImport,
  runZoteroImport,
  getStats,
  getFacets,
  getConfig,
  getSettings,
  highlightPosition,
} from "./lib/api";
import { buildSearchQuery, parseSearch } from "./lib/query";
import { groupRows } from "./lib/grouping";
import { copyText } from "./lib/clipboard";
import { markdownQuote, workMarkdownPath } from "./lib/format";
import { resolveColor } from "./types";
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
  const [density, setDensity] = useState<Density>(() => persist.load("density", "compact", ["compact", "comfortable", "full"]));
  const [mode, setMode] = useState<SearchMode>("keyword");
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
  const [toast, setToast] = useState("");

  const [overlay, setOverlay] = useState<null | "tags" | "settings">(null);
  const [workView, setWorkView] = useState<SearchResult | null>(null);

  const inputRef = useRef<HTMLInputElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const reqRef = useRef(0);

  const terms = useMemo(() => parseSearch(query).positive_terms, [query]);
  const sections = useMemo(() => groupRows(rows, group, sort), [rows, group, sort]);
  const visualRows = useMemo(
    () => (sections ? sections.flatMap((s) => s.rows) : rows),
    [sections, rows]
  );
  const activeRow = useMemo(
    () => rows.find((r) => r.highlight_id === activeId) ?? null,
    [rows, activeId]
  );

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
  }, [refreshMeta]);

  useEffect(() => persist.save("sort", sort), [sort]);
  useEffect(() => persist.save("group", group), [group]);
  useEffect(() => persist.save("density", density), [density]);
  useEffect(() => persist.saveScope(scope), [scope]);

  // Refocus search box when shown via the global hotkey.
  useEffect(() => {
    const un = getCurrentWindow().onFocusChanged(({ payload }) => {
      if (payload) {
        inputRef.current?.focus();
        inputRef.current?.select();
      }
    });
    return () => { un.then((f) => f()); };
  }, []);

  // Import progress events.
  useEffect(() => {
    const a = listen<string>("import:progress", (e) => setStatus(e.payload));
    const b = listen<{ message: string }>("import:complete", (e) => {
      setStatus(e.payload.message);
      setImporting(false);
      refreshMeta();
    });
    return () => { a.then((f) => f()); b.then((f) => f()); };
  }, [refreshMeta]);

  const runSearch = useCallback(
    async (nextPage: number, append: boolean) => {
      if (mode === "semantic") {
        setRows([]);
        setStatus("Semantic search (QMD) is coming soon — switch to keyword.");
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
          raw: query, scope, source: null, color, sort, mode, page: nextPage, pageSize,
        });
        const result = await searchQuery(payload);
        if (reqId !== reqRef.current) return;
        setHasMore(result.has_more);
        setRows((prev) => (append ? [...prev, ...result.rows] : result.rows));
        if (!append) {
          setActiveId(result.rows[0]?.highlight_id ?? null);
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
    [query, scope, color, sort, mode, pageSize]
  );

  // Re-run from page 0 when query/filters/sort change (debounced).
  useEffect(() => {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => {
      setPage(0);
      runSearch(0, false);
    }, DEBOUNCE_MS);
    return () => { if (debounceRef.current) clearTimeout(debounceRef.current); };
  }, [query, scope, color, sort, mode, runSearch]);

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
  const openSource = () => { if (activeRow?.url) openUrl(activeRow.url); };
  const openWorkMd = () => {
    if (activeRow && config) {
      openPath(workMarkdownPath(config.archive_path, activeRow.slug)).catch(() => showToast("Markdown not found"));
    }
  };

  const pickTag = (tag: string) => {
    setQuery((c) => (c ? `${c} tag:"${tag}"` : `tag:"${tag}"`));
    setOverlay(null);
    inputRef.current?.focus();
  };

  const doImport = async (which: "readwise" | "zotero") => {
    setImporting(true);
    setStatus(which === "readwise" ? "Starting Readwise import…" : "Starting Zotero import…");
    try {
      await (which === "readwise" ? runImport() : runZoteroImport());
    } catch (e) {
      setStatus(`Import failed: ${e instanceof Error ? e.message : String(e)}`);
      setImporting(false);
    }
  };

  const onKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    const mod = e.metaKey || e.ctrlKey;
    if (e.key === "ArrowDown") { e.preventDefault(); move(1); }
    else if (e.key === "ArrowUp") { e.preventDefault(); move(-1); }
    else if (e.key === "Enter") { e.preventDefault(); openSource(); }
    else if (mod && e.shiftKey && e.key.toLowerCase() === "c") { e.preventDefault(); copyMarkdown(); }
    else if (mod && e.key.toLowerCase() === "c" && !window.getSelection()?.toString()) { e.preventDefault(); copyHighlight(); }
    else if (mod && e.shiftKey && e.key.toLowerCase() === "p") { e.preventDefault(); setShowPane((s) => !s); }
    else if (mod && e.shiftKey && e.key.toLowerCase() === "t") { e.preventDefault(); setOverlay("tags"); }
    else if (mod && e.shiftKey && e.key.toLowerCase() === "l") { e.preventDefault(); if (activeRow) setWorkView(activeRow); }
    else if (mod && e.shiftKey && e.key.toLowerCase() === "o") { e.preventDefault(); openWorkMd(); }
    else if (mod && e.key === ",") { e.preventDefault(); setOverlay("settings"); }
    else if (e.key === "Escape") {
      e.preventDefault();
      if (workView) setWorkView(null);
      else if (overlay) setOverlay(null);
      else if (query) setQuery("");
      else getCurrentWindow().hide();
    }
  };

  const total = stats ? `${stats.highlights.toLocaleString()} highlights · ${stats.works.toLocaleString()} works` : "";

  return (
    <div className="relative flex h-screen flex-col overflow-hidden bg-white text-zinc-900" onKeyDown={onKeyDown}>
      <div className="flex items-center gap-2 border-b border-zinc-200 pr-3">
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
      </div>

      <Toolbar
        sort={sort} group={group} mode={mode} density={density} showPane={showPane}
        onSort={setSort} onGroup={setGroup} onMode={setMode} onDensity={setDensity}
        onTogglePane={() => setShowPane((s) => !s)}
        onOpenTags={() => setOverlay("tags")}
        onOpenSettings={() => setOverlay("settings")}
        onImport={doImport}
        importing={importing}
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

      <div className="flex min-h-0 flex-1">
        <div className={`flex min-w-0 flex-col ${showPane ? "w-[46%] border-r border-zinc-100" : "flex-1"}`}>
          {visualRows.length === 0 ? (
            <div className="flex flex-1 flex-col items-center justify-center gap-3 px-6 text-center text-zinc-400">
              {!query && !scope && !color ? (
                <>
                  {total && <p className="text-sm">{total}</p>}
                  {config && stats?.highlights === 0 && (
                    <p className="rounded bg-amber-50 px-3 py-2 text-xs text-amber-600">
                      No data yet. Add a Readwise key in Settings (⚙) or import Zotero.
                    </p>
                  )}
                  <p className="text-xs text-zinc-300">
                    cat OR dog · "exact phrase" · -exclude · prefix* · au:scott ty:books y:2023 · /\bAI\b/
                  </p>
                </>
              ) : (
                <p className="text-sm">{loading ? "Searching…" : `No results for "${query}"`}</p>
              )}
            </div>
          ) : (
            <ResultsList
              rows={rows}
              sections={sections}
              density={density}
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

      <div className="flex items-center justify-between border-t border-zinc-100 bg-zinc-50 px-4 py-1.5 text-xs text-zinc-400">
        <span className="truncate">
          {rows.length > 0 ? `${rows.length} shown${hasMore ? "+" : ""}` : total}
          {status && <span className={importing ? "text-blue-500" : "text-zinc-500"}> · {status}</span>}
        </span>
        <span className="shrink-0 text-zinc-300">↑↓ nav · ↵ source · ⌘C copy · ⌘⇧C md · ⌘⇧L work · ⌘⇧P pane · esc</span>
      </div>

      {toast && (
        <div className="absolute bottom-10 left-1/2 -translate-x-1/2 rounded bg-zinc-800 px-3 py-1.5 text-xs text-white shadow-lg">
          {toast}
        </div>
      )}

      {overlay === "tags" && <TagPicker onPick={pickTag} onClose={() => setOverlay(null)} />}
      {overlay === "settings" && (
        <SettingsPanel
          onClose={() => setOverlay(null)}
          onSaved={(shortcutChanged) => {
            setOverlay(null);
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
