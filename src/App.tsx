import { useState, useEffect, useRef, useCallback, type KeyboardEvent } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { openUrl } from "@tauri-apps/plugin-opener";
import { SearchBar } from "./components/SearchBar";
import { FilterBar } from "./components/FilterBar";
import { ResultsList } from "./components/ResultsList";
import { HighlightDetail } from "./components/HighlightDetail";
import {
  searchHighlights,
  runImport,
  runZoteroImport,
  getStats,
  getFacets,
  getConfig,
} from "./lib/api";
import type { SearchResult, Stats, Config, Facets, SearchMode } from "./types";

const DEBOUNCE_MS = 150;

export default function App() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [activeIndex, setActiveIndex] = useState(0);
  const [selected, setSelected] = useState<string | null>(null);
  const [isSearching, setIsSearching] = useState(false);
  const [importMsg, setImportMsg] = useState("");
  const [isImporting, setIsImporting] = useState(false);
  const [stats, setStats] = useState<Stats | null>(null);
  const [config, setConfig] = useState<Config | null>(null);
  const [facets, setFacets] = useState<Facets | null>(null);

  // Filters (ADR-0005: keyword default; source/colour per user request)
  const [source, setSource] = useState<string | null>(null);
  const [color, setColor] = useState<string | null>(null);
  const [mode, setMode] = useState<SearchMode>("keyword");

  const inputRef = useRef<HTMLInputElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const selectedResult = results.find((r) => r.highlight_id === selected) ?? null;

  const refreshMeta = useCallback(() => {
    getStats().then(setStats).catch(() => {});
    getFacets().then(setFacets).catch(() => {});
  }, []);

  useEffect(() => {
    refreshMeta();
    getConfig().then(setConfig).catch(() => {});
  }, [refreshMeta]);

  // Refocus the search box whenever the window is shown via the global hotkey.
  useEffect(() => {
    const w = getCurrentWindow();
    const un = w.onFocusChanged(({ payload: focused }) => {
      if (focused) {
        inputRef.current?.focus();
        inputRef.current?.select();
      }
    });
    return () => {
      un.then((f) => f());
    };
  }, []);

  // Import progress events from Rust.
  useEffect(() => {
    const a = listen<string>("import:progress", (e) => setImportMsg(e.payload));
    const b = listen<{ message: string }>("import:complete", (e) => {
      setImportMsg(e.payload.message);
      setIsImporting(false);
      refreshMeta();
    });
    return () => {
      a.then((f) => f());
      b.then((f) => f());
    };
  }, [refreshMeta]);

  // Run search when query or any filter changes (debounced).
  useEffect(() => {
    if (debounceRef.current) clearTimeout(debounceRef.current);

    if (!query.trim()) {
      setResults([]);
      setIsSearching(false);
      return;
    }
    if (mode === "semantic") {
      setResults([]);
      setIsSearching(false);
      setImportMsg("Semantic search (QMD) is coming soon — switch to keyword.");
      return;
    }

    setIsSearching(true);
    debounceRef.current = setTimeout(async () => {
      try {
        const r = await searchHighlights({ query, source, color, mode });
        setResults(r);
        setActiveIndex(0);
      } catch (e) {
        console.error("Search error:", e);
        setResults([]);
      } finally {
        setIsSearching(false);
      }
    }, DEBOUNCE_MS);
  }, [query, source, color, mode]);

  const handleKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActiveIndex((i) => Math.min(i + 1, results.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActiveIndex((i) => Math.max(i - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      const r = results[activeIndex];
      if (!r) return;
      if (e.metaKey && r.url) {
        openUrl(r.url);
      } else {
        setSelected(r.highlight_id);
      }
    } else if (e.key === "Escape") {
      e.preventDefault();
      if (selected) {
        setSelected(null);
      } else if (query) {
        setQuery("");
      } else {
        getCurrentWindow().hide();
      }
    }
  };

  const doImport = async (which: "readwise" | "zotero") => {
    setIsImporting(true);
    setImportMsg(which === "readwise" ? "Starting Readwise import…" : "Starting Zotero import…");
    try {
      await (which === "readwise" ? runImport() : runZoteroImport());
    } catch (e: unknown) {
      setImportMsg(`Import failed: ${e instanceof Error ? e.message : String(e)}`);
      setIsImporting(false);
    }
  };

  return (
    <div
      className="flex flex-col h-screen bg-white text-zinc-900 overflow-hidden"
      onKeyDown={handleKeyDown}
    >
      <SearchBar
        ref={inputRef}
        value={query}
        onChange={setQuery}
        isSearching={isSearching}
        placeholder={
          stats ? `Search ${stats.highlights.toLocaleString()} highlights…` : "Search highlights…"
        }
      />

      <FilterBar
        facets={facets}
        source={source}
        color={color}
        mode={mode}
        onSource={setSource}
        onColor={setColor}
        onMode={setMode}
      />

      {results.length > 0 && (
        <ResultsList
          results={results}
          activeIndex={activeIndex}
          selected={selected}
          onSelect={(id) => setSelected(selected === id ? null : id)}
          onActivate={setActiveIndex}
        />
      )}

      {!query && results.length === 0 && (
        <div className="flex-1 flex flex-col items-center justify-center gap-4 text-zinc-400 px-6">
          {stats && (
            <p className="text-sm">
              {stats.highlights.toLocaleString()} highlights across{" "}
              {stats.works.toLocaleString()} works
            </p>
          )}
          {config && stats?.highlights === 0 && (
            <p className="text-xs text-amber-600 bg-amber-50 rounded px-3 py-2 text-center">
              No data yet. Add a Readwise key in{" "}
              <code className="font-mono">~/.config/highlight-scout/config.toml</code>{" "}
              or import Zotero below.
            </p>
          )}
          <p className="text-xs text-zinc-300">↑↓ navigate · ↵ open · ⌘↵ source · esc hide</p>
        </div>
      )}

      {query && !isSearching && results.length === 0 && mode === "keyword" && (
        <div className="flex-1 flex items-center justify-center text-sm text-zinc-400">
          No results for "{query}"
        </div>
      )}

      {selectedResult && (
        <HighlightDetail result={selectedResult} onClose={() => setSelected(null)} />
      )}

      <div className="flex items-center justify-between border-t border-zinc-100 px-4 py-2 text-xs text-zinc-400 bg-zinc-50">
        <span className="truncate">
          {results.length > 0 && `${results.length} results`}
          {importMsg && (
            <span className={isImporting ? "text-blue-500" : "text-zinc-500"}>
              {results.length > 0 ? " · " : ""}
              {importMsg}
            </span>
          )}
        </span>
        <span className="flex items-center gap-1 shrink-0">
          <button
            onClick={() => doImport("readwise")}
            disabled={isImporting}
            className="rounded px-2 py-1 hover:bg-zinc-200 disabled:opacity-50 transition-colors"
          >
            Import Readwise
          </button>
          <button
            onClick={() => doImport("zotero")}
            disabled={isImporting}
            className="rounded px-2 py-1 hover:bg-zinc-200 disabled:opacity-50 transition-colors"
          >
            Import Zotero
          </button>
        </span>
      </div>
    </div>
  );
}
