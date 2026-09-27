export interface SearchResult {
  highlight_id: string;
  work_id: string;
  slug: string;
  text: string;
  note: string | null;
  title: string;
  author: string | null;
  authors: string[];
  work_type: string;
  source_system: string;
  source_id: string | null;
  url: string | null;
  highlighted_at: string | null;
  tags: string[];
  location: string | null;
  annotation_color: string | null;
  annotation_type: string | null;
  format: string;
  asset_path: string | null;
  citation: string | null;
  collections: string[];
  zotero_link: string | null;
  relevance: number | null;
  snippet: string;
  ocr_text: string | null;
}

export interface SearchPage {
  rows: SearchResult[];
  has_more: boolean;
}

export type { RegexFilter, SearchMode, SortMode } from "@scout/query";

export interface TagCount {
  tag: string;
  count: number;
}

export interface WorkPosition {
  pos: number;
  total: number;
  max_loc: number;
}

export interface ImportStatus {
  works_imported: number;
  highlights_imported: number;
  message: string;
}

export interface Stats {
  highlights: number;
  works: number;
}

export interface Config {
  archive_path: string;
  has_api_key: boolean;
  shortcut: string;
  zotero_db_path: string;
}

export interface Settings {
  readwise_api_key: string;
  archive_path: string;
  zotero_db_path: string;
  shortcut: string;
  result_limit: number;
  import_reminder_days: number;
  sync_on_launch: boolean;
  /** While the app runs, sync all sources every N hours. 0 = off. */
  sync_interval_hours: number;
  autostart_enabled: boolean;
  ocr_on_import: boolean;
  r2_enabled: boolean;
  r2_account_id: string;
  r2_endpoint: string;
  r2_bucket: string;
  r2_prefix: string;
  r2_has_credentials: boolean;
}

export interface R2ActionStatus {
  ok: boolean;
  message: string;
  uploaded: number;
  downloaded: number;
  skipped: number;
  failed: number;
}

export interface Facets {
  sources: string[];
  colors: string[];
}

export interface ImportLogEntry {
  timestamp: string;
  source: string;
  works: number;
  highlights: number;
  status: string;
  message: string;
  duration_ms: number;
}

export type GroupMode = "work" | "author" | "date" | "tag" | "none";
export type Density = "minimal" | "compact" | "comfortable" | "full";

export const COLOR_MAP: Record<string, string> = {
  red: "#ef4444",
  green: "#22c55e",
  blue: "#3b82f6",
  yellow: "#eab308",
  orange: "#f97316",
  purple: "#a855f7",
  magenta: "#d946ef",
  gray: "#9ca3af",
};

/**
 * Resolve an annotation_color value to a CSS colour. Zotero custom colours are
 * stored as hex (e.g. "#fff066") and pass through; named colours map via COLOR_MAP.
 */
export function resolveColor(value: string | null): string | null {
  if (!value) return null;
  if (value.startsWith("#")) return value;
  return COLOR_MAP[value] ?? "#9ca3af";
}

export interface SyncSourceResult {
  source: string;
  label: string;
  added: number;
  error: string | null;
  finished_at: string;
}

export interface SyncReport {
  seq: number;
  trigger: "launch" | "interval" | "manual";
  started_at: string;
  finished_at: string;
  results: SyncSourceResult[];
  summary: string;
}

export interface SyncSourceStatus {
  key: string;
  label: string;
  configured: boolean;
  last_synced_at: string | null;
  last_error: string | null;
}

export interface SyncStatus {
  running: boolean;
  last_report: SyncReport | null;
  sources: SyncSourceStatus[];
}
