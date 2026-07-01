import { Overlay } from "./TagPicker";
import type { ImportAction } from "./Toolbar";
import { ocrAvailable } from "../lib/api";

export const IMPORT_GROUPS: Array<{
  title: string;
  items: Array<{ action: ImportAction; label: string; hint?: string }>;
}> = [
  {
    title: "From a file — no account needed",
    items: [
      { action: "csv", label: "CSV", hint: "any export; map the columns" },
      { action: "kindle", label: "Kindle clippings", hint: "My Clippings.txt" },
      { action: "json", label: "JSON", hint: "Highlight Scout's own format" },
      { action: "x", label: "X / Twitter saved", hint: "likes + bookmarks (saved.jsonl)" },
      { action: "export-json", label: "Export all to JSON…", hint: "back up everything" },
    ],
  },
  {
    title: "Connected sources",
    items: [
      { action: "readwise", label: "Update from Readwise", hint: "needs an API token (Settings)" },
      { action: "readwise-tweets", label: "Readwise saved tweets", hint: "full text via Reader" },
      { action: "zotero", label: "Import Zotero", hint: "local database" },
    ],
  },
  {
    title: "Maintenance",
    items: [
      { action: "qmd-reindex", label: "Rebuild semantic index (QMD)" },
      { action: "log", label: "View import log…" },
    ],
  },
];

const importBtn = (action: ImportAction, label: string, hint: string | undefined, onPick: (a: ImportAction) => void) => (
  <button
    key={action}
    onClick={() => onPick(action)}
    className="flex items-baseline justify-between rounded-lg border border-zinc-200 px-3 py-2 text-left text-sm hover:border-amber-400 hover:bg-amber-50"
  >
    <span className="font-medium text-zinc-700">{label}</span>
    {hint && <span className="text-xs text-zinc-400">{hint}</span>}
  </button>
);

/** The import-source buttons, reused in the empty-state overlay and Settings. */
export function ImportButtons({ onPick }: { onPick: (a: ImportAction) => void }) {
  return (
    <div className="flex flex-col gap-4">
      {IMPORT_GROUPS.map((g) => (
        <div key={g.title}>
          <p className="mb-1.5 text-xs font-semibold uppercase tracking-wide text-zinc-400">{g.title}</p>
          <div className="flex flex-col gap-1">
            {g.items.map((it) => importBtn(it.action, it.label, it.hint, onPick))}
            {g.title === "Maintenance" && ocrAvailable() &&
              importBtn("ocr", "OCR images", "extract text from image highlights", onPick)}
          </div>
        </div>
      ))}
    </div>
  );
}

export function ImportMenu({ onPick, onClose }: { onPick: (a: ImportAction) => void; onClose: () => void }) {
  return (
    <Overlay title="Import highlights" onClose={onClose}>
      <ImportButtons onPick={onPick} />
    </Overlay>
  );
}
