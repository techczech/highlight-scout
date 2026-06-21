import { useEffect, useRef } from "react";
import type { Density, SearchResult } from "../types";
import { resolveColor } from "../types";
import type { Section } from "../lib/grouping";
import { compact, formatDate, typeIcon } from "../lib/format";
import { renderInlineMarkdown, renderMarkdown } from "../lib/markdown";

interface Props {
  rows: SearchResult[];
  sections: Section[] | null;
  density: Density;
  terms: string[];
  semantic: boolean;
  showPane: boolean;
  activeId: string | null;
  onActivate: (id: string) => void;
  onOpenDetail: (id: string) => void;
  onScrollEnd: () => void;
}

export function ResultsList({
  rows,
  sections,
  density,
  terms,
  semantic,
  showPane,
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
      terms={terms}
      semantic={semantic}
      showPane={showPane}
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
              <div className="sticky top-0 z-20 flex items-baseline gap-2 bg-zinc-100/95 px-4 py-1 backdrop-blur">
                <span className="truncate text-xs font-semibold text-zinc-600">{section.title}</span>
                {section.subtitle && <span className="text-xs text-zinc-400">{section.subtitle}</span>}
              </div>
              {section.subs
                ? section.subs.map((sub) => (
                    <div key={sub.id}>
                      <div className="sticky top-6 z-10 flex items-baseline gap-2 bg-zinc-50/95 px-4 py-0.5 pl-6 backdrop-blur">
                        <span className="truncate text-xs font-medium text-zinc-500">{sub.title}</span>
                        {sub.subtitle && <span className="text-xs text-zinc-300">{sub.subtitle}</span>}
                      </div>
                      {sub.rows.map(renderRow)}
                    </div>
                  ))
                : section.rows.map(renderRow)}
            </div>
          ))
        : rows.map(renderRow)}
    </div>
  );
}

function Row({
  row,
  density,
  terms,
  semantic,
  showPane,
  active,
  onActivate,
  onOpen,
}: {
  row: SearchResult;
  density: Density;
  terms: string[];
  semantic: boolean;
  showPane: boolean;
  active: boolean;
  onActivate: () => void;
  onOpen: () => void;
}) {
  const ref = useRef<HTMLButtonElement>(null);
  const colorDot = resolveColor(row.annotation_color);
  // In semantic mode, flag whether a query keyword also appears (keyword+semantic)
  // or the match is semantic-only.
  const hay = `${row.text} ${row.title} ${row.author ?? ""}`.toLowerCase();
  const keywordHit = terms.some((t) => hay.includes(t.toLowerCase()));

  useEffect(() => {
    if (active) ref.current?.scrollIntoView({ block: "nearest" });
  }, [active]);

  const date = formatDate(row.highlighted_at);
  const year =
    row.highlighted_at && /^\d{4}/.test(row.highlighted_at) ? row.highlighted_at.slice(0, 4) : "";
  const author = row.author ?? "";
  const imgPrefix = row.format === "image" ? "🖼 " : "";
  const bodyText = row.text || (row.format === "image" ? "[image annotation]" : "");

  const rowBase = `flex w-full border-b border-zinc-100 px-4 text-left ${
    active ? "bg-amber-50" : "hover:bg-zinc-50"
  }`;

  // Minimal: single truncated line with author + year aligned in right-hand columns.
  if (density === "minimal") {
    return (
      <button
        ref={ref}
        onClick={onActivate}
        onDoubleClick={onOpen}
        className={`${rowBase} items-center gap-2 py-1.5`}
      >
        <span className="shrink-0 text-sm opacity-70">{typeIcon(row)}</span>
        {colorDot && (
          <span className="h-2 w-2 shrink-0 rounded-full" style={{ backgroundColor: colorDot }} />
        )}
        <span className="min-w-0 flex-1 truncate text-sm text-zinc-800">
          {imgPrefix}
          {renderInlineMarkdown(compact(bodyText, 240), terms)}
        </span>
        {semantic && row.relevance != null && (
          <span className="shrink-0 text-[10px] font-medium text-violet-600">
            {Math.round(row.relevance * 100)}%
          </span>
        )}
        {!showPane && (
          <>
            <span className="w-40 shrink-0 truncate text-right text-xs text-zinc-500">{author}</span>
            <span className="w-12 shrink-0 text-right text-xs text-zinc-400">{year}</span>
          </>
        )}
      </button>
    );
  }

  // compact = first 2 lines, comfortable = first 4 lines, full = entire text.
  // NB: line-clamp sets display:-webkit-box, so it must NOT be combined with
  // `block` (which would override the display and defeat the clamp).
  const quoteClass =
    density === "compact"
      ? "line-clamp-2"
      : density === "comfortable"
        ? "line-clamp-4"
        : "block whitespace-pre-wrap";

  return (
    <button
      ref={ref}
      onClick={onActivate}
      onDoubleClick={onOpen}
      className={`${rowBase} items-start gap-2 py-2`}
    >
      <span className="mt-0.5 shrink-0 text-sm opacity-70">{typeIcon(row)}</span>
      {colorDot && (
        <span className="mt-1.5 h-2 w-2 shrink-0 rounded-full" style={{ backgroundColor: colorDot }} />
      )}
      <span className="min-w-0 flex-1">
        {semantic && (
          <span className="mb-0.5 flex items-center gap-1.5">
            {row.relevance != null && (
              <span className="rounded bg-violet-100 px-1 text-[10px] font-medium text-violet-700">
                {Math.round(row.relevance * 100)}%
              </span>
            )}
            <span
              className={`rounded px-1 text-[10px] font-medium ${
                keywordHit ? "bg-amber-100 text-amber-700" : "bg-zinc-100 text-zinc-500"
              }`}
            >
              {keywordHit ? "keyword + semantic" : "✦ semantic"}
            </span>
          </span>
        )}
        <span className={`text-sm text-zinc-800 ${quoteClass}`}>
          {imgPrefix}
          {density === "full"
            ? renderMarkdown(bodyText, terms)
            : renderInlineMarkdown(bodyText, terms)}
        </span>
        <span className="mt-1 flex flex-wrap items-center gap-1.5 text-xs text-zinc-400">
          {author && <span className="font-medium text-zinc-500">{author}</span>}
          {year && <span>· {year}</span>}
          {row.title && <span className="truncate">· {row.title}</span>}
          {density === "full" && date && <span>· {date}</span>}
          {density === "full" &&
            row.tags.slice(0, 4).map((t) => (
              <span key={t} className="rounded bg-blue-50 px-1 text-blue-600">{t}</span>
            ))}
        </span>
      </span>
    </button>
  );
}
