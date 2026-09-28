// State for archive search (writing, tweets, highlights): debounced search,
// the selected document and passage, its citation, and the background index
// keeper's progress. The engine runs in the backend, off the main thread.
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  archiveCite,
  archiveIndexRefresh,
  archiveSearch,
  errorLine,
  jobBuilt,
  noteLine,
  resultSummary,
  toCorpusError,
  wantsIndex,
  type ArchiveDoc,
  type ArchiveSearchResults,
  type CitedPassage,
  type IndexJob,
} from "./archive";

const DEBOUNCE_MS = 160;
const LIMIT = 50;

export function docKey(d: ArchiveDoc): string {
  return `${d.corpus}:${d.rel_path}`;
}

export function useArchiveSearch(query: string, enabled: boolean) {
  const [results, setResults] = useState<ArchiveSearchResults | null>(null);
  const [notes, setNotes] = useState<string[]>([]);
  const [error, setError] = useState<string>("");
  const [loading, setLoading] = useState(false);
  const [activeKey, setActiveKey] = useState<string | null>(null);
  const [passageId, setPassageId] = useState<string | null>(null);
  const [cited, setCited] = useState<CitedPassage | null>(null);
  const [citeError, setCiteError] = useState("");
  const [job, setJob] = useState<IndexJob | null>(null);
  const [version, setVersion] = useState(0);
  const reqRef = useRef(0);

  const docs = results?.results ?? [];
  const active = useMemo(() => docs.find((d) => docKey(d) === activeKey) ?? null, [docs, activeKey]);

  // Background index keeper: follow its progress; re-run the search when it built something.
  useEffect(() => {
    const un = listen<IndexJob>("corpus:index", (e) => {
      setJob(e.payload);
      if (jobBuilt(e.payload)) setVersion((v) => v + 1);
    });
    return () => { un.then((f) => f()); };
  }, []);

  const refreshIndex = useCallback(() => {
    archiveIndexRefresh().catch(() => {});
  }, []);

  // Check the indexes at launch, after every sync pass (the highlights archive
  // may have changed) and whenever archive search is switched on. A check
  // while one is running is a no-op in the backend.
  useEffect(() => {
    refreshIndex();
    const un = listen("sync:finished", () => refreshIndex());
    return () => { un.then((f) => f()); };
  }, [refreshIndex]);
  useEffect(() => {
    if (enabled) refreshIndex();
  }, [enabled, refreshIndex]);

  useEffect(() => {
    if (!enabled) return;
    if (!query.trim()) {
      reqRef.current++;
      setResults(null);
      setNotes([]);
      setError("");
      setLoading(false);
      return;
    }
    const reqId = ++reqRef.current;
    const t = setTimeout(async () => {
      setLoading(true);
      try {
        const r = await archiveSearch({ query, limit: LIMIT });
        if (reqId !== reqRef.current) return;
        setResults(r.body);
        setNotes(r.notes.map(noteLine));
        setError("");
        setActiveKey(r.body.results.length ? docKey(r.body.results[0]) : null);
      } catch (e) {
        if (reqId !== reqRef.current) return;
        const err = toCorpusError(e);
        setResults(null);
        setNotes([]);
        setError(errorLine(err));
        if (wantsIndex(err)) refreshIndex();
      } finally {
        if (reqId === reqRef.current) setLoading(false);
      }
    }, DEBOUNCE_MS);
    return () => clearTimeout(t);
  }, [query, enabled, version, refreshIndex]);

  // The selected document's first hit is the passage shown, until another is picked.
  useEffect(() => {
    setPassageId(active?.hits[0]?.passage_id ?? null);
  }, [active]);

  useEffect(() => {
    setCited(null);
    setCiteError("");
    if (!passageId) return;
    let cancelled = false;
    archiveCite(passageId)
      .then((r) => { if (!cancelled) setCited(r.body); })
      .catch((e) => { if (!cancelled) setCiteError(toCorpusError(e).message); });
    return () => { cancelled = true; };
  }, [passageId]);

  const move = useCallback((delta: number) => {
    if (docs.length === 0) return;
    const idx = docs.findIndex((d) => docKey(d) === activeKey);
    const next = Math.max(0, Math.min(docs.length - 1, (idx < 0 ? 0 : idx) + delta));
    setActiveKey(docKey(docs[next]));
  }, [docs, activeKey]);

  const summary = results ? resultSummary(results) : "";
  const indexBusy = job?.phase === "checking" || job?.phase === "building";

  return {
    docs, results, notes, error, loading, summary,
    activeKey, setActiveKey, active, move,
    passageId, setPassageId, cited, citeError,
    job, indexBusy, refreshIndex,
  };
}

export type ArchiveSearchState = ReturnType<typeof useArchiveSearch>;
