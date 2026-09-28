// The one search (HS-M1B): which corpora are ticked, every Classic tool's
// value, and what they turn into: the engine to ask (the corpus engine for
// writing and tweets, the highlight index for highlights), its request, the
// chips of the "Showing" row, and the Filters · Group button's label.
// Pure: no Tauri, no React, no storage (persistence is at the bottom, with
// the store passed in).
import { EMPTY_FILTERS, buildSearchQuery, parseSearch, scopeToFilters, type Filters, type SearchQueryPayload } from "@scout/query";
import type { ArchiveSearchRequest, ArchiveSearchResults } from "./archive";
import { CORPUS_LABEL, CORPUS_ORDER, SORT_LABEL, type CorpusId, type ResultSort } from "./quickFinder";
import type { Density, SortMode } from "../types";

export type GroupBy = "corpus" | "work" | "author" | "date" | "tag" | "none";
export type Mode = "keyword" | "semantic";
/**
 * How the corpus engine ranks a request: `fts` (full text), `hybrid` (full
 * text and meaning fused, so exact words still win) or `semantic` (meaning
 * alone). Keyword asks for full text; Semantic for hybrid, the engine's
 * default when vectors exist; "Find related" asks for meaning alone.
 */
export type RequestMode = "fts" | "semantic" | "hybrid";
/** Which engine answers: the corpus engine, or the highlight index (Classic's). */
export type Engine = "archive" | "highlights";

export interface SearchState {
  /** Ticked corpora, in rail order; never empty. */
  corpora: CorpusId[];
  /** Highlight sources unticked under a ticked Highlights (`x`, `readwise`, `zotero`). */
  offSources: string[];
  mode: Mode;
  /** "corpus" doubles as "the default": it falls back to None under one corpus. */
  group: GroupBy;
  subgroup: GroupBy;
  sort: ResultSort;
  density: Density;
  /** Match: partial (prefix) rather than whole word; keyword search only. */
  partial: boolean;
  color: string | null;
  filters: Filters;
}

export const DEFAULT_DENSITY: Density = "compact";

export const DEFAULT_STATE: SearchState = {
  corpora: [...CORPUS_ORDER],
  offSources: [],
  mode: "keyword",
  group: "corpus",
  subgroup: "none",
  sort: "best",
  density: DEFAULT_DENSITY,
  partial: false,
  color: null,
  filters: EMPTY_FILTERS,
};

export const GROUP_LABEL: Record<GroupBy, string> = {
  corpus: "Corpus",
  work: "Work",
  author: "Author",
  date: "Date (year)",
  tag: "Tag",
  none: "None",
};

export const DENSITY_LABEL: Record<Density, string> = {
  minimal: "Minimal",
  compact: "Compact",
  comfortable: "Comfortable",
  full: "Full quotes",
};
export const DENSITIES: Density[] = ["minimal", "compact", "comfortable", "full"];
export const SORTS: ResultSort[] = ["best", "newest", "oldest"];

export const TYPE_OPTIONS: Array<{ value: string; label: string }> = [
  { value: "articles", label: "Articles" },
  { value: "books", label: "Books" },
  { value: "tweets", label: "Tweets" },
  { value: "pdfs", label: "PDFs" },
  { value: "podcasts", label: "Podcasts" },
];

export const TIME_OPTIONS: Array<{ value: string; label: string }> = [
  { value: "", label: "Any" },
  { value: "t:30d", label: "30 days" },
  { value: "t:6m", label: "6 months" },
  { value: "t:12m", label: "Year" },
];

/** The quick finder's sort words, mapped onto the highlight index's sort. */
export const CLASSIC_SORT: Record<ResultSort, SortMode> = { best: "matches", newest: "recent", oldest: "oldest" };

const inOrder = (list: CorpusId[]) => CORPUS_ORDER.filter((c) => list.includes(c));

// ---------- the rail ----------

