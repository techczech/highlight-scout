// The quick finder (HS-1B): which corpus the rail selects, results grouped
// Writing / Tweets / Highlights, keyboard order across the groups, recent
// searches, and which clipboard format ⌘⇧C uses. Pure: no Tauri, no React.
// The search, the citation text and the counts all come from the engine.
import type { ArchiveDoc, ArchiveSearchRequest, ArchiveSearchResults } from "./archive";

export type CorpusId = "writing" | "tweets" | "highlights";

/** What the rail selects: every corpus, one corpus, or one highlight source. */
export interface CorpusFilter {
  corpus: "all" | CorpusId;
  /** A highlights source system (`x`, `readwise`, `zotero`); only with corpus = highlights. */
  source?: string;
}

export const ALL: CorpusFilter = { corpus: "all" };

export const CORPUS_ORDER: CorpusId[] = ["writing", "tweets", "highlights"];

export const CORPUS_LABEL: Record<string, string> = {
  writing: "Writing",
  tweets: "Tweets",
  highlights: "Highlights",
};

/** What a corpus's documents are called in counts. */
export const CORPUS_UNIT: Record<string, string> = {
  writing: "pieces",
  tweets: "tweets",
  highlights: "works",
};

export const SOURCE_LABEL: Record<string, string> = { x: "X", readwise: "Readwise", zotero: "Zotero" };
const SOURCE_ORDER = ["x", "readwise", "zotero"];

/** Highlight sources in rail order: X, Readwise, Zotero, then any other. */
export function sourceOrder(sources: Record<string, number>): string[] {
  const known = SOURCE_ORDER.filter((s) => s in sources);
  const rest = Object.keys(sources).filter((s) => !SOURCE_ORDER.includes(s)).sort();
  return [...known, ...rest];
}

export function sameFilter(a: CorpusFilter, b: CorpusFilter): boolean {
  return a.corpus === b.corpus && (a.source ?? "") === (b.source ?? "");
}

/**
 * The engine request for a query under a rail selection. A source uses the
 * engine's own `source:` field, so the engine does the filtering.
 */
export function requestFor(query: string, filter: CorpusFilter, limit: number): ArchiveSearchRequest {
  const q = filter.corpus === "highlights" && filter.source ? `${query} source:${filter.source}` : query;
  return { query: q, in: filter.corpus === "all" ? [] : [filter.corpus], limit };
}

export type ResultSort = "best" | "newest" | "oldest";
export const SORT_LABEL: Record<ResultSort, string> = { best: "Best matches", newest: "Newest first", oldest: "Oldest first" };

/** Rows per group while "All corpora" is selected; "show all" opens the corpus. */
export const GROUP_PREVIEW = 5;

export interface ResultGroup {
  corpus: string;
  label: string;
  docs: ArchiveDoc[];
  /** Documents of this corpus among the fetched results. */
  fetched: number;
  /** More of this corpus than shown, or than fetched. */
  more: boolean;
}

export function docKey(d: ArchiveDoc): string {
  return `${d.corpus}:${d.rel_path}`;
}

function byDate(dir: 1 | -1) {
  return (a: ArchiveDoc, b: ArchiveDoc) => {
    if (!a.date && !b.date) return 0;
    if (!a.date) return 1; // undated last either way
    if (!b.date) return -1;
    return a.date < b.date ? -dir : a.date > b.date ? dir : 0;
  };
}

/**
 * Results grouped by corpus, Writing first. Under "All corpora" each group
 * shows its first rows; under one corpus, its group shows every fetched row.
 */
export function groupResults(
  results: ArchiveSearchResults | null,
  filter: CorpusFilter,
  sort: ResultSort = "best",
  preview = GROUP_PREVIEW,
): ResultGroup[] {
  if (!results) return [];
  const truncated = results.results.length < results.total_documents;
  const by = new Map<string, ArchiveDoc[]>();
  for (const d of results.results) {
    const list = by.get(d.corpus) ?? [];
    list.push(d);
    by.set(d.corpus, list);
  }
  const order = [...CORPUS_ORDER.filter((c) => by.has(c)), ...[...by.keys()].filter((c) => !CORPUS_ORDER.includes(c as CorpusId)).sort()];
  return order.map((corpus) => {
    let docs = by.get(corpus)!;
    if (sort !== "best") docs = [...docs].sort(byDate(sort === "newest" ? -1 : 1));
    const all = filter.corpus !== "all";
    const shown = all ? docs : docs.slice(0, preview);
    return {
      corpus,
      label: CORPUS_LABEL[corpus] ?? corpus,
      docs: shown,
      fetched: docs.length,
      more: !all && (docs.length > shown.length || truncated),
    };
  });
}

/** Every visible row, top to bottom: the order ↑↓ moves through. */
export function visibleKeys(groups: ResultGroup[]): string[] {
  return groups.flatMap((g) => g.docs.map(docKey));
}

/** ↑↓: the row `delta` away from the active one, clamped. */
export function moveKey(groups: ResultGroup[], active: string | null, delta: number): string | null {
  const keys = visibleKeys(groups);
  if (!keys.length) return null;
  const i = active ? keys.indexOf(active) : -1;
  return keys[Math.max(0, Math.min(keys.length - 1, (i < 0 ? 0 : i) + delta))];
}

/** ⌥↓ / ⌥↑: the first row of the next (or previous) corpus group; wraps. */
export function groupKey(groups: ResultGroup[], active: string | null, dir: 1 | -1): string | null {
  const live = groups.filter((g) => g.docs.length);
  if (!live.length) return null;
  const at = live.findIndex((g) => g.docs.some((d) => docKey(d) === active));
  const next = at < 0 ? 0 : (at + dir + live.length) % live.length;
  return docKey(live[next].docs[0]);
}

