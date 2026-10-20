// The set page's pieces (HS-2A): an item row, a seed row, a suggestion card
// and the export panel. Presentational and hook-free; SetPage owns the state
// and the keyboard. Quotes are original text, never HTML.
import type { ReactNode } from "react";
import { corpusBadge } from "../../lib/archive";
import { emphasizeSegments } from "../../lib/format";
import { EXPORTS, type ExportKind, type ResolvedItem, type Suggestion, type SuggestionState } from "../../lib/sets";
import { Icon } from "../quickfinder/icons";

const cls = (c: string) => (c === "writing" || c === "tweets" || c === "highlights" ? c : "other");

/** The meta line and the title of an item, from the engine's citation. */
export function itemHead(r: ResolvedItem): { meta: string[]; title: string; titleInMeta: boolean } {
  const c = r.cited;
  if (!c) return { meta: [r.item.source_path], title: "", titleInMeta: false };
  const writing = r.item.corpus === "writing";
  const meta = [c.date_display, !writing ? c.author : null, !writing ? c.title : null].filter(Boolean) as string[];
  return { meta, title: writing ? c.title : "", titleInMeta: !writing };
}

export interface ItemRowProps {
  index: number;
  row: ResolvedItem;
  selected: boolean;
  editing: boolean;
  /** Saved-search sets show a pin. */
  pinnable: boolean;
  dragOver: boolean;
  onSelect: () => void;
  onNote: () => void;
  onNoteSave: (note: string) => void;
  onNoteCancel: () => void;
  onPin: () => void;
  onDragStart: () => void;
  onDragOver: () => void;
  onDrop: () => void;
  onOpen: () => void;
}

export function ItemRow(p: ItemRowProps) {
  const { row } = p;
  const h = itemHead(row);
  const text = row.cited?.quote?.trim() || row.item.quote;
  return (
    <div
      className={`sp-row${p.selected ? " on" : ""}${p.dragOver ? " over" : ""}`}
      data-testid="set-item"
      data-index={p.index}
      data-status={row.status}
      role="option"
      aria-selected={p.selected}
      draggable
      onClick={p.onSelect}
      onDoubleClick={p.onOpen}
      onDragStart={p.onDragStart}
      onDragOver={(e) => { e.preventDefault(); p.onDragOver(); }}
      onDrop={(e) => { e.preventDefault(); p.onDrop(); }}
      ref={p.selected ? (el) => el?.scrollIntoView({ block: "nearest" }) : undefined}
    >
      <span className="no">{p.index + 1}</span>
      <span className="grip" title="Drag to reorder (or ⌥↑ ⌥↓)" aria-hidden="true">⠿</span>
      <div className="body">
        <div className="qf-meta">
          <span className={`qf-src ${cls(row.item.corpus)}`}>{corpusBadge(row.item.corpus)}</span>
          {h.meta.length > 0 && <span className="tail">{h.meta.join(" · ")}</span>}
          {row.status === "changed" && <span className="chg" title="The text is no longer where the set found it. The quote below is what was saved.">source changed</span>}
          {row.status === "repaired" && <span className="rep" title="The passage moved in its file; the set found it again.">moved · found again</span>}
          {p.pinnable && (
            <button className={`pin${row.item.pinned ? " on" : ""}`} data-action="pin" onClick={(e) => { e.stopPropagation(); p.onPin(); }} title={row.item.pinned ? "Pinned: stays when the matches are refreshed" : "Pin: keep this when the matches are refreshed"}>
              {row.item.pinned ? "pinned" : "pin"}
            </button>
          )}
        </div>
        {h.title && <div className="qf-title">{h.title}</div>}
        <div className="qf-snip full">{text}</div>
        {p.editing ? (
          <div className="note editing">
            <Icon name="cite" size="sm" />
            <input
              autoFocus
              defaultValue={row.item.note}
              placeholder="Add a note…"
              aria-label={`Note for item ${p.index + 1}`}
              onKeyDown={(e) => {
                e.stopPropagation();
                if (e.key === "Enter") p.onNoteSave(e.currentTarget.value);
                else if (e.key === "Escape") p.onNoteCancel();
              }}
              onBlur={(e) => p.onNoteSave(e.currentTarget.value)}
            />
          </div>
        ) : (
          <button className={`note${row.item.note ? "" : " empty"}`} data-action="note" onClick={(e) => { e.stopPropagation(); p.onNote(); }}>
            <Icon name="cite" size="sm" />{row.item.note || "Add a note…"}
          </button>
        )}
      </div>
    </div>
  );
}

