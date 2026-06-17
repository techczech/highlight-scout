import { invoke } from "@tauri-apps/api/core";
import type {
  SearchPage,
  SearchResult,
  ImportStatus,
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

export async function runZoteroImport(): Promise<ImportStatus> {
  return invoke<ImportStatus>("run_zotero_import");
}

export async function getStats(): Promise<Stats> {
  return invoke<Stats>("get_stats");
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
