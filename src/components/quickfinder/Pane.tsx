// HS-1B reading pane: the passage in context (the paragraph before, dimmed;
// the matched sentence marked) and exactly what ⌘⇧C will copy.
// Presentational and hook-free. Every text is original source text.
import type { ArchiveDoc, PassageView } from "../../lib/archive";
import { COPY_FORMAT_LABEL, paneByline, splitAroundSentence, type CopyFormat } from "../../lib/quickFinder";
import { Marked } from "./GroupedResults";
import { Icon } from "./icons";

export interface PaneProps {
  doc: ArchiveDoc | null;
  passage: PassageView | null;
  citeError: string;
  terms: string[];
  passageId: string | null;
  onPassage: (id: string) => void;
  format: CopyFormat;
  onFormat: (f: CopyFormat) => void;
  /** Open the public page, else the piece in its app. */
  onOpenPiece: (url: string) => void;
  onOpenFile: (path: string) => void;
}

export function Pane(p: PaneProps) {
  const d = p.doc;
  if (!d) return <div className="qf-pane" data-testid="archive-pane"><p className="none">No result selected</p></div>;
  const v = p.passage;
  const c = v?.cited ?? null;
  const hit = d.hits.find((h) => h.passage_id === p.passageId) ?? d.hits[0];
  const parts = c ? splitAroundSentence(c.quote, hit?.quote) : null;
  const openUrl = c?.public_url ?? d.public_url ?? (c?.link?.startsWith("writeflex://") ? c.link : null);
  const openLabel = (c?.public_url ?? d.public_url) ? "Open piece" : "Open in WriteFlex";
  return (
    <div className="qf-pane" data-testid="archive-pane">
      <div className="scroll">
        <div className="p-title">{d.title || d.rel_path}</div>
        <div className="p-sub">
          <span>{paneByline(d)}</span>
          {openUrl && (
            <button className="link" onClick={() => p.onOpenPiece(openUrl)} title={openUrl}>
              {openLabel} <Icon name="ext" size="sm" />
            </button>
          )}
        </div>

        {c && parts ? (
          <div className="p-quote" data-testid="pane-quote">
            {v?.context_before && <p className="dim">{v.context_before}</p>}
            <p>
              {parts.before && <Marked text={parts.before} terms={p.terms} />}
              <span className="sent"><Marked text={parts.sentence} terms={p.terms} /></span>
              {parts.after && <Marked text={parts.after} terms={p.terms} />}
            </p>
          </div>
        ) : p.citeError ? (
          <p className="warn">{p.citeError}</p>
        ) : (
          <div className="p-quote"><p className="dim">Loading passage…</p></div>
        )}

        {d.hits.length > 1 && (
          <div className="others">
            <div className="lab" style={{ marginBottom: 4 }}>
              Passages in this {d.corpus === "highlights" ? "work" : "piece"} ({d.hits.length})
            </div>
            {d.hits.map((h) => (
              <button key={h.passage_id} className={h.passage_id === p.passageId ? "on" : ""} onClick={() => p.onPassage(h.passage_id)}>
                <span className="ln">line {h.line}</span>
                <Marked text={h.quote} terms={p.terms} />
              </button>
            ))}
          </div>
        )}
      </div>

      <div className="copies">
        <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 6 }}>
          <span className="lab">⌘⇧C copies</span>
          <select value={p.format} onChange={(e) => p.onFormat(e.target.value as CopyFormat)} aria-label="Citation format">
            {(Object.keys(COPY_FORMAT_LABEL) as CopyFormat[]).map((f) => <option key={f} value={f}>{COPY_FORMAT_LABEL[f]}</option>)}
          </select>
        </div>
        <div className="cite-box" data-testid="cite-box">{c ? c.citation.markdown.trimEnd() : "…"}</div>
        <div className="actbar">
          <button className="qf-act" disabled title="“More like these” arrives in a later release">
            <Icon name="spark" size="sm" />Find related
          </button>
          {c && (
            <button className="qf-act" onClick={() => p.onOpenFile(c.path)} title={`${c.path}:${c.line_start}`}>
              <Icon name="doc" size="sm" />Open source file
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
