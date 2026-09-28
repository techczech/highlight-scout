// State for archive search (writing, tweets, highlights): debounced search
// for the ticked corpora (one or more engine requests, merged), results in
// the chosen groups, the selected
// document and passage with its citation, the rail's counts, and the
// background index keeper's progress. The engine runs in the backend, off
// the main thread.
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  archiveCounts,
  archivePassage,
  archiveIndexRefresh,
  archiveSearch,
  errorLine,
  jobBuilt,
  noteLine,
  resultSummary,
  toCorpusError,
  wantsIndex,
  type ArchiveSearchRequest,
  type ArchiveSearchResults,
  type CorpusCount,
  type IndexJob,
  type PassageView,
} from "./archive";
import { docKey, groupArchive, groupKey, moveKey, type ArchiveGroupBy, type ResultSort } from "./quickFinder";
import { mergeResults } from "./searchModel";

const DEBOUNCE_MS = 160;
export const ARCHIVE_LIMIT = 50;

export { docKey };

export interface ArchiveView {
  by: ArchiveGroupBy;
  /** More than one corpus searched: corpus groups show their first rows. */
  multi: boolean;
  sort: ResultSort;
}

/**
 * `manual` (Semantic): the search does not run as the query or the choices
 * change (each run embeds the query and loads the vectors); a change clears
 * the results, and `run()` (Return) searches.
 */
export function useArchiveSearch(query: string, enabled: boolean, requests: ArchiveSearchRequest[], view: ArchiveView, manual = false) {
  const [results, setResults] = useState<ArchiveSearchResults | null>(null);
  const [notes, setNotes] = useState<string[]>([]);
  const [error, setError] = useState<string>("");
  const [loading, setLoading] = useState(false);
  const [activeKey, setActiveKey] = useState<string | null>(null);
  const [passageId, setPassageId] = useState<string | null>(null);
  const [passage, setPassage] = useState<PassageView | null>(null);
  const [counts, setCounts] = useState<CorpusCount[]>([]);
  const [citeError, setCiteError] = useState("");
  const [job, setJob] = useState<IndexJob | null>(null);
  const [version, setVersion] = useState(0);
  const [runKey, setRunKey] = useState(0);
  const ranKey = useRef(0);
  const lastWhat = useRef("");
  const reqRef = useRef(0);

  const docs = results?.results ?? [];
  const groups = useMemo(() => groupArchive(results, view.by, view.sort, view.multi), [results, view.by, view.sort, view.multi]);
  const reqKey = JSON.stringify(requests);
  const active = useMemo(() => docs.find((d) => docKey(d) === activeKey) ?? null, [docs, activeKey]);

  // The rail's counts: at start, and whenever the keeper rebuilt an index.
  useEffect(() => {
    archiveCounts().then(setCounts).catch(() => {});
  }, [version]);

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
    const asked = runKey !== ranKey.current;
    ranKey.current = runKey;
    const what = `${manual}|${query}|${reqKey}`;
    const same = what === lastWhat.current;
    lastWhat.current = what;
    // Semantic keeps its results through an index rebuild; a new query or choice clears them.
    if (manual && !asked && same) return;
    if (!query.trim() || (manual && !asked)) {
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
        const answers = await Promise.all(requests.map((q) => archiveSearch(q)));
        if (reqId !== reqRef.current) return;
        const body = mergeResults(answers.map((a) => a.body));
        setResults(body);
        setNotes([...new Set(answers.flatMap((a) => a.notes).map(noteLine))]);
        setError("");
        const first = groupArchive(body, view.by, view.sort, view.multi)[0]?.docs[0];
        setActiveKey(first ? docKey(first) : null);
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
    }, manual ? 0 : DEBOUNCE_MS);
    return () => clearTimeout(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query, enabled, version, refreshIndex, reqKey, manual, runKey]);

  /** Run the search now (Semantic's Return; Refresh). */
  const run = useCallback(() => setRunKey((k) => k + 1), []);

  // The selected document's first hit is the passage shown, until another is picked.
  useEffect(() => {
    setPassageId(active?.hits[0]?.passage_id ?? null);
  }, [active]);

  useEffect(() => {
    setPassage(null);
    setCiteError("");
    if (!passageId) return;
    let cancelled = false;
    archivePassage(passageId)
      .then((r) => { if (!cancelled) setPassage(r.body); })
      .catch((e) => { if (!cancelled) setCiteError(toCorpusError(e).message); });
    return () => { cancelled = true; };
  }, [passageId]);

  // ↑↓ in the grouped order; ⌥↓ / ⌥↑ to the next / previous corpus group.
  const move = useCallback((delta: number) => {
    const k = moveKey(groups, activeKey, delta);
    if (k) setActiveKey(k);
  }, [groups, activeKey]);
  const moveGroup = useCallback((dir: 1 | -1) => {
    const k = groupKey(groups, activeKey, dir);
    if (k) setActiveKey(k);
  }, [groups, activeKey]);

  const cited = passage?.cited ?? null;
  /** Re-run the current search and reload the counts (Refresh). */
  const rerun = useCallback(() => {
    setVersion((v) => v + 1);
    if (manual) setRunKey((k) => k + 1);
  }, [manual]);

  const summary = results ? resultSummary(results) : "";
  const indexBusy = job?.phase === "checking" || job?.phase === "building";

  return {
    docs, groups, results, notes, error, loading, summary, counts,
    activeKey, setActiveKey, active, move, moveGroup,
    passageId, setPassageId, passage, cited, citeError,
    job, indexBusy, refreshIndex, rerun, run,
  };
}

export type ArchiveSearchState = ReturnType<typeof useArchiveSearch>;
