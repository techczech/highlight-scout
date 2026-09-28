// The empty window (HS-M1B frame 4): the corpus sizes, Classic's grammar line
// and a short table of the Classic keys, all unchanged. With an empty
// highlight library, Classic's "No highlights yet" and Import instead.
export interface WelcomeProps {
  counts: string;
  noHighlights: boolean;
  onImport: () => void;
}

export const GRAMMAR_LINE = `cat OR dog · "exact phrase" · -exclude · prefix* · au:scott ty:books y:2023 · co:red · i: · /\\bAI\\b/`;

const KEYS: Array<[string, string]> = [
  ["Filters, colour, tags, group, sort, rows", "⌘⇧I"],
  ["Filter by tag", "⌘⇧T"],
  ["Cycle group · sort · rows", "⌘⇧G · ⌘⇧S · ⌘⇧D"],
  ["Clear colour filter", "⌘⇧X"],
  ["Toggle reading pane", "⌘\\"],
  ["Keyword / Semantic", "the switch in the search box"],
];

export function Welcome({ counts, noHighlights, onImport }: WelcomeProps) {
  return (
    <div className="qf-welcome" data-testid="welcome">
      {noHighlights ? (
        <>
          <p className="counts">No highlights yet.</p>
          <button className="import" onClick={onImport}>Import highlights →</button>
          <p className="grammar">CSV, Kindle, JSON, Readwise or Zotero — no account required for files.</p>
        </>
      ) : (
        counts && <p className="counts">{counts}</p>
      )}
      <p className="grammar">{GRAMMAR_LINE}</p>
      <div className="keys">
        <div className="h">Classic tools, same keys</div>
        <table>
          <tbody>
            {KEYS.map(([what, key]) => <tr key={what}><td>{what}</td><td>{key}</td></tr>)}
          </tbody>
        </table>
      </div>
    </div>
  );
}
