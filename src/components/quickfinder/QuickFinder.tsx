// The one search window (HS-M1B): the search row (Keyword / Semantic inside
// the box, the Filters · Group button, pane toggle, settings), the "Showing"
// chips row, rail | results | reading pane, then any red sync lines and the
// footer. Layout only; App owns the state and the keyboard, and passes the
// engine's results list and pane in.
import { forwardRef, type ReactNode } from "react";
import type { Chip, Mode } from "../../lib/searchModel";
import { Rail, type RailProps } from "./Rail";
import { Icon } from "./icons";
import { resolveColor } from "../../types";

export interface QuickFinderProps {
  query: string;
  onQuery: (q: string) => void;
  placeholder: string;
  loading: boolean;
  mode: Mode;
  onMode: (m: Mode) => void;
  /** The Filters · Group button with its popover. */
  filters: ReactNode;
  showPane: boolean;
  onTogglePane: () => void;
  chips: Chip[];
  /** The colour a colour chip shows. */
  chipColor: string | null;
  onClearChip: (id: string) => void;
  onClearAll: () => void;
  /** Right end of the chips row: "Highlights only · ⌘⇧I edits". */
  scopeNote: string | null;
  /** Under the chips row: the QMD banner. */
  banner: ReactNode;
  rail: RailProps;
  results: ReactNode;
  pane: ReactNode;
  /** Red sync failure lines, above the footer. */
  alerts: ReactNode;
  footLeft: ReactNode;
  footHints: ReactNode;
  version: string;
  onRefresh: () => void;
  onSettings: () => void;
}

export const QuickFinder = forwardRef<HTMLInputElement, QuickFinderProps>(function QuickFinder(p, ref) {
  const showChips = p.chips.length > 0;
  return (
    <div className="qf flex h-full min-h-0 flex-col" data-testid="quick-finder">
      <div className="qf-search">
        <Icon name="search" size="lg" />
        <input
          ref={ref}
          type="text"
          value={p.query}
          onChange={(e) => p.onQuery(e.target.value)}
          placeholder={p.placeholder}
          autoCorrect="off"
          autoCapitalize="off"
          spellCheck={false}
          aria-label="Search writing, tweets and highlights"
        />
        {p.loading && <span className="qf-spin" aria-label="Searching" />}
        {p.mode === "semantic" && <span className="qf-runs" title="Semantic search runs when you press Return"><Icon name="enter" size="sm" />runs</span>}
        <div className="qf-seg" role="radiogroup" aria-label="Keyword or semantic search" data-testid="mode-switch">
          <button role="radio" aria-checked={p.mode === "keyword"} className={p.mode === "keyword" ? "on" : ""} onClick={() => p.onMode("keyword")} title="Keyword search, as you type">Keyword</button>
          <button role="radio" aria-checked={p.mode === "semantic"} className={p.mode === "semantic" ? "on sem" : ""} onClick={() => p.onMode("semantic")} title="Semantic search of highlights (QMD); press Return to run">Semantic</button>
        </div>
        {p.filters}
        <button className={`tool ico${p.showPane ? " on" : ""}`} onClick={p.onTogglePane} title={`${p.showPane ? "Hide" : "Show"} the reading pane (⌘\\)`} aria-label="Toggle reading pane" aria-pressed={p.showPane}><Icon name="pane" size="sm" /></button>
        <button className="tool ico" onClick={p.onSettings} title="Settings (⌘,)" aria-label="Settings"><Icon name="gear" size="sm" /></button>
      </div>
      {showChips && (
        <div className="qf-chips" data-testid="chips">
          <span className="lab">Showing</span>
          {p.chips.map((c) => (
            <span key={c.id} className={`chip${c.kind === "semantic" ? " sem" : ""}`} data-chip={c.id}>
              {c.kind === "color" && <span className="dot" style={{ backgroundColor: resolveColor(p.chipColor) ?? undefined }} />}
              {c.label}
              <button onClick={() => p.onClearChip(c.id)} aria-label={`Remove ${c.label}`} title="Remove"><Icon name="x" size="sm" /></button>
            </span>
          ))}
          <button className="all" onClick={p.onClearAll}>clear all</button>
          {p.scopeNote && <span className="note">{p.scopeNote}</span>}
        </div>
      )}
      {p.banner}
      <div className="qf-main">
        <Rail {...p.rail} />
        {p.results}
        {p.showPane && p.pane}
      </div>
      {p.alerts}
      <div className="qf-foot">
        <button className="refresh" onClick={p.onRefresh} title="Refresh: re-run the search and reload counts" aria-label="Refresh"><Icon name="refresh" size="sm" /></button>
        <span className="l">{p.footLeft}</span>
        <span className="r">
          {p.footHints}
          <button className="ver" onClick={p.onSettings} title="Version & release notes">v{p.version}</button>
        </span>
      </div>
    </div>
  );
});