/** Tick or untick a corpus. The last ticked corpus stays ticked. */
export function toggleCorpus(s: SearchState, c: CorpusId): SearchState {
  const on = s.corpora.includes(c);
  if (on && s.corpora.length === 1) return s;
  const corpora = inOrder(on ? s.corpora.filter((x) => x !== c) : [...s.corpora, c]);
  return { ...s, corpora, offSources: c === "highlights" ? [] : s.offSources };
}

/** Tick or untick one highlight source; `known` is every source the rail lists. */
export function toggleSource(s: SearchState, src: string, known: string[]): SearchState {
  if (!s.corpora.includes("highlights")) {
    return { ...s, corpora: inOrder([...s.corpora, "highlights"]), offSources: known.filter((k) => k !== src) };
  }
  const off = s.offSources.includes(src) ? s.offSources.filter((x) => x !== src) : [...s.offSources, src];
  if (known.length && known.every((k) => off.includes(k))) {
    // Every source unticked is Highlights unticked, unless it is the only corpus.
    if (s.corpora.length === 1) return s;
    return { ...s, corpora: s.corpora.filter((c) => c !== "highlights"), offSources: [] };
  }
  return { ...s, offSources: off };
}

/** "show all" on a corpus group: that corpus alone, every source. */
export function onlyCorpus(s: SearchState, c: CorpusId): SearchState {
  return { ...s, corpora: [c], offSources: [] };
}

export function sourceTicked(s: SearchState, src: string): boolean {
  return s.corpora.includes("highlights") && !s.offSources.includes(src);
}

/** Keyword / Semantic. Both search every ticked corpus; the ticks stay. */
export function setMode(s: SearchState, mode: Mode): SearchState {
  return mode === s.mode ? s : { ...s, mode };
}

/** The corpus engine's ranking for the search box's mode. */
export function requestMode(s: SearchState): RequestMode {
  return s.mode === "semantic" ? "hybrid" : "fts";
}

// ---------- scope and engine ----------

/** A highlight-only filter is set in the popover (colour, favourites, Zotero, image, type). */
export function highlightFiltersSet(s: SearchState): boolean {
  const f = s.filters;
  return !!s.color || f.favorite || f.zotero || f.hasImage || f.types.length > 0;
}

/** The query carries a token only highlights have: `co:` `i:` `zo:` `tag:` `ty:`. */
export function highlightTokens(query: string): boolean {
  const p = parseSearch(query);
  return !!(p.color || p.has_image || p.zotero || p.tag || p.type);
}

/**
 * Why the results are highlights only although more is ticked, or null.
 * Semantic never narrows: it searches every ticked corpus through the corpus
 * engine, where the highlight-only popover filters do not apply.
 */
export function narrowedBy(s: SearchState, query: string): "filters" | "tokens" | null {
  if (s.mode === "semantic") return null;
  if (s.corpora.length === 1 && s.corpora[0] === "highlights") return null;
  if (highlightFiltersSet(s)) return "filters";
  if (highlightTokens(query)) return "tokens";
  return null;
}

/** The corpora searched: the ticks, narrowed to highlights by a highlight filter (keyword only). */
export function effectiveCorpora(s: SearchState, query: string): CorpusId[] {
  return narrowedBy(s, query) ? ["highlights"] : s.corpora;
}

/**
 * Keyword over Highlights alone goes to the highlight index (colours,
 * locations); anything wider, and every semantic search, to the corpus engine.
 */
export function engineFor(s: SearchState, query: string): Engine {
  if (s.mode === "semantic") return "archive";
  const eff = effectiveCorpora(s, query);
  return eff.length === 1 && eff[0] === "highlights" ? "highlights" : "archive";
}

// ---------- group, sort, rows ----------

export function groupOptions(s: SearchState, query: string): GroupBy[] {
  if (engineFor(s, query) === "highlights") return ["work", "author", "date", "tag", "none"];
  const multi = effectiveCorpora(s, query).length > 1;
  return [...(multi ? (["corpus"] as GroupBy[]) : []), "work", "author", "date", "none"];
}