/** The group count as the header shows it: "12", or "12+" when there may be more. */
export function groupCount(g: ResultGroup, results: ArchiveSearchResults | null): string {
  const truncated = !!results && results.results.length < results.total_documents;
  return `${g.fetched.toLocaleString()}${truncated ? "+" : ""}`;
}

// ---------- copy ----------

export const WRITEFLEX_BUNDLE = "net.dominiklukes.writeflex";

/** The ⌘⇧C format setting: Auto = Markdown in WriteFlex, rich text elsewhere. */
export type CopyFormat = "auto" | "markdown" | "rich";
export const COPY_FORMAT_LABEL: Record<CopyFormat, string> = {
  auto: "Auto: Markdown in WriteFlex, rich text elsewhere",
  markdown: "Always Markdown",
  rich: "Always rich text",
};

/** What ⌘⇧C writes, given the setting and the app the user came from. */
export function resolveCopyFormat(setting: CopyFormat, frontBundle: string | null): "markdown" | "rich" {
  if (setting !== "auto") return setting;
  return frontBundle === WRITEFLEX_BUNDLE ? "markdown" : "rich";
}

/** A name for the app Esc returns to, for "esc back to WriteFlex". */
export function appName(bundle: string | null): string | null {
  if (!bundle) return null;
  if (bundle === WRITEFLEX_BUNDLE) return "WriteFlex";
  const last = bundle.split(".").pop() ?? "";
  return last ? last.charAt(0).toUpperCase() + last.slice(1) : null;
}

/** The link the Link action copies: the public URL, else the archive link. */
export function copyLinkFor(p: { public_url: string | null; link: string | null }): string | null {
  return p.public_url || p.link || null;
}

// ---------- recent searches ----------

export const RECENT_MAX = 8;
const RECENT_KEY = "quickFinder.recent";

/** Add a query to the front of the recent list (trimmed, no duplicates). */
export function pushRecent(list: string[], query: string, max = RECENT_MAX): string[] {
  const q = query.trim();
  if (q.length < 2) return list;
  return [q, ...list.filter((x) => x.toLowerCase() !== q.toLowerCase())].slice(0, max);
}

export function loadRecent(): string[] {
  try {
    const v = JSON.parse(localStorage.getItem(RECENT_KEY) || "[]");
    return Array.isArray(v) ? v.filter((x) => typeof x === "string").slice(0, RECENT_MAX) : [];
  } catch {
    return [];
  }
}

export function saveRecent(list: string[]): void {
  try {
    localStorage.setItem(RECENT_KEY, JSON.stringify(list));
  } catch {
    /* storage unavailable: recent searches are a convenience */
  }
}

// ---------- rows and the reading pane ----------

/** The grey meta line after the badge: date · genre (writing), date · @handle (tweets), source · author (highlights). */
export function rowMeta(d: ArchiveDoc): { by?: string; parts: string[] } {
  const date = d.date ? d.date.slice(0, 10) : null;
  if (d.corpus === "highlights") {
    const src = d.source ? SOURCE_LABEL[d.source] ?? d.source : null;
    return { by: d.author ?? undefined, parts: [src, d.title].filter(Boolean) as string[] };
  }
  if (d.corpus === "tweets") {
    const handle = d.public_url?.match(/x\.com\/([^/]+)\/status\//)?.[1] ?? null;
    return { parts: [date, handle ? `@${handle}` : null].filter(Boolean) as string[] };
  }
  return { parts: [date, d.genre ?? null].filter(Boolean) as string[] };
}

/** Whether a row shows a title line (writing pieces have titles; tweets and highlights lead with the text). */
export function rowHasTitle(d: ArchiveDoc): boolean {
  return d.corpus !== "tweets" && d.corpus !== "highlights";
}

/**
 * Split a passage around its hit sentence, so the pane can mark the
 * sentence the search matched. Whole passage as `before` when the sentence
 * is not found verbatim.
 */
export function splitAroundSentence(passage: string, sentence: string | null | undefined): { before: string; sentence: string; after: string } {
  const s = (sentence ?? "").trim();
  const i = s ? passage.indexOf(s) : -1;
  if (i < 0 || s === passage.trim()) return { before: "", sentence: passage, after: "" };
  return { before: passage.slice(0, i), sentence: s, after: passage.slice(i + s.length) };
}

/** Byline for the pane: "Dominik Lukeš · 23 June 2016 · essay". */
export function paneByline(d: ArchiveDoc): string {
  const date = d.date_display ? (d.date_source === "saved" ? `saved ${d.date_display}` : d.date_display) : null;
  return [d.author, date, d.corpus === "writing" ? d.genre : d.corpus === "tweets" ? "tweet" : d.kind].filter(Boolean).join(" · ");
}

/** The footer's corpus sizes: "Writing 1,506 pieces · Tweets 14,892 · Highlights 14,724 works". */
export function countsLine(counts: Array<{ corpus: string; docs: number; indexed: boolean }>): string {
  return counts
    .filter((c) => c.indexed)
    .map((c) => {
      const label = CORPUS_LABEL[c.corpus] ?? c.corpus;
      const unit = c.corpus === "tweets" ? "" : ` ${CORPUS_UNIT[c.corpus] ?? "documents"}`;
      return `${label} ${c.docs.toLocaleString()}${unit}`;
    })
    .join(" · ");
}