/** A seed in the compact list above the suggestions. */
export function SeedRow({ index, row }: { index: number; row: ResolvedItem }) {
  const c = row.cited;
  const label = c ? (row.item.corpus === "writing" ? c.title : c.author || c.title) : row.item.source_path;
  return (
    <div className="sp-seed" data-testid="seed-row">
      <span className="no">{index + 1}</span>
      <span className={`qf-src ${cls(row.item.corpus)}`}>{corpusBadge(row.item.corpus)}</span>
      <span className="t">{label}</span>
    </div>
  );
}

function Underlined({ text, terms }: { text: string; terms: string[] }): ReactNode {
  return <>{emphasizeSegments(text, terms).map((s, i) => (s.match ? <u key={i}>{s.text}</u> : <span key={i}>{s.text}</span>))}</>;
}

export interface SuggestionCardProps {
  s: Suggestion;
  state: SuggestionState;
  selected: boolean;
  /** Its item number once added. */
  addedAs: number | null;
  onSelect: () => void;
  onAdd: () => void;
  onReject: () => void;
}

export function SuggestionCard(p: SuggestionCardProps) {
  const { s } = p;
  const meta = [s.date_display, s.corpus !== "writing" ? s.author : null, s.corpus !== "writing" ? s.title : null].filter(Boolean) as string[];
  return (
    <div className={`sp-sug ${p.state}${p.selected ? " on" : ""}`} data-testid="suggestion" data-state={p.state} data-passage={s.passage_id} onClick={p.onSelect}>
      <div className="qf-meta">
        <span className={`qf-src ${cls(s.corpus)}`}>{corpusBadge(s.corpus)}</span>
        {meta.length > 0 && <span className="tail">{meta.join(" · ")}</span>}
      </div>
      {s.corpus === "writing" && <div className="qf-title">{s.title}</div>}
      <div className="qf-snip full why"><Underlined text={s.quote} terms={s.shared_terms} /></div>
      <div className="sim"><Icon name="spark" size="sm" />Similar to <b>{s.similar_to}</b>{s.shared_terms.length > 0 && <span className="terms"> · shared: {s.shared_terms.join(", ")}</span>}</div>
      {p.state === "open" && (
        <div className="qf-acts">
          <button className="qf-act add" data-action="add" onClick={(e) => { e.stopPropagation(); p.onAdd(); }}><Icon name="check" size="sm" />Add <kbd>Y</kbd></button>
          <button className="qf-act" data-action="reject" onClick={(e) => { e.stopPropagation(); p.onReject(); }}><Icon name="x" size="sm" />Not this <kbd>N</kbd></button>
        </div>
      )}
      {p.state === "added" && <div className="qf-acts"><span className="qf-act done"><Icon name="check" size="sm" />Added{p.addedAs ? ` as item ${p.addedAs}` : ""}</span></div>}
      {p.state === "rejected" && <div className="qf-acts"><span className="qf-act soft">Rejected · will not be suggested again</span></div>}
    </div>
  );
}

export interface ExportPanelProps {
  disabled: boolean;
  busy: ExportKind | null;
  onExport: (k: ExportKind) => void;
  onCopy: (k: ExportKind) => void;
}

const EXPORT_ICON: Record<ExportKind, "doc" | "pane" | "cite"> = { writeflex: "doc", talkweaver: "pane", citations: "cite" };

export function ExportPanel(p: ExportPanelProps) {
  return (
    <aside className="sp-export" aria-label="Export this set">
      <div className="rl">Export this set</div>
      {EXPORTS.map((x) => (
        <div key={x.kind} className="card">
          <button className="main" data-export={x.kind} disabled={p.disabled || p.busy !== null} onClick={() => p.onExport(x.kind)} title={`Save as a file${x.kind === "talkweaver" ? " TalkWeaver opens" : x.kind === "writeflex" ? " WriteFlex opens" : ""}`}>
            <span className="ic"><Icon name={EXPORT_ICON[x.kind]} size="sm" /></span>
            <span className="tx"><b>{x.title}</b><span>{x.text}</span></span>
          </button>
          <button className="copy" data-copy={x.kind} disabled={p.disabled || p.busy !== null} onClick={() => p.onCopy(x.kind)} title="Copy the text instead of saving a file">Copy</button>
        </div>
      ))}
      <div className="note">Saved as a file in the archive. Reopens with the same order and notes; every item still opens its source.</div>
    </aside>
  );
}
