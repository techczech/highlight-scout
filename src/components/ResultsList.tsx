import { useEffect, useRef } from "react";
import type { SearchResult } from "../types";
import { resolveColor } from "../types";
import { renderSnippet } from "../lib/snippet";

interface Props {
  results: SearchResult[];
  activeIndex: number;
  selected: string | null;
  onSelect: (id: string) => void;
  onActivate: (index: number) => void;
}

export function ResultsList({
  results,
  activeIndex,
  selected,
  onSelect,
  onActivate,
}: Props) {
  if (results.length === 0) return null;

  return (
    <div className="flex-1 overflow-y-auto">
      {results.map((r, i) => (
        <ResultRow
          key={r.highlight_id}
          result={r}
          isActive={i === activeIndex}
          isSelected={selected === r.highlight_id}
          onSelect={() => onSelect(r.highlight_id)}
          onHover={() => onActivate(i)}
        />
      ))}
    </div>
  );
}

function ResultRow({
  result,
  isActive,
  isSelected,
  onSelect,
  onHover,
}: {
  result: SearchResult;
  isActive: boolean;
  isSelected: boolean;
  onSelect: () => void;
  onHover: () => void;
}) {
  const ref = useRef<HTMLButtonElement>(null);
  const colorDot = resolveColor(result.annotation_color);
  const date = result.highlighted_at ? result.highlighted_at.split("T")[0] : null;

  useEffect(() => {
    if (isActive) ref.current?.scrollIntoView({ block: "nearest" });
  }, [isActive]);

  return (
    <button
      ref={ref}
      onClick={onSelect}
      onMouseMove={onHover}
      className={`w-full text-left px-4 py-3 border-b border-zinc-100 transition-colors ${
        isActive ? "bg-amber-50" : "hover:bg-zinc-50"
      } ${isSelected ? "border-l-2 border-l-amber-400" : ""}`}
    >
      <div className="flex items-start gap-2">
        {colorDot && (
          <span
            className="mt-1 h-2 w-2 shrink-0 rounded-full"
            style={{ backgroundColor: colorDot }}
          />
        )}
        <div className="min-w-0 flex-1">
          <p className="text-sm text-zinc-800 leading-snug line-clamp-2">
            {renderSnippet(result.snippet || result.text.slice(0, 200))}
          </p>
          <div className="mt-1 flex flex-wrap items-center gap-2 text-xs text-zinc-400">
            <span className="font-medium text-zinc-600 truncate max-w-[200px]">
              {result.title}
            </span>
            {result.author && <span>· {result.author}</span>}
            <span className="rounded bg-zinc-100 px-1.5 py-0.5 text-zinc-500">
              {result.source_system}
            </span>
            {date && <span>{date}</span>}
            {result.tags.slice(0, 3).map((tag) => (
              <span
                key={tag}
                className="rounded bg-blue-50 px-1.5 py-0.5 text-blue-600"
              >
                {tag}
              </span>
            ))}
          </div>
        </div>
      </div>
    </button>
  );
}
