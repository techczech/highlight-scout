import { openUrl } from "@tauri-apps/plugin-opener";
import type { SearchResult } from "../types";
import { resolveColor } from "../types";

interface Props {
  result: SearchResult;
  onClose: () => void;
}

export function HighlightDetail({ result, onClose }: Props) {
  const colorDot = resolveColor(result.annotation_color);

  const date = result.highlighted_at
    ? result.highlighted_at.split("T")[0]
    : null;

  return (
    <div className="border-t border-zinc-200 bg-white p-5 max-h-64 overflow-y-auto">
      <div className="flex items-start justify-between gap-3 mb-3">
        <div className="flex items-center gap-2 min-w-0">
          {colorDot && (
            <span
              className="h-3 w-3 shrink-0 rounded-full"
              style={{ backgroundColor: colorDot }}
            />
          )}
          <div className="min-w-0">
            <p className="font-medium text-sm text-zinc-800 truncate">
              {result.title}
            </p>
            {result.author && (
              <p className="text-xs text-zinc-500">{result.author}</p>
            )}
          </div>
        </div>
        <button
          onClick={onClose}
          className="shrink-0 text-zinc-400 hover:text-zinc-600 text-lg leading-none"
          aria-label="Close detail"
        >
          ×
        </button>
      </div>

      <blockquote className="border-l-2 border-amber-400 pl-3 text-sm text-zinc-700 leading-relaxed mb-3">
        {result.text}
      </blockquote>

      {result.note && (
        <p className="text-sm text-zinc-600 bg-zinc-50 rounded p-2 mb-3">
          {result.note}
        </p>
      )}

      <div className="flex flex-wrap items-center gap-2 text-xs text-zinc-400">
        <span className="rounded bg-zinc-100 px-1.5 py-0.5 text-zinc-500">
          {result.work_type}
        </span>
        <span className="rounded bg-zinc-100 px-1.5 py-0.5 text-zinc-500">
          {result.source_system}
        </span>
        {date && <span>{date}</span>}
        {result.tags.map((tag) => (
          <span
            key={tag}
            className="rounded bg-blue-50 px-1.5 py-0.5 text-blue-600"
          >
            {tag}
          </span>
        ))}
        {result.url && (
          <button
            onClick={() => openUrl(result.url!)}
            className="text-blue-500 hover:underline ml-auto"
          >
            Source ↗
          </button>
        )}
      </div>
    </div>
  );
}
