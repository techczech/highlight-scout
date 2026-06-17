export interface SearchResult {
  highlight_id: string;
  work_id: string;
  text: string;
  note: string | null;
  title: string;
  author: string | null;
  work_type: string;
  source_system: string;
  url: string | null;
  highlighted_at: string | null;
  tags: string[];
  annotation_color: string | null;
  snippet: string;
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

export interface Facets {
  sources: string[];
  colors: string[];
}

export type SearchMode = "keyword" | "semantic";

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
