// State for the highlight index search (Classic's engine), used when
// Highlights alone is searched: debounced keyword search with the popover's
// filters and colour, paging on scroll, Classic's
// grouping, the selected row and its position in the work.
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { highlightPosition, searchQuery } from "./api";
import { flattenSections, groupRows } from "./grouping";
import { CLASSIC_SORT, highlightPayload, highlightSearchable, type GroupBy, type SearchState } from "./searchModel";
import type { GroupMode, SearchResult, WorkPosition } from "../types";

const DEBOUNCE_MS = 130;

const asMode = (g: GroupBy): GroupMode => (g === "corpus" ? "none" : g);

export function useHighlightSearch(opts: {
  query: string;
  enabled: boolean;
  state: SearchState;
  group: GroupBy;
  subgroup: GroupBy;
  pageSize: number;
  /** Highlight sources the rail lists, so a partial tick restricts to them. */
  knownSources: string[];
  /** Bumped to re-run the current search (imports, Refresh). */
  dataVersion: number;
}) {
  const { query, enabled, state, pageSize, knownSources, dataVersion } = opts;
  const [rows, setRows] = useState<SearchResult[]>([]);
  const [page, setPage] = useState(0);
  const [hasMore, setHasMore] = useState(false);
  const [loading, setLoading] = useState(false);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [position, setPosition] = useState<WorkPosition | null>(null);
  const [status, setStatus] = useState("");
  const reqRef = useRef(0);

  const sort = CLASSIC_SORT[state.sort];
  const group = asMode(opts.group);
  const subgroup = asMode(opts.subgroup);
  const sections = useMemo(() => groupRows(rows, group, subgroup, sort), [rows, group, subgroup, sort]);
  const visualRows = useMemo(() => flattenSections(sections, rows), [sections, rows]);
  const activeRow = useMemo(() => rows.find((r) => r.highlight_id === activeId) ?? null, [rows, activeId]);

  // Keep the selection on the first visible (grouped) row when it is empty or
  // has fallen out of the results, so a new search selects the top row.
  useEffect(() => {
    if (visualRows.length === 0) {
      if (activeId !== null) setActiveId(null);
      return;
    }
    if (!activeId || !visualRows.some((r) => r.highlight_id === activeId)) setActiveId(visualRows[0].highlight_id);
  }, [visualRows, activeId]);

  const knownKey = knownSources.join(",");
  const filterKey = JSON.stringify([state.filters, state.color, state.sort, state.partial, state.offSources, knownKey]);

  const runPage = useCallback(async (nextPage: number, append: boolean) => {
    if (!highlightSearchable(state, query)) {
      reqRef.current++;
      setRows([]);
      setHasMore(false);
      setLoading(false);
      return;
    }
    const payload = highlightPayload(state, query, nextPage, pageSize, knownSources);
    if (!payload) {
      reqRef.current++;
      setRows([]);
      setHasMore(false);
      return;
    }
    const reqId = ++reqRef.current;
    setLoading(true);
    try {
      const result = await searchQuery(payload);
      if (reqId !== reqRef.current) return;
      setHasMore(result.has_more);
      setRows((prev) => (append ? [...prev, ...result.rows] : result.rows));
      if (!append) {
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
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query, filterKey, pageSize]);

  // Re-run from page 0 when the query or a filter changes (debounced).
  // (Semantic search goes to the corpus engine, never here.)
  useEffect(() => {
    if (!enabled) return;
    setStatus("");
    const t = setTimeout(() => {
      setPage(0);
      runPage(0, false);
    }, DEBOUNCE_MS);
    return () => clearTimeout(t);
  }, [enabled, runPage, dataVersion]);

  const loadMore = useCallback(() => {
    if (loading || !hasMore) return;
    const n = page + 1;
    setPage(n);
    runPage(n, true);
  }, [loading, hasMore, page, runPage]);

  // Position in the work ("3 of 12 · location 2011 of 4400") for the selected row.
  useEffect(() => {
    setPosition(null);
    const r = activeRow;
    if (!enabled || !r || !r.location || (r.work_type || "").toLowerCase().startsWith("tweet")) return;
    let cancelled = false;
    highlightPosition(r.work_id, r.location)
      .then((p) => { if (!cancelled) setPosition(p); })
      .catch(() => {});
    return () => { cancelled = true; };
  }, [activeRow, enabled]);

  const move = useCallback((delta: number) => {
    if (!visualRows.length) return;
    const i = visualRows.findIndex((r) => r.highlight_id === activeId);
    setActiveId(visualRows[Math.max(0, Math.min(visualRows.length - 1, (i < 0 ? 0 : i) + delta))].highlight_id);
  }, [visualRows, activeId]);

  /** ⌥↓ / ⌥↑: the first row of the next (or previous) work, author, year or tag group; wraps. */
  const moveGroup = useCallback((dir: 1 | -1) => {
    const firsts = sectionFirsts(sections);
    if (!firsts.length) return;
    const at = sectionIndexOf(sections, activeId);
    const n = at < 0 ? 0 : (at + dir + firsts.length) % firsts.length;
    setActiveId(firsts[n]);
  }, [sections, activeId]);

  return { rows, sections, visualRows, activeId, setActiveId, activeRow, position, loading, hasMore, loadMore, status, move, moveGroup };
}

type Sections = ReturnType<typeof groupRows>;

/** The first row id of each innermost section, in display order. */
export function sectionFirsts(sections: Sections): string[] {
  if (!sections) return [];
  return sections.flatMap((s) => (s.subs ?? [s]).filter((x) => x.rows.length).map((x) => x.rows[0].highlight_id));
}

function sectionIndexOf(sections: Sections, id: string | null): number {
  if (!sections || !id) return -1;
  const inner = sections.flatMap((s) => s.subs ?? [s]).filter((x) => x.rows.length);
  return inner.findIndex((x) => x.rows.some((r) => r.highlight_id === id));
}

export type HighlightSearchState = ReturnType<typeof useHighlightSearch>;
