import { Overlay } from "./TagPicker";
import type { ImportAction } from "./Toolbar";

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
      { action: "zotero", label: "Import Zotero", hint: "local database" },
      { action: "readwise-seed", label: "Seed from a Readwise archive", hint: "advanced" },
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

/** The import-source buttons, reused in the empty-state overlay and Settings. */
export function ImportButtons({ onPick }: { onPick: (a: ImportAction) => void }) {
  return (
    <div className="flex flex-col gap-4">
      {IMPORT_GROUPS.map((g) => (
        <div key={g.title}>
          <p className="mb-1.5 text-xs font-semibold uppercase tracking-wide text-zinc-400">{g.title}</p>
          <div className="flex flex-col gap-1">
            {g.items.map((it) => (
              <button
                key={it.action}
                onClick={() => onPick(it.action)}
                className="flex items-baseline justify-between rounded-lg border border-zinc-200 px-3 py-2 text-left text-sm hover:border-amber-400 hover:bg-amber-50"
              >
                <span className="font-medium text-zinc-700">{it.label}</span>
                {it.hint && <span className="text-xs text-zinc-400">{it.hint}</span>}
              </button>
            ))}
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
