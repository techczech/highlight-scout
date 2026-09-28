// The quick finder window (HS-1B): search row, rail | grouped results |
// reading pane, then any red sync lines and the footer. Layout only; App
// owns the state and the keyboard.
import { forwardRef, type ReactNode } from "react";
import { GroupedResults, type GroupedResultsProps } from "./GroupedResults";
import { Pane, type PaneProps } from "./Pane";
import { Rail, type RailProps } from "./Rail";
import { Icon } from "./icons";

export interface QuickFinderProps {
  query: string;
  onQuery: (q: string) => void;
  loading: boolean;
  rail: RailProps;
  results: GroupedResultsProps;
  pane: PaneProps;
  showPane: boolean;
  /** Red sync failure lines, above the footer. */
  alerts: ReactNode;
  footLeft: ReactNode;
  version: string;
  onClassic: () => void;
  onSettings: () => void;
}

export const QuickFinder = forwardRef<HTMLInputElement, QuickFinderProps>(function QuickFinder(p, ref) {
  return (
    <div className="qf flex h-full min-h-0 flex-col" data-testid="quick-finder">
      <div className="qf-search">
        <Icon name="search" size="lg" />
        <input
          ref={ref}
          type="text"
          value={p.query}
          onChange={(e) => p.onQuery(e.target.value)}
          placeholder="Search… writing, tweets and highlights"
          autoCorrect="off"
          autoCapitalize="off"
          spellCheck={false}
          aria-label="Search writing, tweets and highlights"
        />
        {p.loading && <span className="qf-spin" aria-label="Searching" />}
        <span className="small">actions on selection</span>
        <kbd>⌘K</kbd>
        <button className="tool" onClick={p.onClassic} title="The highlight index search: filters, colours, semantic search">Classic search</button>
        <button className="tool" onClick={p.onSettings} title="Settings (⌘,)" aria-label="Settings"><Icon name="gear" size="sm" /></button>
      </div>
      <div className="qf-main">
        <Rail {...p.rail} />
        <GroupedResults {...p.results} />
        {p.showPane && <Pane {...p.pane} />}
      </div>
      {p.alerts}
      <div className="qf-foot">
        <span className="l">{p.footLeft}</span>
        <span className="r">
          <span>↑↓ nav</span>
          <span><kbd>⌥↓</kbd> next corpus</span>
          <span><kbd>⌘C</kbd> quote</span>
          <span><kbd>⌘⇧C</kbd> + citation</span>
          <span><kbd>esc</kbd> hide</span>
          <button className="ver" onClick={p.onSettings} title="Version & release notes">v{p.version}</button>
        </span>
      </div>
    </div>
  );
});
