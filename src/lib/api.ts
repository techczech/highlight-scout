import { invoke } from "@tauri-apps/api/core";
import type { SearchResult, ImportStatus, Stats, Config, Facets } from "../types";

export interface SearchParams {
  query: string;
  source?: string | null;
  color?: string | null;
  mode?: "keyword" | "semantic";
}

export async function searchHighlights(p: SearchParams): Promise<SearchResult[]> {
  return invoke<SearchResult[]>("search_highlights", {
    query: p.query,
    source: p.source ?? null,
    color: p.color ?? null,
    mode: p.mode ?? "keyword",
  });
}

export async function runImport(): Promise<ImportStatus> {
  return invoke<ImportStatus>("run_import");
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
