// The meaning index's state for the window: read at launch, when Semantic is
// switched on, and after any index or meaning build; the build's progress
// from `corpus:meaning` events. `build()` is the only way a build starts.
import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { meaningBuild, meaningState, type MeaningJob, type MeaningState } from "./meaning";
import type { IndexJob } from "./archive";

export function useMeaning(semantic: boolean) {
  const [state, setState] = useState<MeaningState | null>(null);
  const [job, setJob] = useState<MeaningJob | null>(null);
  const [version, setVersion] = useState(0);

  const reload = useCallback(() => {
    meaningState().then(setState).catch(() => {});
  }, []);

  useEffect(() => { reload(); }, [reload, version]);
  useEffect(() => { if (semantic) reload(); }, [semantic, reload]);

  useEffect(() => {
    const a = listen<MeaningJob>("corpus:meaning", (e) => {
      setJob(e.payload);
      if (e.payload.phase !== "building") setVersion((v) => v + 1);
    });
    // A keeper build can leave vectors stale (or, with the model, current).
    const b = listen<IndexJob>("corpus:index", (e) => {
      if (e.payload.phase === "built" || e.payload.phase === "failed") setVersion((v) => v + 1);
    });
    return () => { a.then((f) => f()); b.then((f) => f()); };
  }, []);

  const build = useCallback(async () => {
    setJob({ phase: "building", corpora: [], corpus: null, done: 0, total: 0, message: "starting" });
    try {
      await meaningBuild([]);
    } catch (e) {
      setJob({ phase: "failed", corpora: [], corpus: null, done: 0, total: 0, message: `Meaning index build failed: ${e instanceof Error ? e.message : String(e)}` });
    }
  }, []);

  return { state, job, build };
}
