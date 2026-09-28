// The rail's result counts (ticket 08): with a query, each rail row shows how
// many results the query has in that corpus or highlight source, the number
// its "show all" would list. Writing and tweets are counted by the corpus
// engine (documents, the `total_documents` of a one-corpus search); Highlights
// and its sources by the highlight index (highlights, what Highlights alone
// lists). One count call per corpus, cached per request; an answer to an
// older query is dropped. Never computed from the visible page.
// Pure: the engine calls are passed in.
import type { ArchiveSearchRequest } from "./archive";
import { highlightPayload, narrowedBy, timeToken, type HighlightPayload, type SearchState } from "./searchModel";

export interface HighlightCounts {
  total: number;
  sources: Record<string, number>;
}

export interface RailCountJob {
  /** Identifies the answers: the same key is the same counts. */
  key: string;
  /** One corpus-engine request per corpus other than highlights. */
  archive: Array<{ corpus: string; request: ArchiveSearchRequest }>;
  /** Corpora the query cannot reach (highlight filters or tokens narrow it to highlights). */
  notSearched: string[];
  /** The highlight index request for Highlights and every source. */
  highlights: HighlightPayload | null;
  /** The sources the rail lists, counted 0 when nothing matches there. */
  sources: string[];
}

/**
 * What to count for this query, or null when the rail shows sizes: no query,
 * or semantic search (which runs on Return and has no count).
 */
export function railCountJob(s: SearchState, query: string, listed: string[], known: string[]): RailCountJob | null {
  if (!query.trim() || s.mode === "semantic") return null;
  const narrowed = narrowedBy(s, query) !== null;
  const q = [query.trim(), timeToken(s.filters.time)].filter(Boolean).join(" ");
  const others = listed.filter((c) => c !== "highlights");
  const archive = narrowed ? [] : others.map((corpus) => ({ corpus, request: { query: q, in: [corpus], limit: 1 } }));
  // Every source, as "show all" on Highlights searches them.
  const highlights = listed.includes("highlights") ? highlightPayload({ ...s, offSources: [] }, query, 0, 1, known) : null;
  const job = { archive, notSearched: narrowed ? others : [], highlights, sources: known };
  return { key: JSON.stringify(job), ...job };
}

export interface RailCounts {
  key: string;
  /** Results per corpus; null when not searched or the count failed. */
  corpora: Record<string, number | null>;
  /** Results per highlight source; null when the count failed. */
  sources: Record<string, number | null>;
  /** Corpora still being counted ("highlights" covers its sources). Their old value, if any, is shown faded. */
  pending: string[];
}

/** A rail row's count: its value and whether it is still being counted. */
export function rowCount(c: RailCounts, corpus: string, source?: string): { value: number | null | undefined; pending: boolean } {
  const value = source === undefined ? c.corpora[corpus] : c.sources[source];
  return { value, pending: c.pending.includes(corpus) };
}

/** Results the highlight list has for the ticked sources, once counted. */
export function tickedHighlightTotal(c: RailCounts | null, offSources: string[]): number | null {
  if (!c || c.pending.includes("highlights")) return null;
  if (!offSources.length) return c.corpora.highlights ?? null;
  let n = 0;
  for (const [src, v] of Object.entries(c.sources)) {
    if (offSources.includes(src)) continue;
    if (v === null) return null;
    n += v;
  }
  return n;
}

export interface RailCounterDeps {
  countArchive: (r: ArchiveSearchRequest) => Promise<number>;
  countHighlights: (p: HighlightPayload) => Promise<HighlightCounts>;
  onChange: (c: RailCounts | null) => void;
  /** Queries remembered (default 40). */
  cacheSize?: number;
}

export interface RailCounter {
  /** Count for this job (null: back to sizes). Answers to an earlier run are dropped. */
  run: (job: RailCountJob | null) => void;
  /** Forget every cached count (the archive or an index changed). */
  clear: () => void;
}

export function createRailCounter(deps: RailCounterDeps): RailCounter {
  const cacheSize = deps.cacheSize ?? 40;
  const cache = new Map<string, RailCounts>();
  let gen = 0;
  let current: RailCounts | null = null;
  const emit = (c: RailCounts | null) => {
    current = c;
    deps.onChange(c);
  };
  const remember = (c: RailCounts) => {
    cache.delete(c.key);
    cache.set(c.key, c);
    while (cache.size > cacheSize) cache.delete(cache.keys().next().value as string);
  };

  return {
    run(job) {
      const g = ++gen;
      if (!job) return emit(null);
      const hit = cache.get(job.key);
      if (hit) {
        remember(hit);
        return emit(hit);
      }
      let failed = false;
      let state: RailCounts = {
        key: job.key,
        corpora: { ...(current?.corpora ?? {}) },
        sources: { ...(current?.sources ?? {}) },
        pending: [...job.archive.map((a) => a.corpus), ...(job.highlights ? ["highlights"] : [])],
      };
      for (const c of job.notSearched) state.corpora[c] = null;
      emit(state);
      if (!state.pending.length) return remember(state);

      const settle = (id: string, patch: Partial<Pick<RailCounts, "corpora" | "sources">>) => {
        if (g !== gen) return; // an older query's answer
        state = { ...state, ...patch, pending: state.pending.filter((x) => x !== id) };
        if (!state.pending.length && !failed) remember(state);
        emit(state);
      };
      for (const { corpus, request } of job.archive) {
        deps.countArchive(request).then((n) => {
          if (typeof n !== "number") throw new Error("no count");
          return n;
        }).then(
          (n) => settle(corpus, { corpora: { ...state.corpora, [corpus]: n } }),
          () => { failed = true; settle(corpus, { corpora: { ...state.corpora, [corpus]: null } }); },
        );
      }
      if (job.highlights) {
        deps.countHighlights(job.highlights).then((h) => {
          if (!h || typeof h.total !== "number") throw new Error("no count");
          return h;
        }).then(
          (h) => settle("highlights", {
            corpora: { ...state.corpora, highlights: h.total },
            sources: Object.fromEntries([...new Set([...job.sources, ...Object.keys(h.sources)])].map((s) => [s, h.sources[s] ?? 0])),
          }),
          () => {
            failed = true;
            settle("highlights", { corpora: { ...state.corpora, highlights: null }, sources: Object.fromEntries(job.sources.map((s) => [s, null])) });
          },
        );
      }
    },
    clear() {
      cache.clear();
    },
  };
}
