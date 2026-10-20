// The reading pane for a highlight (HS-M1B): Classic's pane in the quick
// finder's frame. Everything Classic's pane had stays: text or image, note,
// citation, Open PDF in Zotero and the source link, Copy ▾, the metadata line
// (colour, "n of m · location x of y", date, type, collections, tags), Find
// related, Show work highlights (the work's highlights in order: see in
// context) and New window; the foot can show what ⌘⇧C copies, as for any corpus.
// The quote keeps Classic's reading size: 15px at line height 1.625.
import { convertFileSrc } from "@tauri-apps/api/core";
import { SOURCE_LABEL, type CopyFormat } from "../../lib/quickFinder";
import { CopyPreview } from "./CopyPreview";
import { compact, formatDate, isZotero, originalUrl, shortUrl, uniqueTags } from "../../lib/format";
import { renderMarkdown } from "../../lib/markdown";
import { toMarkdown } from "../../lib/copyFormats";
import { resolveColor, type SearchResult, type WorkPosition } from "../../types";
import { CopyMenu } from "../CopyMenu";
import { Icon } from "./icons";

export interface HighlightPaneProps {
  row: SearchResult | null;
  terms: string[];
  position: WorkPosition | null;
  format: CopyFormat;
  onFormat: (f: CopyFormat) => void;
  /** What ⌘⇧C copies is shown (folded away by default). */
  copyPreview: boolean;
  onCopyPreview: () => void;
  onOpenUrl: (url: string) => void;
  onFindRelated: (row: SearchResult) => void;
  onShowWork: (row: SearchResult) => void;
  onNewWindow: (row: SearchResult) => void;
  onToast: (msg: string) => void;
}

/** Classic's "3 of 12 · location 2011 of 4400" (or "location 2011" before the position loads). */
export function positionLine(row: SearchResult, position: WorkPosition | null): string | null {
  if ((row.work_type || "").toLowerCase().startsWith("tweet") || !row.location) return null;
  return position ? `${position.pos} of ${position.total} · location ${row.location} of ${position.max_loc}` : `location ${row.location}`;
}

export function HighlightPane(p: HighlightPaneProps) {
  const row = p.row;
  if (!row) return <div className="qf-pane" data-testid="highlight-pane"><p className="none">Select a highlight to read it in full</p></div>;
  const isTweet = (row.work_type || "").toLowerCase().startsWith("tweet");
  const url = originalUrl(row);
  const dot = resolveColor(row.annotation_color);
  const tags = uniqueTags(row);
  const typeLabel = isZotero(row) ? "zotero" : row.work_type;
  const source = SOURCE_LABEL[row.source_system] ?? row.source_system;
  const pos = positionLine(row, p.position);
  return (
    <div className="qf-pane hl" data-testid="highlight-pane">
      <div className="scroll">
        <div className="p-title">{compact(row.title || "Untitled", isTweet ? 60 : 200)}</div>
        <div className="p-sub">
          <span>{[row.author, row.work_type, source].filter(Boolean).join(" · ")}</span>
          {row.zotero_link && (
            <button className="link" onClick={() => p.onOpenUrl(row.zotero_link!)}>Open PDF in Zotero <Icon name="ext" size="sm" /></button>
          )}
          {url && <button className="link" onClick={() => p.onOpenUrl(url)} title={url}>{shortUrl(url)}</button>}
        </div>

        {row.format === "image" && row.asset_path ? (
          <img className="p-img" src={convertFileSrc(row.asset_path)} alt="annotation" />
        ) : (
          <blockquote className="p-quote classic" data-testid="pane-quote" style={dot ? { borderLeftColor: dot } : undefined}>
            {renderMarkdown((row.text || "").trim(), p.terms)}
          </blockquote>
        )}

        {row.note && (
          <div className="p-block"><div className="lab">Note</div><p className="box pre">{row.note}</p></div>
        )}
        {row.citation && (
          <div className="p-block"><div className="lab">Citation</div><p className="box small">{row.citation}</p></div>
        )}

        <div className="p-metaline" data-testid="pane-meta">
          {dot && <span className="dot" style={{ backgroundColor: dot }} />}
          {row.annotation_color && <span>{row.annotation_color}</span>}
          {pos && <span data-testid="pane-position">{pos}</span>}
          {row.highlighted_at && <span>{formatDate(row.highlighted_at)}</span>}
          {typeLabel && <span className="pill">{typeLabel}</span>}
          <CopyMenu row={row} onToast={p.onToast} />
        </div>
        {row.collections.length > 0 && (
          <div className="p-pills">📁 {row.collections.map((c) => <span key={c} className="coll">{c}</span>)}</div>
        )}
        {tags.length > 0 && <div className="p-pills">{tags.map((t) => <span key={t} className="tag">{t}</span>)}</div>}
      </div>

      <CopyPreview open={p.copyPreview} onToggle={p.onCopyPreview} format={p.format} onFormat={p.onFormat} text={toMarkdown(row).trimEnd()}>
        <button className="qf-act violet" onClick={() => p.onFindRelated(row)} title="Find semantically related highlights in a new window (⌘⇧F)">
          <Icon name="spark" size="sm" />Find related <kbd>⌘⇧F</kbd>
        </button>
        <button className="qf-act" onClick={() => p.onShowWork(row)} title="Every highlight of this work, in order (⌘⇧L)">
          Show work highlights → <kbd>⌘⇧L</kbd>
        </button>
        <button className="qf-act" onClick={() => p.onNewWindow(row)} title="Open this work in its own window (⌘⇧N)">
          ⧉ New window
        </button>
      </CopyPreview>
    </div>
  );
}
