// The foot of both reading panes (ticket 08): the pane's action bar, and what
// ⌘⇧C copies with its format, folded away behind a small disclosure at the
// end of the action bar. Closed by default, so the reading text has the
// room; the choice is remembered. Opened, it is the preview as before.
// Presentational and hook-free; the open state and its store live with the
// caller (`loadCopyPreview` / `saveCopyPreview`).
import type { ReactNode } from "react";
import { COPY_FORMAT_LABEL, type CopyFormat } from "../../lib/quickFinder";

export const COPY_PREVIEW_KEY = "quickFinder.copyPreview";

type Store = Pick<Storage, "getItem" | "setItem">;

/** Whether the preview was left open; closed unless it was. */
export function loadCopyPreview(store: Store): boolean {
  try {
    return store.getItem(COPY_PREVIEW_KEY) === "open";
  } catch {
    return false;
  }
}

export function saveCopyPreview(open: boolean, store: Store): void {
  try { store.setItem(COPY_PREVIEW_KEY, open ? "open" : "closed"); } catch { /* a convenience only */ }
}

export interface CopyPreviewProps {
  open: boolean;
  onToggle: () => void;
  format: CopyFormat;
  onFormat: (f: CopyFormat) => void;
  /** What ⌘⇧C copies, as Markdown. */
  text: string;
  /** The pane's action buttons. */
  children: ReactNode;
}

export function CopyPreview(p: CopyPreviewProps) {
  return (
    <div className="copies" data-testid="copies">
      {p.open && (
        <>
          <div className="cp-head">
            <span className="lab">⌘⇧C copies</span>
            <select value={p.format} onChange={(e) => p.onFormat(e.target.value as CopyFormat)} aria-label="Citation format">
              {(Object.keys(COPY_FORMAT_LABEL) as CopyFormat[]).map((f) => <option key={f} value={f}>{COPY_FORMAT_LABEL[f]}</option>)}
            </select>
          </div>
          <div className="cite-box" id="copy-preview" data-testid="cite-box">{p.text}</div>
        </>
      )}
      <div className="actbar">
        {p.children}
        <button
          className="cp-toggle"
          data-testid="copy-preview-toggle"
          aria-expanded={p.open}
          aria-controls="copy-preview"
          onClick={p.onToggle}
          title={p.open ? "Hide what ⌘⇧C copies" : "Show what ⌘⇧C copies, and its format"}
        >
          <span className="chev" aria-hidden="true">›</span>ⓘ ⌘⇧C copies
        </button>
      </div>
    </div>
  );
}
