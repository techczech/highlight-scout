// Highlight index results (HS-M1B, Highlights alone): Classic's list in the
// quick finder's rows. Classic's groups (work, author, year, tag, with a
// "then" level), its four densities, the semantic score badges and paging on
// scroll are kept; the selected row carries the copy actions, as every
// corpus's does. Presentational and hook-free.
import type { ReactNode } from "react";
import type { Section } from "../../lib/grouping";
import { compact, formatDate } from "../../lib/format";
import { renderInlineMarkdown, renderMarkdown } from "../../lib/markdown";
import { SOURCE_LABEL, SORT_LABEL, type ResultSort } from "../../lib/quickFinder";
import { resolveColor, type Density, type SearchResult } from "../../types";
import { RowActions, type RowActionHandlers } from "./GroupedResults";

export interface HighlightResultsProps {
  query: string;
  terms: string[];
  rows: SearchResult[];
  sections: Section[] | null;
  density: Density;
  semantic: boolean;
  showPane: boolean;
  /** "Group: Work", after the query in the list head. */
  groupLabel: string;
  sort: ResultSort;
  onSort: (s: ResultSort) => void;
  activeId: string | null;
  onActivate: (id: string) => void;
  /** Double-click: the work's highlights (Classic's detail view). */
  onOpenDetail: (id: string) => void;
  onScrollEnd: () => void;
  hasMore: boolean;
  actions: RowActionHandlers;
  /** What the list shows when there are no rows (searching, no results, press ⏎ …). */
  empty: ReactNode;
}

/** The grey meta line: source · location · author · year (the work's head already names work and author). */
export function highlightMeta(row: SearchResult, groupedByWork: boolean): string[] {
  const src = SOURCE_LABEL[row.source_system] ?? row.source_system;
  const year = row.highlighted_at && /^\d{4}/.test(row.highlighted_at) ? row.highlighted_at.slice(0, 4) : null;
  const isTweet = (row.work_type || "").toLowerCase().startsWith("tweet");
  return [
    src,
    !isTweet && row.location ? `location ${row.location}` : null,
    groupedByWork ? null : row.author,
    groupedByWork ? null : row.title ? compact(row.title, 60) : null,
    year,
  ].filter(Boolean) as string[];
}

function Row({ row, p, groupedByWork }: { row: SearchResult; p: HighlightResultsProps; groupedByWork: boolean }) {
  const active = p.activeId === row.highlight_id;
  const dot = resolveColor(row.annotation_color);
  const body = row.text || (row.format === "image" ? "[image annotation]" : "");
  const img = row.format === "image" ? "🖼 " : "";
  const hay = `${row.text} ${row.title} ${row.author ?? ""}`.toLowerCase();
  const keywordHit = p.terms.some((t) => hay.includes(t.toLowerCase()));
  const year = row.highlighted_at && /^\d{4}/.test(row.highlighted_at) ? row.highlighted_at.slice(0, 4) : "";
  const common = {
    className: `qf-row hl${active ? " on" : ""}`,
    "data-testid": "highlight-row",
    "data-id": row.highlight_id,
    role: "option",
    "aria-selected": active,
    onClick: () => p.onActivate(row.highlight_id),
    onDoubleClick: () => p.onOpenDetail(row.highlight_id),
    ref: active ? (el: HTMLDivElement | null) => el?.scrollIntoView({ block: "nearest" }) : undefined,
  };
  if (p.density === "minimal") {
    return (
      <div {...common}>
        <div className="body min">
          {dot && <span className="dot" style={{ backgroundColor: dot }} />}
          <span className="txt">{img}{renderInlineMarkdown(compact(body, 240), p.terms)}</span>
          {p.semantic && row.relevance != null && <span className="rel">{Math.round(row.relevance * 100)}%</span>}
          {!p.showPane && <><span className="au">{row.author ?? ""}</span><span className="yr">{year}</span></>}
          {active && <RowActions a={p.actions} />}
        </div>
      </div>
    );
  }
  return (
    <div {...common}>
      <div className="body">
        <div className="qf-meta">
          {dot && <span className="dot" style={{ backgroundColor: dot }} title={row.annotation_color ?? undefined} />}
          <span className="tail">{highlightMeta(row, groupedByWork).join(" · ")}</span>
          {p.density === "full" && row.highlighted_at && <span>· {formatDate(row.highlighted_at)}</span>}
        </div>
        {p.semantic && (
          <div className="qf-badges">
            {row.relevance != null && <span className="rel">{Math.round(row.relevance * 100)}%</span>}
            <span className={keywordHit ? "kw" : "sem"}>{keywordHit ? "keyword + semantic" : "✦ semantic"}</span>
          </div>
        )}
        <div className={`qf-snip${p.density === "full" ? " full" : ""}`}>
          {img}
          {p.density === "full" ? renderMarkdown(body, p.terms) : renderInlineMarkdown(body, p.terms)}
        </div>
        {p.density === "full" && row.tags.length > 0 && (
          <div className="qf-tags">{row.tags.slice(0, 4).map((t) => <span key={t}>{t}</span>)}</div>
        )}
        {active && <RowActions a={p.actions} />}
      </div>
    </div>
  );
}

export function HighlightResults(p: HighlightResultsProps) {
  const onScroll = (e: React.UIEvent<HTMLDivElement>) => {
    const el = e.currentTarget;
    if (el.scrollHeight - el.scrollTop - el.clientHeight < 300) p.onScrollEnd();
  };
  const byWork = (s: Section) => s.id.startsWith("work:") || s.id.includes(">work:");
  const head = (
    <div className="qf-listhead">
      {p.query.trim() ? <b>{p.query.trim()}</b> : <b>Highlights</b>}
      <span>·</span>
      <span>{p.groupLabel}</span>
      <select value={p.sort} onChange={(e) => p.onSort(e.target.value as ResultSort)} aria-label="Sort results">
        {(Object.keys(SORT_LABEL) as ResultSort[]).map((s) => <option key={s} value={s}>{SORT_LABEL[s]}</option>)}
      </select>
    </div>
  );
  const rows = (list: SearchResult[], w: boolean) => list.map((r) => <Row key={r.highlight_id} row={r} p={p} groupedByWork={w} />);
  return (
    <div className={`qf-list d-${p.density}`} data-testid="highlight-results">
      {head}
      {p.rows.length === 0 ? (
        <div className="qf-empty" data-testid="highlight-empty">{p.empty}</div>
      ) : (
        <div className="qf-scroll" role="listbox" aria-label="Results" onScroll={onScroll}>
          {p.sections
            ? p.sections.map((s) => (
                <section key={s.id} data-testid="result-group" data-group={s.id}>
                  <div className="qf-sechead"><span className="lbl">{s.title}</span>{s.subtitle && <span className="sub">{s.subtitle}</span>}</div>
                  {s.subs
                    ? s.subs.map((sub) => (
                        <div key={sub.id}>
                          <div className="qf-sechead sub2"><span className="lbl">{sub.title}</span>{sub.subtitle && <span className="sub">{sub.subtitle}</span>}</div>
                          {rows(sub.rows, byWork(sub) || byWork(s))}
                        </div>
                      ))
                    : rows(s.rows, byWork(s))}
                </section>
              ))
            : rows(p.rows, false)}
          {p.hasMore && <div className="qf-more">Scroll for more…</div>}
        </div>
      )}
    </div>
  );
}
