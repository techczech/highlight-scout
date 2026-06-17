import { invoke } from "@tauri-apps/api/core";
import type {
  SearchPage,
  SearchResult,
  ImportStatus,
  ImportLogEntry,
  Stats,
  Config,
  Settings,
  Facets,
  TagCount,
  WorkPosition,
} from "../types";
import type { SearchQueryPayload } from "./query";

export async function searchQuery(query: SearchQueryPayload): Promise<SearchPage> {
  return invoke<SearchPage>("search_query", { query });
}

export async function semanticSearch(query: string): Promise<SearchResult[]> {
  return invoke<SearchResult[]>("semantic_search", { query });
}

export async function findRelated(text: string, excludeId: string): Promise<SearchResult[]> {
  return invoke<SearchResult[]>("find_related", { text, excludeId });
}

export async function getHighlight(id: string): Promise<SearchResult | null> {
  return invoke<SearchResult | null>("get_highlight", { id });
}

export async function qmdReindex(): Promise<string> {
  return invoke<string>("qmd_reindex");
}

export async function qmdAvailable(): Promise<boolean> {
  return invoke<boolean>("qmd_available");
}

export async function workHighlights(workId: string): Promise<SearchResult[]> {
  return invoke<SearchResult[]>("work_highlights", { workId });
}

export async function highlightPosition(
  workId: string,
  location: string
): Promise<WorkPosition | null> {
  return invoke<WorkPosition | null>("highlight_position", { workId, location });
}

export async function listTags(): Promise<TagCount[]> {
  return invoke<TagCount[]>("list_tags");
}

export async function runImport(): Promise<ImportStatus> {
  return invoke<ImportStatus>("run_import");
}

export async function runReadwiseSeed(): Promise<ImportStatus> {
  return invoke<ImportStatus>("run_readwise_seed");
}

export interface CsvInspect {
  headers: string[];
  sample_rows: string[][];
  delimiter: string;
}

export interface CsvMapping {
  text: string | null;
  title: string | null;
  author: string | null;
  note: string | null;
  date: string | null;
  location: string | null;
  tags: string | null;
  url: string | null;
  color: string | null;
  delimiter: string;
}

export async function inspectCsv(path: string): Promise<CsvInspect> {
  return invoke<CsvInspect>("inspect_csv", { path });
}

export async function importCsv(path: string, mapping: CsvMapping): Promise<ImportStatus> {
  return invoke<ImportStatus>("import_csv", { path, mapping });
}

export async function importKindle(path: string): Promise<ImportStatus> {
  return invoke<ImportStatus>("import_kindle", { path });
}

export async function importJson(path: string): Promise<ImportStatus> {
  return invoke<ImportStatus>("import_json", { path });
}

export async function exportJson(path: string): Promise<number> {
  return invoke<number>("export_json", { path });
}

export async function runZoteroImport(): Promise<ImportStatus> {
  return invoke<ImportStatus>("run_zotero_import");
}

export async function getStats(): Promise<Stats> {
  return invoke<Stats>("get_stats");
}

export async function getImportLog(): Promise<ImportLogEntry[]> {
  return invoke<ImportLogEntry[]>("get_import_log");
}

export async function getFacets(): Promise<Facets> {
  return invoke<Facets>("get_facets");
}

export async function getConfig(): Promise<Config> {
  return invoke<Config>("get_config");
}

export async function getSettings(): Promise<Settings> {
  return invoke<Settings>("get_settings");
}

export async function saveSettings(settings: Settings): Promise<void> {
  return invoke("save_settings", { settings });
}
