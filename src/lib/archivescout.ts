// "Analyse in ArchiveScout" (HS-5A): the hand-off is ArchiveScout's own
// URL scheme, `archivescout://explore?mode=kwic&q=<term>&in=<corpora>`
// (archive-scout corpus-lab ticket 02), which opens its concordance on the
// same term and corpora. Pure: no Tauri, no React.
import type { CorpusId } from "./quickFinder";

export const ARCHIVESCOUT_RELEASES = "https://github.com/techczech/archive-scout/releases";

/** The word ArchiveScout's concordance is searched for: the query without its operators. */
export function analyseTerm(query: string): string {
  const words: string[] = [];
  for (const m of query.matchAll(/(-?)"([^"]*)"|(\S+)/g)) {
    if (m[2] !== undefined) {
      // A phrase keeps its words together; a leading minus excludes it.
      if (!m[1]) words.push(m[2].trim());
      continue;
    }
    const w = m[3];
    if (w === "OR" || w === "AND" || w === "NOT") continue;
    if (w.startsWith("-") || w.startsWith("/") || /^[a-z]{1,4}:/i.test(w)) continue;
    words.push(w.replace(/[()"]/g, "").replace(/\*$/, ""));
  }
  return words.filter(Boolean).join(" ").trim();
}

export interface AnalyseSearch {
  query: string;
  /** The ticked corpora. */
  corpora: CorpusId[];
  /** Highlight sources when some are unticked; empty = all. */
  sources?: string[];
}

/** The link, or null when the query leaves nothing to look up. */
export function analyseUrl(s: AnalyseSearch): string | null {
  const term = analyseTerm(s.query);
  if (!term) return null;
  const p = new URLSearchParams();
  p.set("mode", "kwic");
  p.set("q", term);
  if (s.corpora.length) p.set("in", s.corpora.join(","));
  if (s.sources?.length && s.corpora.includes("highlights")) p.set("source", s.sources.join(","));
  // URLSearchParams writes spaces as "+", which ArchiveScout reads as spaces too; keep %20 for plain links.
  return `archivescout://explore?${p.toString().replace(/\+/g, "%20").replace(/%2C/g, ",")}`;
}

/** "30 pieces · 64 matches" beside the Analyse button: the real counts. */
export function resultCountLine(r: { total_documents: number; total_passages: number } | null, unit = "pieces"): string {
  if (!r) return "";
  const n = (x: number, one: string, many: string) => `${x.toLocaleString()} ${x === 1 ? one : many}`;
  return `${n(r.total_documents, unit.replace(/s$/, ""), unit)} · ${n(r.total_passages, "match", "matches")}`;
}

/** The button's tooltip. */
export function analyseTitle(term: string, corpora: CorpusId[]): string {
  const names: Record<string, string> = { writing: "Writing", tweets: "Tweets", highlights: "Highlights" };
  const where = corpora.length === 3 || corpora.length === 0 ? "all corpora" : corpora.map((c) => names[c] ?? c).join(", ");
  return `Opens ArchiveScout on “${term}” in ${where} · concordance, collocates, n-grams`;
}