/** "then" applies to the highlight index's groups. */
export function subgroupOptions(s: SearchState, query: string): GroupBy[] {
  if (engineFor(s, query) !== "highlights") return [];
  return ["none", "work", "author", "date", "tag"];
}

/** The default Group: Corpus when more than one corpus is searched, else None. */
export function defaultGroup(s: SearchState, query: string): GroupBy {
  return groupOptions(s, query).includes("corpus") ? "corpus" : "none";
}

export function effectiveGroup(s: SearchState, query: string): GroupBy {
  return groupOptions(s, query).includes(s.group) ? s.group : defaultGroup(s, query);
}

export function effectiveSubgroup(s: SearchState, query: string): GroupBy {
  const g = effectiveGroup(s, query);
  if (g === "none") return "none";
  return subgroupOptions(s, query).includes(s.subgroup) && s.subgroup !== g ? s.subgroup : "none";
}

function next<T>(list: T[], current: T): T {
  const i = list.indexOf(current);
  return list[(i + 1) % list.length];
}

export const cycleGroup = (s: SearchState, query: string): SearchState => ({ ...s, group: next(groupOptions(s, query), effectiveGroup(s, query)) });
export const cycleSort = (s: SearchState): SearchState => ({ ...s, sort: next(SORTS, s.sort) });
export const cycleDensity = (s: SearchState): SearchState => ({ ...s, density: next(DENSITIES, s.density) });

// ---------- the Filters · Group button and the chips ----------

/** Filters set: colour, quick toggles, each type and the time window. */
export function filterCount(s: SearchState): number {
  const f = s.filters;
  return (s.color ? 1 : 0) + (f.favorite ? 1 : 0) + (f.zotero ? 1 : 0) + (f.hasImage ? 1 : 0) + f.types.length + (f.time ? 1 : 0);
}

/** "Filters (1) · Group: Work". */
export function buttonLabel(s: SearchState, query: string): string {
  const n = filterCount(s);
  return `Filters${n ? ` (${n})` : ""} · Group: ${GROUP_LABEL[effectiveGroup(s, query)]}`;
}

export interface Chip {
  id: string;
  label: string;
  /** Styling hint: the colour's swatch, or the violet semantic chip. */
  kind?: "color" | "semantic";
}

/** Every choice not at its default, in the order the "Showing" row lists them. */
export function chips(s: SearchState, query: string): Chip[] {
  const out: Chip[] = [];
  const f = s.filters;
  if (s.color) out.push({ id: "color", label: `Colour: ${s.color}`, kind: "color" });
  const g = effectiveGroup(s, query);
  if (g !== defaultGroup(s, query)) out.push({ id: "group", label: `Group: ${GROUP_LABEL[g]}` });
  const sub = effectiveSubgroup(s, query);
  if (sub !== "none") out.push({ id: "subgroup", label: `then: ${GROUP_LABEL[sub]}` });
  if (s.sort !== "best") out.push({ id: "sort", label: `Sort: ${SORT_LABEL[s.sort]}` });
  if (s.density !== DEFAULT_DENSITY) out.push({ id: "density", label: `Rows: ${DENSITY_LABEL[s.density]}` });
  if (s.partial && s.mode === "keyword") out.push({ id: "partial", label: "Match: partial" });
  if (s.mode === "semantic") out.push({ id: "semantic", label: "Semantic", kind: "semantic" });
  if (f.favorite) out.push({ id: "favorite", label: "★ Favorites" });
  if (f.zotero) out.push({ id: "zotero", label: "Zotero" });
  if (f.hasImage) out.push({ id: "hasImage", label: "Has image" });
  for (const t of f.types) out.push({ id: `type:${t}`, label: `Type: ${TYPE_OPTIONS.find((o) => o.value === t)?.label ?? t}` });
  if (f.time) out.push({ id: "time", label: `Time: ${TIME_OPTIONS.find((o) => o.value === f.time)?.label ?? f.time}` });
  return out;
}

