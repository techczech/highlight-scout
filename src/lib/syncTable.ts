// Settings → Sync as a table (HS-3A): per source, when it last synced, and
// the result of its last run or its current error. Pure; the data is the
// app's sync status plus the failure times from sync-state.json.
import type { SyncStatus } from "../types";

/** Table names for the sources (the red line keeps the shorter labels). */
export const SOURCE_TABLE_LABEL: Record<string, string> = {
  readwise: "Readwise highlights",
  readwise_tweets: "Readwise saved tweets",
  zotero: "Zotero",
};

export interface SyncRow {
  key: string;
  label: string;
  state: "ok" | "failed" | "off";
  /** "this morning, 07:42", or "never". */
  lastSynced: string;
  /** "last try 09:10 failed" when the current error is newer than the last success. */
  lastTry: string | null;
  /** "added 732", the error in plain words, "not connected", or "—". */
  result: string;
}

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

function hm(d: Date): string {
  return d.toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit" });
}

/** "this morning, 07:42" · "yesterday, 18:05" · "3 Sep, 10:00" · "3 Sep 2025". */
export function whenLine(iso: string | null, now = new Date()): string {
  if (!iso) return "never";
  const t = new Date(iso);
  if (Number.isNaN(t.getTime())) return "unknown";
  const day = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  const days = Math.round((day(now) - day(t)) / 86_400_000);
  if (days === 0) {
    const h = t.getHours();
    const part = h < 12 ? "this morning" : h < 18 ? "this afternoon" : "this evening";
    return `${part}, ${hm(t)}`;
  }
  if (days === 1) return `yesterday, ${hm(t)}`;
  const date = `${t.getDate()} ${MONTHS[t.getMonth()]}`;
  if (t.getFullYear() === now.getFullYear()) return `${date}, ${hm(t)}`;
  return `${date} ${t.getFullYear()}`;
}

/** One row per source (every source, connected or not), in the app's order. */
export function syncRows(status: SyncStatus, errorTimes: Record<string, string>, now = new Date()): SyncRow[] {
  const results = new Map((status.last_report?.results ?? []).map((r) => [r.source, r]));
  return status.sources.map((s) => {
    const label = SOURCE_TABLE_LABEL[s.key] ?? s.label;
    if (!s.configured) {
      return { key: s.key, label, state: "off", lastSynced: whenLine(s.last_synced_at, now), lastTry: null, result: "not connected" };
    }
    const r = results.get(s.key);
    const failedAt = errorTimes[s.key] ?? null;
    if (s.last_error) {
      return {
        key: s.key,
        label,
        state: "failed",
        lastSynced: whenLine(s.last_synced_at, now),
        lastTry: failedAt ? `last try ${whenLine(failedAt, now)} failed` : "last try failed",
        result: s.last_error,
      };
    }
    return {
      key: s.key,
      label,
      state: "ok",
      lastSynced: whenLine(s.last_synced_at, now),
      lastTry: null,
      result: r && !r.error ? `added ${r.added.toLocaleString()}` : "—",
    };
  });
}

/** The table's heading: when the rows' results come from. */
export function syncTableHeading(status: SyncStatus, now = new Date()): string {
  const r = status.last_report;
  if (!r) return "Since Highlight Scout opened: no sync yet";
  const failed = r.results.some((x) => x.error);
  return `Last sync: ${whenLine(r.finished_at, now)}${failed ? " · a source failed" : ""}`;
}
