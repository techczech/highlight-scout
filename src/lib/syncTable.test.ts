import { describe, expect, test } from "vitest";
import type { SyncStatus } from "../types";
import { syncRows, syncTableHeading, whenLine } from "./syncTable";

const now = new Date(2026, 8, 28, 11, 30); // 28 Sep 2026, 11:30 local

const at = (d: number, h: number, m = 0) => new Date(2026, 8, d, h, m).toISOString();

const status: SyncStatus = {
  running: false,
  sources: [
    { key: "readwise", label: "Readwise", configured: true, last_synced_at: at(28, 7, 42), last_error: "couldn't read Readwise's reply (nextPageCursor)" },
    { key: "readwise_tweets", label: "Readwise saved tweets", configured: true, last_synced_at: at(28, 9, 10), last_error: null },
    { key: "zotero", label: "Zotero", configured: false, last_synced_at: null, last_error: null },
  ],
  last_report: {
    seq: 2,
    trigger: "launch",
    started_at: at(28, 9, 9),
    finished_at: at(28, 9, 10),
    results: [
      { source: "readwise", label: "Readwise", added: 0, error: "couldn't read Readwise's reply (nextPageCursor)", finished_at: at(28, 9, 10) },
      { source: "readwise_tweets", label: "Readwise saved tweets", added: 181, error: null, finished_at: at(28, 9, 10) },
    ],
    summary: "",
  },
};

describe("Settings → Sync table", () => {
  test("times read as this morning, yesterday, a date, or never", () => {
    expect(whenLine(at(28, 7, 42), now)).toBe("this morning, 07:42");
    expect(whenLine(at(28, 14, 5), new Date(2026, 8, 28, 20))).toBe("this afternoon, 14:05");
    expect(whenLine(at(27, 18, 5), now)).toBe("yesterday, 18:05");
    expect(whenLine(at(3, 10, 0), now)).toBe("3 Sep, 10:00");
    expect(whenLine(new Date(2025, 8, 3).toISOString(), now)).toBe("3 Sep 2025");
    expect(whenLine(null, now)).toBe("never");
  });

  test("one row per source: last synced, the failure with its time, or what the last run added", () => {
    const rows = syncRows(status, { readwise: at(28, 9, 10) }, now);
    expect(rows.map((r) => [r.label, r.state, r.lastSynced, r.lastTry, r.result])).toEqual([
      ["Readwise highlights", "failed", "this morning, 07:42", "last try this morning, 09:10 failed", "couldn't read Readwise's reply (nextPageCursor)"],
      ["Readwise saved tweets", "ok", "this morning, 09:10", null, "added 181"],
      ["Zotero", "off", "never", null, "not connected"],
    ]);
    expect(syncTableHeading(status, now)).toBe("Last sync: this morning, 09:10 · a source failed");
    expect(syncTableHeading({ ...status, last_report: null }, now)).toBe("Since Highlight Scout opened: no sync yet");
    // No run this session: the result is unknown, the time is still real.
    expect(syncRows({ ...status, last_report: null }, {}, now)[1].result).toBe("—");
  });
});