/** Undo one chip (its ×). */
export function clearChip(s: SearchState, id: string): SearchState {
  const f = s.filters;
  switch (id) {
    case "color": return { ...s, color: null };
    case "group": return { ...s, group: "corpus" };
    case "subgroup": return { ...s, subgroup: "none" };
    case "sort": return { ...s, sort: "best" };
    case "density": return { ...s, density: DEFAULT_DENSITY };
    case "partial": return { ...s, partial: false };
    case "semantic": return setMode(s, "keyword");
    case "favorite": return { ...s, filters: { ...f, favorite: false } };
    case "zotero": return { ...s, filters: { ...f, zotero: false } };
    case "hasImage": return { ...s, filters: { ...f, hasImage: false } };
    case "time": return { ...s, filters: { ...f, time: "" } };
  }
  if (id.startsWith("type:")) return { ...s, filters: { ...f, types: f.types.filter((t) => t !== id.slice(5)) } };
  return s;
}

/** "clear all": every chip undone; the ticks stay. */
export function clearAll(s: SearchState, query: string): SearchState {
  // Twice: leaving semantic reveals the Match chip it hid.
  const once = (x: SearchState) => chips(x, query).reduce((acc, c) => clearChip(acc, c.id), x);
  return once(once(s));
}

/** The popover's "Clear all": the filters (colour, quick, type, time), not the view. */
export function clearFilters(s: SearchState): SearchState {
  return { ...s, color: null, filters: { ...EMPTY_FILTERS } };
}

/** Add `tag:"…"` to the query (Classic's tag picker did the same). */
export function withTag(query: string, tag: string): string {
  const t = `tag:"${tag}"`;
  const q = query.trim();
  return q ? `${q} ${t}` : t;
}

// ---------- engine requests ----------

/** Highlight sources to restrict to: [] when every known source is ticked. */
export function tickedSources(s: SearchState, known: string[]): string[] {
  if (!s.offSources.length) return [];
  return known.filter((k) => !s.offSources.includes(k));
}

/** The time window as a corpus-engine token (`after:2026-03-28`), or "". */
export function timeToken(time: string): string {
  const after = scopeToFilters(time).after;
  return after ? `after:${after}` : "";
}

export type HighlightPayload = SearchQueryPayload & { sources: string[] };

/**
 * The highlight index request, or null when nothing can match (the Zotero
 * quick filter with Zotero unticked). The popover filters and the typed
 * tokens combine as in Classic; a typed `source:` is honoured.
 */
export function highlightPayload(s: SearchState, query: string, page: number, pageSize: number, known: string[]): HighlightPayload | null {
  const p = buildSearchQuery({
    raw: query,
    filters: s.filters,
    source: parseSearch(query).source,
    color: s.color,
    sort: CLASSIC_SORT[s.sort],
    mode: s.mode,
    partial: s.partial,
    page,
    pageSize,
  });
  let sources = tickedSources(s, known);
  if (sources.length && p.zotero) {
    if (!sources.includes("zotero")) return null;
    sources = ["zotero"];
  }
  return { ...p, sources };
}

/** Whether the highlight index has anything to run: a query, a filter or a colour. */
export function highlightSearchable(s: SearchState, query: string): boolean {
  return !!query.trim() || highlightFiltersSet(s) || !!s.filters.time;
}

/**
 * The corpus engine requests for the ticked corpora. A partial set of
 * highlight sources is its own request (the engine's `source:` filter would
 * otherwise drop writing and tweets, which have no source); the answers are
 * merged by `mergeResults`.
 */
export function archiveRequests(s: SearchState, query: string, limit: number, known: string[]): ArchiveSearchRequest[] {
  const time = timeToken(s.filters.time);
  const q = [query.trim(), time].filter(Boolean).join(" ");
  const eff = effectiveCorpora(s, query);
  const mode = requestMode(s);
  const sources = eff.includes("highlights") ? tickedSources(s, known) : [];
  if (!sources.length) {
    const all = CORPUS_ORDER.every((c) => eff.includes(c));
    return [{ query: q, in: all ? [] : eff, limit, mode }];
  }
  const rest = eff.filter((c) => c !== "highlights");
  return [
    ...(rest.length ? [{ query: q, in: rest, limit, mode }] : []),
    { query: `${q} source:${sources.join(",")}`, in: ["highlights"], limit, mode },
  ];
}

