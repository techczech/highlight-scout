import { convertFileSrc } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { SearchResult, WorkPosition } from "../types";
import { resolveColor } from "../types";
import { copyText } from "../lib/clipboard";
import { openWorkWindow, openRelatedWindow } from "../lib/window";
import {
  compact,
  formatDate,
  isZotero,
  originalUrl,
  shortUrl,
  uniqueTags,
} from "../lib/format";
import { renderMarkdown } from "../lib/markdown";

interface Props {
  row: SearchResult | null;
  terms: string[];
  position?: WorkPosition | null;
  onShowWork: (row: SearchResult) => void;
  onToast: (msg: string) => void;
}

export function ReadingPane({ row, terms, position, onShowWork, onToast }: Props) {
  if (!row) {
    return (
      <div className="flex h-full items-center justify-center p-6 text-sm text-zinc-300">
        Select a highlight to read it in full
      </div>
    );
  }

  const isTweet = (row.work_type || "").toLowerCase().startsWith("tweet");
  const titleCap = isTweet ? 60 : 200;
  const url = originalUrl(row);
  const colorDot = resolveColor(row.annotation_color);
  const tagList = uniqueTags(row);
  const typeLabel = isZotero(row) ? "zotero" : row.work_type;

  return (
    <div className="flex h-full flex-col overflow-y-auto p-5">
      <div className="mb-3 flex items-start gap-2">
        {colorDot && (
          <span className="mt-1 h-3 w-3 shrink-0 rounded-full" style={{ backgroundColor: colorDot }} />
        )}
        <div className="min-w-0">
          <p className="text-sm font-semibold leading-snug text-zinc-800">
            {compact(row.title || "Untitled", titleCap)}
          </p>
          {row.author && <p className="text-xs text-zinc-500">{row.author}</p>}
        </div>
      </div>

      {row.format === "image" && row.asset_path ? (
        <img
          src={convertFileSrc(row.asset_path)}
          alt="annotation"
          className="mb-3 max-h-80 w-auto self-start rounded border border-zinc-200"
        />
      ) : (
        <blockquote className="mb-3 border-l-2 border-amber-400 pl-3 text-[15px] leading-relaxed text-zinc-700">
          {renderMarkdown((row.text || "").trim(), terms)}
        </blockquote>
      )}

      {row.note && (
        <div className="mb-3">
          <p className="mb-1 text-xs font-semibold uppercase tracking-wide text-zinc-400">Note</p>
          <p className="whitespace-pre-wrap rounded bg-zinc-50 p-2 text-sm text-zinc-600">{row.note}</p>
        </div>
      )}

      {row.citation && (
        <div className="mb-3">
          <p className="mb-1 text-xs font-semibold uppercase tracking-wide text-zinc-400">Citation</p>
          <p className="rounded bg-zinc-50 p-2 text-xs leading-relaxed text-zinc-600">{row.citation}</p>
        </div>
      )}

      <div className="mt-auto border-t border-zinc-100 pt-3 text-xs text-zinc-400">
        <div className="flex flex-wrap items-center gap-2">
          {row.zotero_link ? (
            <button onClick={() => openUrl(row.zotero_link!)} className="font-medium text-blue-500 hover:underline">
              Open PDF in Zotero ↗
            </button>
          ) : (
            url && (
              <button onClick={() => openUrl(url)} className="text-blue-500 hover:underline">
                {shortUrl(url)}
              </button>
            )
          )}
          {row.zotero_link && url && (
            <button onClick={() => openUrl(url)} className="text-blue-500 hover:underline">
              {shortUrl(url)}
            </button>
          )}
          {row.citation && (
            <button
              onClick={() => copyText(row.citation!).then(() => onToast("Citation copied"))}
              className="rounded bg-zinc-100 px-1.5 py-0.5 text-zinc-500 hover:bg-zinc-200"
            >
              Copy citation
            </button>
          )}
          {row.highlighted_at && <span>{formatDate(row.highlighted_at)}</span>}
          {typeLabel && <span className="rounded bg-zinc-100 px-1.5 py-0.5">{typeLabel}</span>}
        </div>
        {!isTweet && row.location && (
          <p className="mt-1 italic">
            {position
              ? `${position.pos} of ${position.total} · location ${row.location} of ${position.max_loc}`
              : `location ${row.location}`}
          </p>
        )}
        {row.collections.length > 0 && (
          <div className="mt-1 flex flex-wrap items-center gap-1">
            <span className="text-zinc-400">📁</span>
            {row.collections.map((c) => (
              <span key={c} className="rounded bg-emerald-50 px-1.5 py-0.5 text-emerald-700">{c}</span>
            ))}
          </div>
        )}
        {tagList.length > 0 && (
          <div className="mt-1 flex flex-wrap gap-1">
            {tagList.map((t) => (
              <span key={t} className="rounded bg-blue-50 px-1.5 py-0.5 text-blue-600">{t}</span>
            ))}
          </div>
        )}
        <div className="mt-3 flex flex-wrap gap-1">
          <button
            onClick={() => openRelatedWindow(row.highlight_id).catch(() => onToast("Could not open window"))}
            className="rounded bg-violet-100 px-2 py-1 text-violet-700 hover:bg-violet-200"
            title="Find semantically related highlights in a new window (⌘⇧F)"
          >
            ✦ Find related
          </button>
          <button
            onClick={() => onShowWork(row)}
            className="rounded bg-zinc-100 px-2 py-1 text-zinc-600 hover:bg-zinc-200"
          >
            Show work highlights →
          </button>
          <button
            onClick={() => openWorkWindow(row.work_id, row.title).catch(() => onToast("Could not open window"))}
            className="rounded bg-zinc-100 px-2 py-1 text-zinc-600 hover:bg-zinc-200"
            title="Open this work in its own window"
          >
            ⧉ New window
          </button>
        </div>
      </div>
    </div>
  );
}
