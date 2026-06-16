import { invoke } from "@tauri-apps/api/core";
import type { SearchResult, ImportStatus, Stats, Config } from "../types";

export async function searchHighlights(query: string): Promise<SearchResult[]> {
  return invoke<SearchResult[]>("search_highlights", { query });
}

export async function runImport(): Promise<ImportStatus> {
  return invoke<ImportStatus>("run_import");
}

export async function getStats(): Promise<Stats> {
  return invoke<Stats>("get_stats");
}

export async function getConfig(): Promise<Config> {
  return invoke<Config>("get_config");
}
