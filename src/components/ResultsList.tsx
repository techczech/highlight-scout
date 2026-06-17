import { useEffect, useRef } from "react";
import type { Density, SearchResult } from "../types";
import { resolveColor } from "../types";
import type { Section } from "../lib/grouping";
import { compact, formatDate, typeIcon } from "../lib/format";

interface Props {
  rows: SearchResult[];
  sections: Section[] | null;
  density: Density;
  activeId: string | null;
  onActivate: (id: string) => void;
  onOpenDetail: (id: string) => void;
  onScrollEnd: () => void;
}

export function ResultsList({
  rows,
  sections,
  density,
  activeId,
  onActivate,
  onOpenDetail,
  onScrollEnd,
}: Props) {
  const handleScroll = (e: React.UIEvent<HTMLDivElement>) => {
    const el = e.currentTarget;
    if (el.scrollHeight - el.scrollTop - el.clientHeight < 300) onScrollEnd();
  };

  const renderRow = (row: SearchResult) => (
    <Row
      key={row.highlight_id}
      row={row}
      density={density}
      active={activeId === row.highlight_id}
      onActivate={() => onActivate(row.highlight_id)}
      onOpen={() => onOpenDetail(row.highlight_id)}
    />
  );

  return (
    <div className="flex-1 overflow-y-auto" onScroll={handleScroll}>
      {sections
        ? sections.map((section) => (
            <div key={section.id}>
              <div className="sticky top-0 z-10 flex items-baseline gap-2 bg-zinc-100/95 px-4 py-1 backdrop-blur">
                <span className="truncate text-xs font-semibold text-zinc-600">{section.title}</span>
                {section.subtitle && <span className="text-xs text-zinc-400">{section.subtitle}</span>}
              </div>
              {section.rows.map(renderRow)}
            </div>
          ))
        : rows.map(renderRow)}
    </div>
  );
}

function Row({
  row,
  density,
  active,
  onActivate,
  onOpen,
}: {
  row: SearchResult;
  density: Density;
  active: boolean;
  onActivate: () => void;
  onOpen: () => void;
}) {
  const ref = useRef<HTMLButtonElement>(null);
  const colorDot = resolveColor(row.annotation_color);

  useEffect(() => {
    if (active) ref.current?.scrollIntoView({ block: "nearest" });
  }, [active]);

  const quoteClass =
    density === "compact"
      ? "truncate"
      : density === "comfortable"
        ? "line-clamp-3"
        : "whitespace-pre-wrap";

  const showMeta = density !== "compact";
  const date = formatDate(row.highlighted_at);

  return (
    <button
      ref={ref}
      onMouseMove={onActivate}
      onClick={onActivate}
      onDoubleClick={onOpen}
      className={`flex w-full items-start gap-2 border-b border-zinc-100 px-4 py-2 text-left ${
        active ? "bg-amber-50" : "hover:bg-zinc-50"
      }`}
    >
      <span className="mt-0.5 shrink-0 text-sm opacity-70">{typeIcon(row)}</span>
      {colorDot && (
        <span className="mt-1.5 h-2 w-2 shrink-0 rounded-full" style={{ backgroundColor: colorDot }} />
      )}
      <span className="min-w-0 flex-1">
        <span className={`block text-sm text-zinc-800 ${quoteClass}`}>
          {row.format === "image" ? "🖼 " : ""}
          {density === "compact"
            ? compact(row.text || (row.format === "image" ? "[image annotation]" : ""), 240)
            : row.text || (row.format === "image" ? "[image annotation]" : "")}
        </span>
        {showMeta && (
          <span className="mt-1 flex flex-wrap items-center gap-1.5 text-xs text-zinc-400">
            <span className="truncate font-medium text-zinc-500">{row.title}</span>
            {row.author && <span>· {row.author}</span>}
            {date && <span>· {date}</span>}
            {density === "full" &&
              row.tags.slice(0, 4).map((t) => (
                <span key={t} className="rounded bg-blue-50 px-1 text-blue-600">{t}</span>
              ))}
          </span>
        )}
      </span>
    </button>
  );
}
