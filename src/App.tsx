import { useState, useEffect, useCallback, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { SearchBar } from "./components/SearchBar";
import { ResultsList } from "./components/ResultsList";
import { HighlightDetail } from "./components/HighlightDetail";
import { searchHighlights, runImport, getStats, getConfig } from "./lib/api";
import type { SearchResult, Stats, Config } from "./types";

const DEBOUNCE_MS = 150;

export default function App() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [isSearching, setIsSearching] = useState(false);
  const [isImporting, setIsImporting] = useState(false);
  const [importMsg, setImportMsg] = useState("");
  const [stats, setStats] = useState<Stats | null>(null);
  const [config, setConfig] = useState<Config | null>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const selectedResult = results.find((r) => r.highlight_id === selected) ?? null;

  useEffect(() => {
    getStats().then(setStats).catch(() => {});
    getConfig().then(setConfig).catch(() => {});
  }, []);

  useEffect(() => {
    const unlisten1 = listen<string>("import:progress", (e) => {
      setImportMsg(e.payload);
    });
    const unlisten2 = listen<{ message: string }>("import:complete", (e) => {
      setImportMsg(e.payload.message);
      setIsImporting(false);
      getStats().then(setStats).catch(() => {});
    });
    return () => {
      unlisten1.then((f) => f());
      unlisten2.then((f) => f());
    };
  }, []);

  const handleQueryChange = useCallback((value: string) => {
    setQuery(value);
    setSelected(null);
    if (debounceRef.current) clearTimeout(debounceRef.current);
    if (!value.trim()) {
      setResults([]);
      setIsSearching(false);
      return;
    }
    setIsSearching(true);
    debounceRef.current = setTimeout(async () => {
      try {
        const r = await searchHighlights(value);
        setResults(r);
      } catch (e) {
        console.error("Search error:", e);
        setResults([]);
      } finally {
        setIsSearching(false);
      }
    }, DEBOUNCE_MS);
  }, []);

  const handleImport = async () => {
    setIsImporting(true);
    setImportMsg("Starting import…");
    try {
      await runImport();
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      setImportMsg(`Import failed: ${msg}`);
      setIsImporting(false);
    }
  };

  return (
    <div className="flex flex-col h-screen bg-white text-zinc-900 overflow-hidden">
      <SearchBar
        value={query}
        onChange={handleQueryChange}
        isSearching={isSearching}
        placeholder={
          stats
            ? `Search ${stats.highlights.toLocaleString()} highlights…`
            : "Search highlights…"
        }
      />

      {results.length > 0 && (
        <ResultsList
          results={results}
          selected={selected}
          onSelect={(id) => setSelected(selected === id ? null : id)}
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
          {config && !config.has_api_key && (
            <p className="text-xs text-amber-600 bg-amber-50 rounded px-3 py-2 text-center">
              No API key configured.
              <br />
              Edit{" "}
              <code className="font-mono">
                ~/.config/highlight-scout/config.toml
              </code>
            </p>
          )}
        </div>
      )}

      {query && !isSearching && results.length === 0 && (
        <div className="flex-1 flex items-center justify-center text-sm text-zinc-400">
          No results for "{query}"
        </div>
      )}

      {selectedResult && (
        <HighlightDetail
          result={selectedResult}
          onClose={() => setSelected(null)}
        />
      )}

      <div className="flex items-center justify-between border-t border-zinc-100 px-4 py-2 text-xs text-zinc-400 bg-zinc-50">
        <span>
          {results.length > 0 && `${results.length} results`}
          {importMsg && (
            <span className={isImporting ? "text-blue-500" : "text-zinc-500"}>
              {results.length > 0 ? " · " : ""}
              {importMsg}
            </span>
          )}
        </span>
        <button
          onClick={handleImport}
          disabled={isImporting}
          className="rounded px-2 py-1 hover:bg-zinc-200 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          {isImporting ? "Importing…" : "Import Readwise"}
        </button>
      </div>
    </div>
  );
}
