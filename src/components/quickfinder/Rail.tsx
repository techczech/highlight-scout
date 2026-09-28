// HS-M1B left rail: Search in (tick one or more corpora, with their real
// sizes; the highlight sources beneath Highlights), Sets (a placeholder until
// sets arrive) and Recent searches. Presentational and hook-free.
import type { CorpusCount } from "../../lib/archive";
import { CORPUS_LABEL, CORPUS_ORDER, SOURCE_LABEL, sourceOrder, type CorpusId } from "../../lib/quickFinder";
import { Icon } from "./icons";

export interface RailProps {
  counts: CorpusCount[];
  /** Ticked corpora. */
  corpora: CorpusId[];
  /** Whether a highlight source is ticked. */
  sourceOn: (src: string) => boolean;
  onCorpus: (c: CorpusId) => void;
  onSource: (src: string) => void;
  recent: string[];
  onRecent: (q: string) => void;
}

function N({ n }: { n: number | undefined }) {
  return n === undefined ? <span className="n"><span className="ph" title="count not known yet" /></span> : <span className="n">{n.toLocaleString()}</span>;
}

function Tick({ on }: { on: boolean }) {
  return (
    <span className={`tick${on ? " on" : ""}`} aria-hidden="true">
      {on && <svg viewBox="0 0 12 12"><path d="m2.5 6.2 2.3 2.3 4.7-5" /></svg>}
    </span>
  );
}

export function Rail({ counts, corpora, sourceOn, onCorpus, onSource, recent, onRecent }: RailProps) {
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
            <Tick on={on} /><span className={`dot ${c}`} /><span className="label">{CORPUS_LABEL[c] ?? c}</span><N n={cc?.indexed ? cc.docs : undefined} />
          </button>,
          ...sourceOrder(sources).map((s) => {
            const son = sourceOn(s);
            return (
              <button key={`${c}:${s}`} className="ri tickrow sub" role="checkbox" aria-checked={son} data-source={s} onClick={() => onSource(s)}>
                <Tick on={son} /><span className="label">{SOURCE_LABEL[s] ?? s}</span><N n={sources[s]} />
              </button>
            );
          }),
        ];
      })}

      <div className="rl">Sets</div>
      <div className="ri muted" title="Sets arrive in the next release"><Icon name="set" size="sm" /><span className="label">No sets yet</span></div>

      <div className="rl">Recent</div>
      {recent.length === 0 && <div className="ri muted"><span className="label">No recent searches</span></div>}
      {recent.map((q) => (
        <button key={q} className="ri recent" data-recent={q} onClick={() => onRecent(q)} title={`Search “${q}” again`}>
          <Icon name="clock" size="sm" /><span className="label">{q}</span>
        </button>
      ))}

      <div className="foot">Tick one or more. Counts are pieces, tweets and works.</div>
    </nav>
  );
}
