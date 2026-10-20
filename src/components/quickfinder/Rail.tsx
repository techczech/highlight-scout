// HS-M1B left rail: Search in (tick one or more corpora, with their real
// sizes; the highlight sources beneath Highlights), Sets (the archive's set
// files: open one, start a new one, or save the current search as a set) and
// Recent searches. With a query each row shows its result
// count instead of its size (ticket 08); sizes are the quieter style.
// Presentational and hook-free.
import type { CorpusCount } from "../../lib/archive";
import { rowCount, type RailCounts } from "../../lib/railCounts";
import { CORPUS_LABEL, CORPUS_ORDER, SOURCE_LABEL, sourceOrder, type CorpusId } from "../../lib/quickFinder";
import { Icon } from "./icons";

export interface RailProps {
  counts: CorpusCount[];
  /** Result counts for the current query; null shows the sizes. */
  results: RailCounts | null;
  /** Ticked corpora. */
  corpora: CorpusId[];
  /** Whether a highlight source is ticked. */
  sourceOn: (src: string) => boolean;
  onCorpus: (c: CorpusId) => void;
  onSource: (src: string) => void;
  recent: string[];
  onRecent: (q: string) => void;
  /** The archive's sets (empty until the first is made). */
  sets?: RailSet[];
  onOpenSet?: (id: string) => void;
  onNewSet?: () => void;
  /** "Save this search as a set": present only while there is a query. */
  onSaveSearch?: () => void;
}

export interface RailSet {
  id: string;
  title: string;
  items: number;
  /** The set "+ Set" adds to. */
  current: boolean;
  kind: "hand" | "saved-search";
}

function N({ n }: { n: number | undefined }) {
  return n === undefined ? <span className="n size"><span className="ph" title="count not known yet" /></span> : <span className="n size" title="In the archive">{n.toLocaleString()}</span>;
}

/** A result count: faded while it is recounted, a dash when the query does not reach it. */
function R({ value, pending }: { value: number | null | undefined; pending: boolean }) {
  if (value === undefined) return <span className="n hits"><span className="ph" title="counting…" /></span>;
  if (value === null) return <span className="n hits none" title={pending ? "counting…" : "Not searched by this query"}>–</span>;
  return <span className={`n hits${pending ? " stale" : ""}`} title="Results for this query">{value.toLocaleString()}</span>;
}

function Tick({ on }: { on: boolean }) {
  return (
    <span className={`tick${on ? " on" : ""}`} aria-hidden="true">
      {on && <svg viewBox="0 0 12 12"><path d="m2.5 6.2 2.3 2.3 4.7-5" /></svg>}
    </span>
  );
}

export function Rail({ counts, results, corpora, sourceOn, onCorpus, onSource, recent, onRecent, sets = [], onOpenSet, onNewSet, onSaveSearch }: RailProps) {
  const by = new Map(counts.map((c) => [c.corpus, c]));
  const list = [...CORPUS_ORDER.filter((c) => by.has(c) || counts.length === 0), ...counts.map((c) => c.corpus).filter((c) => !CORPUS_ORDER.includes(c as never))];
  return (
    <nav className="qf-rail" aria-label="Search in" data-testid="rail">
      <div className="rl">Search in</div>
      {list.map((c) => {
        const cc = by.get(c);
        const on = corpora.includes(c as CorpusId);
        const sources = cc?.sources ?? {};
        return [
          <button key={c} className="ri tickrow" role="checkbox" aria-checked={on} data-corpus={c} onClick={() => onCorpus(c as CorpusId)}>
            <Tick on={on} /><span className={`dot ${c}`} /><span className="label">{CORPUS_LABEL[c] ?? c}</span>{results ? <R {...rowCount(results, c)} /> : <N n={cc?.indexed ? cc.docs : undefined} />}
          </button>,
          ...sourceOrder(sources).map((s) => {
            const son = sourceOn(s);
            return (
              <button key={`${c}:${s}`} className="ri tickrow sub" role="checkbox" aria-checked={son} data-source={s} onClick={() => onSource(s)}>
                <Tick on={son} /><span className="label">{SOURCE_LABEL[s] ?? s}</span>{results ? <R {...rowCount(results, c, s)} /> : <N n={sources[s]} />}
              </button>
            );
          }),
        ];
      })}

      <div className="rl">Sets</div>
      {sets.length === 0 && <div className="ri muted"><Icon name="set" size="sm" /><span className="label">No sets yet</span></div>}
      {sets.map((s) => (
        <button key={s.id} className={`ri setrow${s.current ? " cur" : ""}`} data-set={s.id} onClick={() => onOpenSet?.(s.id)} title={s.current ? `${s.title}: + Set adds here` : `Open ${s.title}`}>
          <Icon name={s.kind === "saved-search" ? "search" : "set"} size="sm" /><span className="label">{s.title}</span><span className="n size">{s.items}</span>
        </button>
      ))}
      {onNewSet && <button className="ri act" data-action="new-set" onClick={onNewSet}><span className="plus">+</span><span className="label">New set</span></button>}
      {onSaveSearch && <button className="ri act" data-action="save-search-set" onClick={onSaveSearch} title="A set that follows this search: new matches appear after a sync"><span className="plus">+</span><span className="label">Save this search as a set</span></button>}

      <div className="rl">Recent</div>
      {recent.length === 0 && <div className="ri muted"><span className="label">No recent searches</span></div>}
      {recent.map((q) => (
        <button key={q} className="ri recent" data-recent={q} onClick={() => onRecent(q)} title={`Search “${q}” again`}>
          <Icon name="clock" size="sm" /><span className="label">{q}</span>
        </button>
      ))}

      <div className="foot">{results ? "Tick one or more. Counts are results: pieces, tweets and highlights." : "Tick one or more. Counts are pieces, tweets and works."}</div>
    </nav>
  );
}
