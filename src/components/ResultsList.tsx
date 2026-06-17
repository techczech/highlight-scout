import { useEffect, useRef } from "react";
import type { SearchResult } from "../types";
import { resolveColor } from "../types";
import type { Section } from "../lib/grouping";
import { compact, typeIcon } from "../lib/format";

interface Props {
  rows: SearchResult[];
  sections: Section[] | null;
  activeId: string | null;
  onActivate: (id: string) => void;
  onOpenDetail: (id: string) => void;
  onScrollEnd: () => void;
}

export function ResultsList({
  rows,
  sections,
  activeId,
  onActivate,
  onOpenDetail,
  onScrollEnd,
}: Props) {
  const handleScroll = (e: React.UIEvent<HTMLDivElement>) => {
    const el = e.currentTarget;
    if (el.scrollHeight - el.scrollTop - el.clientHeight < 300) onScrollEnd();
  };

  return (
    <div className="flex-1 overflow-y-auto" onScroll={handleScroll}>
      {sections
        ? sections.map((section) => (
            <div key={section.id}>
              <div className="sticky top-0 z-10 flex items-baseline gap-2 bg-zinc-100/95 px-4 py-1 backdrop-blur">
                <span className="text-xs font-semibold text-zinc-600 truncate">{section.title}</span>
                {section.subtitle && <span className="text-xs text-zinc-400">{section.subtitle}</span>}
              </div>
              {section.rows.map((row) => (
                <Row
                  key={`${section.id}:${row.highlight_id}`}
                  row={row}
                  active={activeId === row.highlight_id}
                  onActivate={() => onActivate(row.highlight_id)}
                  onOpen={() => onOpenDetail(row.highlight_id)}
                />
              ))}
            </div>
          ))
        : rows.map((row) => (
            <Row
              key={row.highlight_id}
              row={row}
              active={activeId === row.highlight_id}
              onActivate={() => onActivate(row.highlight_id)}
              onOpen={() => onOpenDetail(row.highlight_id)}
            />
          ))}
    </div>
  );
}

function Row({
  row,
  active,
  onActivate,
  onOpen,
}: {
  row: SearchResult;
  active: boolean;
  onActivate: () => void;
  onOpen: () => void;
}) {
  const ref = useRef<HTMLButtonElement>(null);
  const colorDot = resolveColor(row.annotation_color);

  useEffect(() => {
    if (active) ref.current?.scrollIntoView({ block: "nearest" });
  }, [active]);

  return (
    <button
      ref={ref}
      onMouseMove={onActivate}
      onClick={onActivate}
      onDoubleClick={onOpen}
      className={`flex w-full items-center gap-2 border-b border-zinc-100 px-4 py-2 text-left ${
        active ? "bg-amber-50" : "hover:bg-zinc-50"
      }`}
    >
      <span className="shrink-0 text-sm opacity-70">{typeIcon(row)}</span>
      {colorDot && (
        <span className="h-2 w-2 shrink-0 rounded-full" style={{ backgroundColor: colorDot }} />
      )}
      <span className="truncate text-sm text-zinc-800">
        {row.format === "image" ? "🖼 " : ""}
        {compact(row.text || (row.format === "image" ? "[image annotation]" : ""), 240)}
      </span>
    </button>
  );
}
