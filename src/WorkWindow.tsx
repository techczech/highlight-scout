import { useEffect, useState } from "react";
import { workHighlights, getConfig } from "./lib/api";
import { WorkBody } from "./components/WorkView";
import type { SearchResult } from "./types";

/** Standalone window showing a single work's highlights (?work=<id>). */
export default function WorkWindow({ workId }: { workId: string }) {
  const [work, setWork] = useState<SearchResult | null>(null);
  const [archiveRoot, setArchiveRoot] = useState("");
  const [toast, setToast] = useState("");
  const [error, setError] = useState("");

  useEffect(() => {
    getConfig().then((c) => setArchiveRoot(c.archive_path)).catch(() => {});
    workHighlights(workId)
      .then((rows) => {
        if (rows[0]) setWork(rows[0]);
        else setError("No highlights for this work.");
      })
      .catch((e) => setError(String(e)));
  }, [workId]);

  const showToast = (msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(""), 1600);
  };

  return (
    <div className="relative flex h-screen flex-col overflow-hidden bg-white p-4 text-zinc-900">
      {error && <p className="text-sm text-red-500">{error}</p>}
      {work ? (
        <>
          <h1 className="mb-2 text-sm font-semibold text-zinc-800">{work.title}</h1>
          <WorkBody work={work} archiveRoot={archiveRoot} onToast={showToast} showNewWindow={false} />
        </>
      ) : (
        !error && <p className="text-sm text-zinc-400">Loading…</p>
      )}
      {toast && (
        <div className="absolute bottom-6 left-1/2 -translate-x-1/2 rounded bg-zinc-800 px-3 py-1.5 text-xs text-white shadow-lg">
          {toast}
        </div>
      )}
    </div>
  );
}