/** One answer from several requests: results in request order, totals summed. */
export function mergeResults(list: ArchiveSearchResults[]): ArchiveSearchResults | null {
  if (!list.length) return null;
  if (list.length === 1) return list[0];
  return {
    ...list[0],
    corpora: [...new Set(list.flatMap((r) => r.corpora))],
    total_documents: list.reduce((n, r) => n + r.total_documents, 0),
    total_passages: list.reduce((n, r) => n + r.total_passages, 0),
    results: list.flatMap((r) => r.results),
  };
}

/** The line at the right of the chips row. */
export function scopeNote(s: SearchState, query: string): string | null {
  if (s.mode === "semantic") return highlightFiltersSet(s) ? "Semantic search ignores the highlight filters (colour, quick filters, type)" : null;
  const why = narrowedBy(s, query);
  if (why === "filters" || why === "tokens") return "Highlight filters narrow the results to highlights · ⌘⇧I edits";
  if (engineFor(s, query) === "highlights") return "Highlights only · ⌘⇧I edits";
  return null;
}

/** What the search box searches, for its placeholder. */
export function scopeWords(corpora: CorpusId[]): string {
  const words = corpora.map((c) => (CORPUS_LABEL[c] ?? c).toLowerCase());
  return words.length <= 1 ? words.join("") : `${words.slice(0, -1).join(", ")} and ${words[words.length - 1]}`;
}

// ---------- persistence ----------

export type PrefStore = Pick<Storage, "getItem" | "setItem">;

const VIEW_KEY = "search.view";
const GROUPS: GroupBy[] = ["corpus", "work", "author", "date", "tag", "none"];
const CORPORA: CorpusId[] = [...CORPUS_ORDER];

function pick<T extends string>(v: unknown, valid: readonly T[], fallback: T): T {
  return typeof v === "string" && (valid as readonly string[]).includes(v) ? (v as T) : fallback;
}

/**
 * The view choices remembered across launches (ticks, group, sort, rows,
 * match). The filters keep Classic's own short-lived store; colour and
 * the Keyword / Semantic switch start fresh. The old per-search keys ("group", "density", …) are
 * not read: both searches wrote their defaults on every launch, so they
 * record no choice.
 */
export function loadView(store: PrefStore, filters: Filters = EMPTY_FILTERS): SearchState {
  let v: Record<string, unknown> = {};
  try {
    v = JSON.parse(store.getItem(VIEW_KEY) || "{}") ?? {};
  } catch {
    v = {};
  }
  const corpora = Array.isArray(v.corpora) ? inOrder((v.corpora as unknown[]).filter((c): c is CorpusId => CORPORA.includes(c as CorpusId))) : [];
  const offSources = Array.isArray(v.offSources) ? (v.offSources as unknown[]).filter((x): x is string => typeof x === "string") : [];
  return {
    ...DEFAULT_STATE,
    corpora: corpora.length ? corpora : [...CORPUS_ORDER],
    offSources: corpora.includes("highlights") ? offSources : [],
    group: pick(v.group, GROUPS, "corpus"),
    subgroup: pick(v.subgroup, GROUPS, "none"),
    sort: pick(v.sort, SORTS, "best"),
    density: pick(v.density, DENSITIES, DEFAULT_DENSITY),
    partial: v.partial === true,
    filters,
  };
}

export function saveView(s: SearchState, store: PrefStore): void {
  const view = { corpora: s.corpora, offSources: s.offSources, group: s.group, subgroup: s.subgroup, sort: s.sort, density: s.density, partial: s.partial };
  try { store.setItem(VIEW_KEY, JSON.stringify(view)); } catch { /* a convenience only */ }
}
