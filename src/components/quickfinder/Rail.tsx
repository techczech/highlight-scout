// HS-1B left rail: Search in (corpora with their real sizes, highlight
// sources beneath), Sets (a placeholder until sets arrive) and Recent
// searches. Presentational and hook-free.
import type { CorpusCount } from "../../lib/archive";
import { CORPUS_LABEL, CORPUS_ORDER, SOURCE_LABEL, sameFilter, sourceOrder, type CorpusFilter } from "../../lib/quickFinder";
import { Icon } from "./icons";

export interface RailProps {
  counts: CorpusCount[];
  filter: CorpusFilter;
  onFilter: (f: CorpusFilter) => void;
  recent: string[];
  onRecent: (q: string) => void;
}

function N({ n }: { n: number | undefined }) {
  return n === undefined ? <span className="n"><span className="ph" title="count not known yet" /></span> : <span className="n">{n.toLocaleString()}</span>;
}

export function Rail({ counts, filter, onFilter, recent, onRecent }: RailProps) {
  const by = new Map(counts.map((c) => [c.corpus, c]));
  const corpora = [...CORPUS_ORDER.filter((c) => by.has(c) || counts.length === 0), ...counts.map((c) => c.corpus).filter((c) => !CORPUS_ORDER.includes(c as never))];
  const item = (f: CorpusFilter, cls: string, children: React.ReactNode, key: string) => (
    <button key={key} className={`ri ${cls}${sameFilter(f, filter) ? " on" : ""}`} data-filter={key} onClick={() => onFilter(f)}>
      {children}
    </button>
  );
  return (
    <nav className="qf-rail" aria-label="Search in" data-testid="rail">
      <div className="rl">Search in</div>
      {item({ corpus: "all" }, "", <><Icon name="search" size="sm" /><span className="label">All corpora</span></>, "all")}
      {corpora.map((c) => {
        const cc = by.get(c);
        const sources = cc?.sources ?? {};
        return [
          item({ corpus: c as CorpusFilter["corpus"] }, "", <><span className={`dot ${c}`} /><span className="label">{CORPUS_LABEL[c] ?? c}</span><N n={cc?.indexed ? cc.docs : undefined} /></>, c),
          ...sourceOrder(sources).map((s) =>
            item({ corpus: "highlights", source: s }, "sub", <><span className="label">{SOURCE_LABEL[s] ?? s}</span><N n={sources[s]} /></>, `${c}:${s}`),
          ),
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

      <div className="foot">Counts are pieces, tweets and works.</div>
    </nav>
  );
}
