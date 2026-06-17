import type { Facets, SearchMode } from "../types";
import { resolveColor } from "../types";

interface Props {
  facets: Facets | null;
  source: string | null;
  color: string | null;
  mode: SearchMode;
  onSource: (s: string | null) => void;
  onColor: (c: string | null) => void;
  onMode: (m: SearchMode) => void;
}

export function FilterBar({
  facets,
  source,
  color,
  mode,
  onSource,
  onColor,
  onMode,
}: Props) {
  return (
    <div className="flex items-center gap-2 border-b border-zinc-100 bg-zinc-50 px-4 py-1.5 text-xs overflow-x-auto">
      {/* Mode toggle — ADR-0005: keyword default, semantic fast-follow */}
      <div className="flex rounded bg-zinc-200 p-0.5">
        <button
          onClick={() => onMode("keyword")}
          className={`rounded px-2 py-0.5 ${
            mode === "keyword" ? "bg-white text-zinc-800 shadow-sm" : "text-zinc-500"
          }`}
        >
          Keyword
        </button>
        <button
          onClick={() => onMode("semantic")}
          title="Semantic search (QMD) — coming soon"
          className={`rounded px-2 py-0.5 ${
            mode === "semantic" ? "bg-white text-zinc-800 shadow-sm" : "text-zinc-400"
          }`}
        >
          Semantic
        </button>
      </div>

      <span className="text-zinc-300">|</span>

      {/* Source filter */}
      <button
        onClick={() => onSource(null)}
        className={`rounded px-2 py-0.5 ${
          source === null ? "bg-zinc-800 text-white" : "text-zinc-500 hover:bg-zinc-200"
        }`}
      >
        All
      </button>
      {facets?.sources.map((s) => (
        <button
          key={s}
          onClick={() => onSource(source === s ? null : s)}
          className={`rounded px-2 py-0.5 capitalize ${
            source === s ? "bg-zinc-800 text-white" : "text-zinc-500 hover:bg-zinc-200"
          }`}
        >
          {s}
        </button>
      ))}

      {facets && facets.colors.length > 0 && (
        <>
          <span className="text-zinc-300">|</span>
          {facets.colors.map((c) => {
            const css = resolveColor(c);
            const active = color === c;
            return (
              <button
                key={c}
                onClick={() => onColor(active ? null : c)}
                title={c}
                className={`h-4 w-4 shrink-0 rounded-full border transition-transform ${
                  active ? "scale-125 border-zinc-800" : "border-zinc-300"
                }`}
                style={{ backgroundColor: css ?? "#fff" }}
              />
            );
          })}
        </>
      )}
    </div>
  );
}
