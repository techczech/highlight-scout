import type { SyncSourceStatus } from "../types";

/** Red lines to show: one per configured source whose last sync failed. */
export function failureLines(sources: SyncSourceStatus[]): string[] {
  return sources
    .filter((s) => s.configured && s.last_error)
    .map((s) => `${s.label}: ${s.last_error}`);
}

function ago(iso: string, now: number): string {
  const t = Date.parse(iso);
  if (Number.isNaN(t)) return "unknown";
  const mins = Math.max(0, Math.round((now - t) / 60_000));
  if (mins < 1) return "just now";
  if (mins < 60) return `${mins} min ago`;
  const hours = Math.round(mins / 60);
  if (hours < 48) return `${hours} h ago`;
  return new Date(t).toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" });
}

/** "Readwise: 5 min ago" per configured source; "never" when it has not synced. */
export function lastSyncedLines(sources: SyncSourceStatus[], now = Date.now()): string[] {
  return sources
    .filter((s) => s.configured)
    .map((s) => `${s.label}: ${s.last_synced_at ? ago(s.last_synced_at, now) : "never"}`);
}
