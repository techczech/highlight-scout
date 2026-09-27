import { describe, expect, test } from "vitest";
import type { SyncSourceStatus } from "../types";
import { failureLines, lastSyncedLines } from "./sync";

const src = (o: Partial<SyncSourceStatus>): SyncSourceStatus => ({
  key: "readwise",
  label: "Readwise",
  configured: true,
  last_synced_at: null,
  last_error: null,
  ...o,
});

describe("sync status lines", () => {
  test("failures name the source; unconfigured sources are hidden", () => {
    expect(
      failureLines([
        src({ last_error: "token rejected (401)" }),
        src({ key: "zotero", label: "Zotero", configured: false, last_error: "could not open the Zotero database" }),
        src({ key: "readwise_tweets", label: "Readwise saved tweets" }),
      ]),
    ).toEqual(["Readwise: token rejected (401)"]);
  });

  test("last synced reads relative, never when missing", () => {
    const now = Date.parse("2026-09-27T12:00:00Z");
    expect(
      lastSyncedLines(
        [
          src({ last_synced_at: "2026-09-27T11:55:00Z" }),
          src({ key: "zotero", label: "Zotero", last_synced_at: "2026-09-27T09:00:00Z" }),
          src({ key: "readwise_tweets", label: "Readwise saved tweets" }),
        ],
        now,
      ),
    ).toEqual(["Readwise: 5 min ago", "Zotero: 3 h ago", "Readwise saved tweets: never"]);
  });
});
