// "Analyse in ArchiveScout ↗" (HS-5A): the count of the search ("30 pieces ·
// 64 matches in Writing") with the hand-off button beside it. If ArchiveScout
// is not installed the button says so and links to its release page; it is
// never a dead button. Presentational and hook-free.
import { Icon } from "./icons";

export interface AnalyseBarProps {
  /** "30 pieces · 64 matches". */
  count: string;
  /** "in Writing". */
  scope: string;
  /** What ArchiveScout would look up; empty = nothing to hand over. */
  term: string;
  /** The button's tooltip. */
  title: string;
  /** null while not known: the button works as if installed. */
  installed: boolean | null;
  onAnalyse: () => void;
  onGet: () => void;
}

export function AnalyseBar(p: AnalyseBarProps) {
  const missing = p.installed === false;
  return (
    <div className="qf-countrow" data-testid="analyse-bar">
      <b>{p.count}</b>
      {p.scope && <span className="scope">{p.scope}</span>}
      {missing ? (
        <button className="qf-analyse missing" data-action="get-archivescout" onClick={p.onGet} title="ArchiveScout is not installed on this Mac. Opens its release page.">
          <Icon name="ext" size="sm" />ArchiveScout is not installed · get it
        </button>
      ) : (
        <button className="qf-analyse" data-action="analyse" disabled={!p.term} onClick={p.onAnalyse} title={p.term ? p.title : "Type a word to analyse it in ArchiveScout"}>
          <Icon name="spark" size="sm" />Analyse in ArchiveScout<Icon name="ext" size="sm" />
        </button>
      )}
    </div>
  );
}
