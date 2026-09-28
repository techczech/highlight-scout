// Archive search over the Scout corpora (writing, tweets, highlights) via the
// scout-corpus facade. Shapes mirror `scout … --json` (scout-core
// docs/cli-json.md, schema_version 1); the backend returns them unchanged.
import { invoke } from "@tauri-apps/api/core";

export interface ArchiveHit {
  passage_id: string;
  line_start: number;
  line_end: number;
  line: number;
  /** Original source text of the hit sentence (or passage). */
  quote: string;
  score: number;
  link: string | null;
  tags?: string[];
  color?: string;
  saved_at?: string;
}

export interface Citation {
  markdown: string;
  plain: string;
}

export interface ArchiveDoc {
  corpus: string;
  rel_path: string;
  path: string;
  title: string;
  author: string | null;
  date: string | null;
  date_display: string | null;
  date_source?: string;
  genre?: string | null;
  lang?: string | null;
  kind?: string;
  source?: string;
  public_url: string | null;
  score: number;
  rank: number;
  title_match: boolean;
  passage_count: number;
  hits: ArchiveHit[];
  citation: Citation;
}

export interface ArchiveSearchResults {
  schema_version: number;
  query: string;
  corpora: string[];
  total_documents: number;
  total_passages: number;
  results: ArchiveDoc[];
}

export interface CitedPassage {
  schema_version: number;
  passage_id: string;
  corpus: string;
  rel_path: string;
  /** Absolute source file. */
  path: string;
  line_start: number;
  line_end: number;
  /** The whole passage, original text. */
  quote: string;
  title: string;
  author: string | null;
  date: string | null;
  date_display: string | null;
  date_source?: string;
  link: string | null;
  public_url: string | null;
  citation: Citation;
}

export interface Answer<T> {
  body: T;
  /** The CLI's stderr notes, e.g. "scout: note: using writing; not indexed: tweets". */
  notes: string[];
}

export interface CorpusError {
  kind: "registry_missing" | "index_missing" | "no_indexed_corpus" | "passage_not_found" | "other";
  message: string;
}

export interface IndexStatus {
  corpus: string;
  kind: string;
  index_path: string;
  exists: boolean;
  docs: number;
  passages: number;
  tokens: number;
  built_at: string | null;
  config_current: boolean;
  stale: { added: number; changed: number; removed: number } | null;
}

export interface IndexJob {
  phase: "idle" | "checking" | "building" | "current" | "built" | "failed" | "unavailable";
  corpora: string[];
  builds: Array<{ corpus: string; reason: string; error: string | null }>;
  message: string;
}

export interface CorpusIndexState {
  status: { schema_version: number; corpora: IndexStatus[] } | null;
  job: IndexJob;
  running: boolean;
}

export interface ArchiveSearchRequest {
  query: string;
  /** Corpus ids; empty = every indexed corpus. */
  in?: string[];
  limit?: number;
  passage?: boolean;
}

export function archiveSearch(query: ArchiveSearchRequest): Promise<Answer<ArchiveSearchResults>> {
  return invoke<Answer<ArchiveSearchResults>>("corpus_search", { query });
}

export function archiveCite(passageId: string): Promise<Answer<CitedPassage>> {
  return invoke<Answer<CitedPassage>>("corpus_cite", { passageId });
}

export function archiveIndexStatus(): Promise<CorpusIndexState> {
  return invoke<CorpusIndexState>("corpus_index_status");
}

/** Start a background check + incremental build; progress comes as `corpus:index`. */
export function archiveIndexRefresh(): Promise<IndexJob> {
  return invoke<IndexJob>("corpus_index_refresh");
}

const BADGES: Record<string, string> = {
  writing: "Writing",
  tweets: "Tweet",
  highlights: "Highlight",
};

/** The corpus badge shown first on every archive result. */
export function corpusBadge(corpus: string): string {
  return BADGES[corpus] ?? corpus.charAt(0).toUpperCase() + corpus.slice(1);
}

export const BADGE_CLASS: Record<string, string> = {
  writing: "bg-emerald-100 text-emerald-800",
  tweets: "bg-sky-100 text-sky-800",
  highlights: "bg-amber-100 text-amber-800",
};

/** Normalise an invoke rejection into a CorpusError. */
export function toCorpusError(e: unknown): CorpusError {
  if (e && typeof e === "object" && "kind" in e && "message" in e) return e as CorpusError;
  return { kind: "other", message: e instanceof Error ? e.message : String(e) };
}

/** Whether an index build could fix this error. */
export function wantsIndex(e: CorpusError): boolean {
  return e.kind === "index_missing" || e.kind === "no_indexed_corpus";
}

/** What the user reads for an error, with what to do about it. */
export function errorLine(e: CorpusError): string {
  switch (e.kind) {
    case "registry_missing":
      return "Archive search needs the Scout corpus registry. Run `scout corpora init-defaults` once.";
    case "index_missing":
    case "no_indexed_corpus":
      return "The archive index is not built yet — building it in the background…";
    default:
      return `Archive search failed: ${e.message}`;
  }
}

/** Status-bar summary of one search: counts and the corpora searched. */
export function resultSummary(r: ArchiveSearchResults): string {
  const docs = `${r.total_documents.toLocaleString()} document${r.total_documents === 1 ? "" : "s"}`;
  const pass = `${r.total_passages.toLocaleString()} passage${r.total_passages === 1 ? "" : "s"}`;
  const shown = r.results.length < r.total_documents ? ` (top ${r.results.length})` : "";
  return `${docs}${shown} · ${pass} · ${r.corpora.map(corpusBadge).join(", ")}`;
}

/** Turn a scout note into a user-facing line ("scout: note: " stripped). */
export function noteLine(note: string): string {
  return note.replace(/^scout:\s*note:\s*/, "");
}

/** Whether an index job just finished building something (re-run the search). */
export function jobBuilt(job: IndexJob): boolean {
  return job.phase === "built" || (job.phase === "failed" && job.builds.some((b) => !b.error));
}

/** `path:line` for display. */
export function sourceLine(p: { path: string; line_start: number; line_end: number }): string {
  const lines = p.line_end > p.line_start ? `${p.line_start}–${p.line_end}` : `${p.line_start}`;
  return `${p.path}:${lines}`;
}

/** Index-keeper phases worth a status-bar line (a quiet "all current" is not). */
export function jobVisible(job: IndexJob): boolean {
  return job.phase === "building" || job.phase === "built" || job.phase === "failed" || job.phase === "unavailable";
}
