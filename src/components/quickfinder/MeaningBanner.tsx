// Semantic mode's word on the meaning index (ticket 09): missing or out of
// date (with "Build meaning index" and what it costs), building (progress),
// failed (try again). Nothing when the index is current.
import type { MeaningNotice } from "../../lib/meaning";

export function MeaningBanner(p: { notice: MeaningNotice | null; onBuild: () => void }) {
  if (!p.notice) return null;
  const n = p.notice;
  return (
    <div className={`qf-banner meaning ${n.kind}`} role="status" data-testid="meaning-banner" data-state={n.kind}>
      {n.kind === "building" && <span className="qf-spin" aria-hidden="true" />}
      <span>{n.text}</span>
      {n.action && (
        <>
          {" "}
          <button onClick={p.onBuild} data-testid="meaning-build">{n.action}</button>
        </>
      )}
    </div>
  );
}
