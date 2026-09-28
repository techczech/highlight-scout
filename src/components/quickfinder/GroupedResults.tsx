// HS-1B results: grouped Writing / Tweets / Highlights, the selected row
// carrying its copy actions. Presentational and hook-free: the container owns
// the state and the handlers. Quotes are original source text, never HTML.
import type { ReactNode } from "react";
import type { ArchiveDoc, ArchiveSearchResults } from "../../lib/archive";
import { corpusBadge } from "../../lib/archive";
import { emphasizeSegments } from "../../lib/format";
import {
  docKey,
  groupCount,
  rowHasTitle,
  rowMeta,
  SORT_LABEL,
  type CorpusFilter,
  type ResultGroup,
  type ResultSort,
} from "../../lib/quickFinder";
import { Icon } from "./icons";

export type CopiedWhat = "quote" | "citation" | "link" | null;

export interface RowActionHandlers {
  onQuote: () => void;
  onQuoteCitation: () => void;
  onLink: () => void;
  /** What was just copied from the selected row (confirmed in place). */
  copied: CopiedWhat;
  /** The app Esc returns to, e.g. "WriteFlex". */
  backTo: string | null;
  /** Whether the passage is loaded, so there is something to copy. */
  ready: boolean;
}

/** Text with the query terms marked. */
export function Marked({ text, terms }: { text: string; terms: string[] }): ReactNode {
  return (
    <>
      {emphasizeSegments(text, terms).map((s, i) => (s.match ? <mark key={i}>{s.text}</mark> : <span key={i}>{s.text}</span>))}
    </>
  );
}

const COPIED_LABEL: Record<Exclude<CopiedWhat, null>, string> = {
  quote: "Copied quote",
  citation: "Copied quote + citation",
  link: "Copied link",
};

export function RowActions({ a }: { a: RowActionHandlers }) {
  if (a.copied) {
    return (
      <div className="qf-acts" data-testid="row-actions">
        <span className="qf-act done" data-action="copied"><Icon name="check" size="sm" />{COPIED_LABEL[a.copied]}</span>
        <span className="qf-act soft">{a.backTo ? `esc back to ${a.backTo}` : "esc hides"}</span>
      </div>
    );
  }
  return (
    <div className="qf-acts" data-testid="row-actions">
      <button className="qf-act" data-action="quote" disabled={!a.ready} onClick={(e) => { e.stopPropagation(); a.onQuote(); }} title="Copy the passage (⌘C)">
        <Icon name="quote" size="sm" />Quote <kbd>⌘C</kbd>
      </button>
      <button className="qf-act pri" data-action="quote-citation" disabled={!a.ready} onClick={(e) => { e.stopPropagation(); a.onQuoteCitation(); }} title="Copy the passage with its citation (⌘⇧C)">
        <Icon name="cite" size="sm" />Quote + citation <kbd>⌘⇧C</kbd>
      </button>
      <button className="qf-act" data-action="link" disabled={!a.ready} onClick={(e) => { e.stopPropagation(); a.onLink(); }} title="Copy the public link (the archive link when there is none)">
        <Icon name="link" size="sm" />Link
      </button>
      <button className="qf-act" data-action="set" disabled title="Sets arrive in the next release">
        + Set
      </button>
    </div>
  );
}

function Row({ doc, active, terms, onSelect, actions }: {
  doc: ArchiveDoc;
  active: boolean;
  terms: string[];
  onSelect: (key: string) => void;
  actions: RowActionHandlers;
}) {
  const meta = rowMeta(doc);
  const hit = doc.hits[0];
  const cls = doc.corpus === "writing" || doc.corpus === "tweets" || doc.corpus === "highlights" ? doc.corpus : "other";
  return (
    <div
      className={`qf-row${active ? " on" : ""}`}
      data-testid="archive-row"
      data-key={docKey(doc)}
      aria-selected={active}
      role="option"
      onClick={() => onSelect(docKey(doc))}
      ref={active ? (el) => el?.scrollIntoView({ block: "nearest" }) : undefined}
    >
      <div className="body">
        <div className="qf-meta">
          <span className={`qf-src ${cls}`}>{corpusBadge(doc.corpus)}</span>
          {meta.by && <><span className="by">{meta.by}</span><span>·</span></>}
          <span className="tail">{meta.parts.join(" · ")}</span>
          {doc.passage_count > 1 && <span>· {doc.passage_count} passages</span>}
        </div>
        {rowHasTitle(doc) && <div className="qf-title">{doc.title || doc.rel_path}</div>}
        {hit && <div className="qf-snip"><Marked text={hit.quote} terms={terms} /></div>}
        {active && <RowActions a={actions} />}
      </div>
    </div>
  );
}

export interface GroupedResultsProps {
  query: string;
  terms: string[];
  results: ArchiveSearchResults | null;
  groups: ResultGroup[];
  filter: CorpusFilter;
  sort: ResultSort;
  onSort: (s: ResultSort) => void;
  activeKey: string | null;
  onSelect: (key: string) => void;
  onShowAll: (corpus: string) => void;
  actions: RowActionHandlers;
  loading: boolean;
  error: string;
  summary: string;
}

export function GroupedResults(p: GroupedResultsProps) {
  const head = (
    <div className="qf-listhead" title={p.summary || undefined}>
      {p.query.trim() ? <b>{p.query.trim()}</b> : <b>Archive</b>}
      <span>·</span>
      <span>{p.filter.corpus === "all" ? "grouped by corpus" : p.summary || "searching"}</span>
      <select value={p.sort} onChange={(e) => p.onSort(e.target.value as ResultSort)} aria-label="Sort results">
        {(Object.keys(SORT_LABEL) as ResultSort[]).map((s) => <option key={s} value={s}>{SORT_LABEL[s]}</option>)}
      </select>
    </div>
  );
  let body: ReactNode;
  if (p.groups.length === 0) {
    body = (
      <div className="qf-empty" data-testid="archive-empty">
        {p.error ? <p className="warn">{p.error}</p>
          : p.loading ? <p>Searching writing, tweets and highlights…</p>
          : p.results ? <p>No results for “{p.results.query}”</p>
          : <><p>Search your writing, tweets and highlights.</p><p className="hint">"exact phrase" · -exclude · prefix* · in:writing · after:2020 · au:lakoff · /regex/</p></>}
      </div>
    );
  } else {
    body = (
      <div className="qf-scroll" role="listbox" aria-label="Results">
        {p.groups.map((g) => (
          <section key={g.corpus} data-testid="result-group" data-corpus={g.corpus}>
            <div className="qf-sechead">
              <span className={`dot ${g.corpus}`} />
              {g.label}
              <span className="cnt">{groupCount(g, p.results)}</span>
              {g.more && (
                <button className="more" data-action="show-all" onClick={() => p.onShowAll(g.corpus)} title={`Search ${g.label} only`}>
                  show all
                </button>
              )}
            </div>
            {g.docs.map((d) => (
              <Row key={docKey(d)} doc={d} active={docKey(d) === p.activeKey} terms={p.terms} onSelect={p.onSelect} actions={p.actions} />
            ))}
          </section>
        ))}
      </div>
    );
  }
  return (
    <div className="qf-list">
      {head}
      {body}
    </div>
  );
}
