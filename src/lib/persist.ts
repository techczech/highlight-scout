// Lightweight persistence for view preferences (localStorage). Sort and group
// persist between launches; filters persist only briefly so stale filters don't
// linger (mirrors the Raycast extension's 60s scope window).

import { EMPTY_FILTERS, type Filters } from "./query";

const FILTERS_TTL_MS = 60_000;

export function load<T extends string>(key: string, fallback: T, valid: T[]): T {
  const v = localStorage.getItem(key);
  return v && (valid as string[]).includes(v) ? (v as T) : fallback;
}

export function save(key: string, value: string): void {
  localStorage.setItem(key, value);
}

export function loadFilters(): Filters {
  const touched = Number(localStorage.getItem("filtersTouched") || "0");
  if (!Number.isFinite(touched) || Date.now() - touched > FILTERS_TTL_MS) return EMPTY_FILTERS;
  try {
    const raw = localStorage.getItem("filters");
    if (!raw) return EMPTY_FILTERS;
    return { ...EMPTY_FILTERS, ...(JSON.parse(raw) as Partial<Filters>) };
  } catch {
    return EMPTY_FILTERS;
  }
}

export function saveFilters(filters: Filters): void {
  localStorage.setItem("filters", JSON.stringify(filters));
  localStorage.setItem("filtersTouched", String(Date.now()));
}
