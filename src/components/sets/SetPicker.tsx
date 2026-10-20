// Which set: shown by + Set when there is no current set, by "New set" in the
// rail, and by "Save this search as a set". A list of the archive's sets to
// pick from (add mode) and a field to start a new one.
import { useState } from "react";
import type { SetSummary } from "../../lib/sets";
import { trayLabel } from "../../lib/sets";
import "./sets.css";

export interface SetPickerProps {
  /** `add`: pick a set or start one; `new`: start an empty set; `search`: save the search as a set. */
  mode: "add" | "new" | "search";
  sets: SetSummary[];
  defaultTitle?: string;
  /** The query a `search` set follows. */
  query?: string;
  busy?: boolean;
  error?: string;
  onPick: (id: string) => void;
  onCreate: (title: string) => void;
  onClose: () => void;
}

const HEADING = { add: "Add to a set", new: "New set", search: "Save this search as a set" } as const;

export function SetPicker(p: SetPickerProps) {
  const [title, setTitle] = useState(p.defaultTitle ?? "");
  const canPick = p.mode === "add" && p.sets.length > 0;
  return (
    <div className="sp-picker-back" data-testid="set-picker" onMouseDown={(e) => { if (e.target === e.currentTarget) p.onClose(); }}>
      <div className="sp-picker" role="dialog" aria-label={HEADING[p.mode]} onKeyDown={(e) => { if (e.key === "Escape") { e.stopPropagation(); p.onClose(); } }}>
        <h2>{HEADING[p.mode]}</h2>
        {p.mode === "search" && p.query && <p className="hint">A set that follows “{p.query}”: new matches appear after a sync. Pin the ones to keep.</p>}
        {canPick && (
          <div className="list">
            {p.sets.map((s) => (
              <button key={s.id} data-set={s.id} disabled={p.busy} onClick={() => p.onPick(s.id)}>{trayLabel(s.title, s.items)}</button>
            ))}
          </div>
        )}
        <form onSubmit={(e) => { e.preventDefault(); if (title.trim()) p.onCreate(title.trim()); }}>
          <label>{canPick ? "Or start a new set" : "Name the set"}
            <input autoFocus value={title} onChange={(e) => setTitle(e.target.value)} placeholder="Metaphor talk" disabled={p.busy} />
          </label>
          <div className="row">
            <button type="button" onClick={p.onClose}>Cancel</button>
            <button type="submit" className="pri" disabled={!title.trim() || p.busy}>{p.mode === "add" ? "Create and add" : "Create"}</button>
          </div>
        </form>
        {p.error && <p className="warn">{p.error}</p>}
      </div>
    </div>
  );
}
