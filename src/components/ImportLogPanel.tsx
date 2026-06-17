import { useEffect, useState } from "react";
import { getImportLog } from "../lib/api";
import type { ImportLogEntry } from "../types";
import { Overlay } from "./TagPicker";

export function ImportLogPanel({ onClose }: { onClose: () => void }) {
  const [entries, setEntries] = useState<ImportLogEntry[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    getImportLog()
      .then(setEntries)
      .finally(() => setLoading(false));
  }, []);

  const when = (iso: string) => {
    const d = new Date(iso);
    return Number.isNaN(d.getTime())
      ? iso
      : d.toLocaleString("en-GB", { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });
  };

  return (
    <Overlay title="Import log" onClose={onClose} wide>
      {loading ? (
        <p className="p-3 text-sm text-zinc-400">Loading…</p>
      ) : entries.length === 0 ? (
        <p className="p-3 text-sm text-zinc-400">No imports recorded yet.</p>
      ) : (
        <div className="max-h-[60vh] overflow-y-auto">
          {entries.map((e, i) => (
            <div key={i} className="flex items-start gap-3 border-b border-zinc-100 py-2 text-sm">
              <span
                className={`mt-0.5 inline-block h-2 w-2 shrink-0 rounded-full ${
                  e.status === "ok" ? "bg-green-500" : "bg-red-500"
                }`}
                title={e.status}
              />
              <div className="min-w-0 flex-1">
                <div className="flex flex-wrap items-center gap-2">
                  <span className="font-medium capitalize text-zinc-700">{e.source}</span>
                  <span className="text-xs text-zinc-400">{when(e.timestamp)}</span>
                  {e.status === "ok" && (e.works > 0 || e.highlights > 0) && (
                    <span className="text-xs text-zinc-500">
                      {e.works.toLocaleString()} works · {e.highlights.toLocaleString()} highlights
                    </span>
                  )}
                  {e.duration_ms > 0 && (
                    <span className="text-xs text-zinc-300">{(e.duration_ms / 1000).toFixed(1)}s</span>
                  )}
                </div>
                <p className={`text-xs ${e.status === "ok" ? "text-zinc-500" : "text-red-600"}`}>{e.message}</p>
              </div>
            </div>
          ))}
        </div>
      )}
    </Overlay>
  );
}
