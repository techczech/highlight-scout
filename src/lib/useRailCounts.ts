// The rail's result counts as React state: the counter from railCounts.ts
// wired to the engine (`corpus_search` totals) and the highlight index
// (`search_counts`), debounced like the search, its cache cleared whenever
// `epoch` changes (an import, an index rebuild, Refresh).
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { archiveSearch } from "./archive";
import { createRailCounter, type HighlightCounts, type RailCounter, type RailCountJob, type RailCounts } from "./railCounts";

const DEBOUNCE_MS = 160;

export function useRailCounts(job: RailCountJob | null, epoch: unknown): RailCounts | null {
  const [counts, setCounts] = useState<RailCounts | null>(null);
  const counter = useRef<RailCounter | null>(null);
  if (!counter.current) {
    counter.current = createRailCounter({
      countArchive: (r) => archiveSearch(r).then((a) => a.body.total_documents),
      countHighlights: (p) => invoke<HighlightCounts>("search_counts", { query: p }),
      onChange: setCounts,
    });
  }
  const lastEpoch = useRef(epoch);
  const key = job?.key ?? null;

  useEffect(() => {
    const c = counter.current!;
    if (lastEpoch.current !== epoch) {
      lastEpoch.current = epoch;
      c.clear();
    }
    if (!job) {
      c.run(null);
      return;
    }
    const t = setTimeout(() => c.run(job), DEBOUNCE_MS);
    return () => clearTimeout(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, epoch]);

  return counts;
}
