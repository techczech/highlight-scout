// "Analyse in ArchiveScout" (HS-5A): the hand-off is ArchiveScout's own
// URL scheme, `archivescout://explore?mode=kwic&q=<term>&in=<corpora>&…`
// (archive-scout `parse_explore_url`, src-tauri/src/commands/lab_kwic.rs),
// which opens its concordance on the same term, corpora and filters.
//
// The link is built from the search the app actually runs: the query parsed
// by `@scout/query` (the parser behind every search here) and the popover's
// state, read the way `archiveRequests` sends them to the engine. What the
// concordance can take goes along (`in`, `source`, `after`, `before`, `y`,
// `lang`, `genre`); what it cannot (OR, more words, exclusions, regexes,
// highlight-only filters, highlight sources beside other corpora) is named in `notCarried`, for the button's tooltip,
// never dropped silently. Pure: no Tauri, no React.
import { dateRange, parseSearch, scopeToFilters } from "@scout/query";
import type { CorpusId } from "./quickFinder";
import { effectiveCorpora, tickedSources, type SearchState } from "./searchModel";

export const ARCHIVESCOUT_RELEASES = "https://github.com/techczech/archive-scout/releases";

export interface AnalyseLink {
  /** The link, or null when the query leaves nothing to look up. */
  url: string | null;
  /** The concordance's term: the search's first word or phrase. */
  term: string;
  /** The parts of the search the concordance cannot take, in the user's words. */
  notCarried: string[];
}

const YEAR = /^\d{4}$/;

/** The ArchiveScout link for the search the app is running. */
export function analyseLink(s: SearchState, query: string, known: string[]): AnalyseLink {
  const p = parseSearch(query, s.partial);
  const eff = effectiveCorpora(s, query);
  const notCarried: string[] = [];
  const [term = "", ...rest] = p.positive_terms;

  // The concordance looks up one word or phrase.
  if (p.fts.includes(" OR ")) notCarried.push("OR");
  if (rest.length) notCarried.push(`more words (${rest.join(", ")})`);
  if (p.negatives.length) notCarried.push(`excluded words (${p.negatives.map((n) => `-${n}`).join(" ")})`);
  if (p.regexes.length) notCarried.push("regex");
  if (s.partial && s.mode === "keyword") notCarried.push("partial match");
  if (s.mode === "semantic") notCarried.push("meaning search");
  // Highlight-only filters, typed or in the popover.
  if (p.author) notCarried.push("author");
  if (p.title) notCarried.push("title");
  if (p.type || s.filters.types.length) notCarried.push("type");
  if (p.tag) notCarried.push("tag");
  if (p.color || s.color) notCarried.push("colour");
  if (p.favorite || s.filters.favorite) notCarried.push("favourites");
  if (p.has_image || s.filters.hasImage) notCarried.push("has image");

  const q = new URLSearchParams();
  q.set("mode", "kwic");
  q.set("q", term);
  const corpora = p.in ? p.in.split(",").map((c) => c.trim()).filter(Boolean) : eff;
  if (corpora.length) q.set("in", corpora.join(","));

  // Sources. A typed source: is in the query the engine runs over every
  // corpus, so it goes along as it is. The Zotero filter and the ticked
  // highlight sources narrow the highlights only (`archiveRequests` sends
  // them with the highlights alone); ArchiveScout's source= would narrow
  // every corpus, so they go along only when the highlights are all that is
  // searched, and are named as not carried over otherwise.
  const zotero = p.zotero || s.filters.zotero;
  const highlightSources = zotero ? ["zotero"] : corpora.includes("highlights") ? tickedSources(s, known) : [];
  const highlightsOnly = corpora.length === 1 && corpora[0] === "highlights";
  if (p.source) q.set("source", p.source);
  else if (highlightSources.length && highlightsOnly) q.set("source", highlightSources.join(","));
  else if (highlightSources.length) notCarried.push(`highlight sources (only ${highlightSources.join(", ")})`);

  // Dates, as the engine reads them (scout-corpus `apply_query_fields`); the
  // popover's time window is sent after the query, so it wins over a typed after:.
  let after = p.after;
  let before = p.before;
  let year: string | null = null;
  if (p.date) {
    const span = p.date.match(/^(\d{4})-(\d{4})$/);
    if (YEAR.test(p.date)) year = p.date;
    else if (p.after || p.before) notCarried.push(`y:${p.date}`);
    else if (span && Number(span[2]) >= Number(span[1])) {
      after = span[1];
      before = String(Number(span[2]) + 1);
    } else {
      const r = dateRange(p.date);
      if (r) { after = r.start ?? null; before = r.end ?? null; }
      else notCarried.push(`y:${p.date}`);
    }
  }
  const time = scopeToFilters(s.filters.time).after;
  if (time) after = time;
  if (after) q.set("after", after);
  if (before) q.set("before", before);
  if (year) q.set("y", year);
  if (p.lang) q.set("lang", p.lang);
  if (p.genre) q.set("genre", p.genre);

  if (!term) return { url: null, term, notCarried };
  // URLSearchParams writes spaces as "+", which ArchiveScout reads as spaces too; keep %20 for plain links.
  return { url: `archivescout://explore?${q.toString().replace(/\+/g, "%20").replace(/%2C/g, ",")}`, term, notCarried };
}

/** "30 pieces · 64 matches" beside the Analyse button: the real counts. */
export function resultCountLine(r: { total_documents: number; total_passages: number } | null, unit = "pieces"): string {
  if (!r) return "";
  const n = (x: number, one: string, many: string) => `${x.toLocaleString()} ${x === 1 ? one : many}`;
  return `${n(r.total_documents, unit.replace(/s$/, ""), unit)} · ${n(r.total_passages, "match", "matches")}`;
}

/** The button's tooltip: what opens, and what of the search does not go along. */
export function analyseTitle(term: string, corpora: CorpusId[], notCarried: string[] = []): string {
  const names: Record<string, string> = { writing: "Writing", tweets: "Tweets", highlights: "Highlights" };
  const where = corpora.length === 3 || corpora.length === 0 ? "all corpora" : corpora.map((c) => names[c] ?? c).join(", ");
  const base = `Opens ArchiveScout on “${term}” in ${where} · concordance, collocates, n-grams`;
  return notCarried.length ? `${base} · filters not carried over: ${notCarried.join(", ")}` : base;
}
