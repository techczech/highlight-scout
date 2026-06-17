import { convertFileSrc } from "@tauri-apps/api/core";
import { openUrl, openPath } from "@tauri-apps/plugin-opener";
import { useEffect, useState } from "react";
import { workHighlights } from "../lib/api";
import { copyText } from "../lib/clipboard";
import { openWorkWindow } from "../lib/window";
import type { SearchResult } from "../types";
import { resolveColor } from "../types";
import { authorLabel, formatDate, markdownQuote, workMarkdownPath } from "../lib/format";
import { renderMarkdown } from "../lib/markdown";
import { Overlay } from "./TagPicker";

/** The header toolbar + scrollable highlight list for a work. Shared by the
 * in-app overlay (WorkView) and the standalone window (WorkWindow). */
export function WorkBody({
  work,
  archiveRoot,
  onToast,
  showNewWindow = true,
}: {
  work: SearchResult;
  archiveRoot: string;
  onToast: (msg: string) => void;
  showNewWindow?: boolean;
}) {
  const [rows, setRows] = useState<SearchResult[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    workHighlights(work.work_id)
      .then((r) => {
        if (!cancelled) setRows(r);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [work.work_id]);

  const copyAll = async () => {
    await copyText(rows.map((r) => markdownQuote(r)).join("\n\n"));
    onToast(`Copied ${rows.length} highlights`);
  };

  const maxLoc = Math.max(0, ...rows.map((r) => parseInt(r.location || "", 10)).filter(Number.isFinite));

  return (
    <>
      <div className="mb-2 flex flex-wrap items-center gap-2 text-xs text-zinc-500">
        <span className="font-medium text-zinc-700">{authorLabel(work)}</span>
        <span className="rounded bg-zinc-100 px-1.5 py-0.5">{work.work_type}</span>
        <span>{rows.length} highlights</span>
        <span className="ml-auto flex flex-wrap gap-1">
          {showNewWindow && (
            <button
              onClick={() => openWorkWindow(work.work_id, work.title).catch(() => onToast("Could not open window"))}
              className="rounded bg-zinc-100 px-2 py-1 hover:bg-zinc-200"
            >
              New window
            </button>
          )}
          <button onClick={copyAll} className="rounded bg-zinc-100 px-2 py-1 hover:bg-zinc-200">Copy all</button>
          <button
            onClick={() => openPath(workMarkdownPath(archiveRoot, work.slug)).catch(() => onToast("Markdown file not found"))}
            className="rounded bg-zinc-100 px-2 py-1 hover:bg-zinc-200"
          >
            Open Markdown
          </button>
          {work.citation && (
            <button
              onClick={() => copyText(work.citation!).then(() => onToast("Citation copied"))}
              className="rounded bg-zinc-100 px-2 py-1 hover:bg-zinc-200"
            >
              Copy citation
            </button>
          )}
          {work.zotero_link ? (
            <button onClick={() => openUrl(work.zotero_link!)} className="rounded bg-zinc-100 px-2 py-1 hover:bg-zinc-200">Open in Zotero</button>
          ) : (
            work.url && (
              <button onClick={() => openUrl(work.url!)} className="rounded bg-zinc-100 px-2 py-1 hover:bg-zinc-200">Source</button>
            )
          )}
        </span>
      </div>

      {work.citation && (
        <p className="mb-2 rounded bg-zinc-50 p-2 text-xs leading-relaxed text-zinc-600">{work.citation}</p>
      )}
      {work.collections.length > 0 && (
        <div className="mb-2 flex flex-wrap items-center gap-1 text-xs">
          <span className="text-zinc-400">📁</span>
          {work.collections.map((c) => (
            <span key={c} className="rounded bg-emerald-50 px-1.5 py-0.5 text-emerald-700">{c}</span>
          ))}
        </div>
      )}

      <div className="flex-1 overflow-y-auto">
        {loading && <p className="p-3 text-sm text-zinc-400">Loading…</p>}
        {rows.map((r, i) => {
          const colorDot = resolveColor(r.annotation_color);
          const loc = parseInt(r.location || "", 10);
          const caption = [
            `${i + 1} of ${rows.length}`,
            Number.isFinite(loc) && maxLoc ? `location ${loc} of ${maxLoc}` : "",
            formatDate(r.highlighted_at),
          ]
            .filter(Boolean)
            .join(" · ");
          return (
            <div key={r.highlight_id} className="border-b border-zinc-100 py-3">
              <div className="flex items-start gap-2">
                {colorDot && <span className="mt-1.5 h-2 w-2 shrink-0 rounded-full" style={{ backgroundColor: colorDot }} />}
                <div className="min-w-0 flex-1">
                  {r.format === "image" && r.asset_path ? (
                    <img src={convertFileSrc(r.asset_path)} alt="annotation" className="max-h-72 rounded border border-zinc-200" />
                  ) : (
                    <div className="text-sm leading-relaxed text-zinc-700">{renderMarkdown(r.text)}</div>
                  )}
                  {r.note && <p className="mt-1 rounded bg-zinc-50 p-2 text-sm text-zinc-500">Note: {r.note}</p>}
                  <p className="mt-1 text-xs italic text-zinc-400">{caption}</p>
                </div>
                <button
                  onClick={() => copyText(r.text).then(() => onToast("Copied"))}
                  className="shrink-0 rounded px-1.5 py-0.5 text-xs text-zinc-400 hover:bg-zinc-100"
                  title="Copy highlight"
                >
                  Copy
                </button>
              </div>
            </div>
          );
        })}
      </div>
    </>
  );
}

interface Props {
  work: SearchResult;
  archiveRoot: string;
  onClose: () => void;
  onToast: (msg: string) => void;
}

export function WorkView({ work, archiveRoot, onClose, onToast }: Props) {
  return (
    <Overlay title={work.title} onClose={onClose} wide>
      <WorkBody work={work} archiveRoot={archiveRoot} onToast={onToast} />
    </Overlay>
  );
}
