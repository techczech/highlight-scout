import { useEffect, useMemo, useState } from "react";
import { listTags } from "../lib/api";
import type { TagCount } from "../types";

interface Props {
  onPick: (tag: string) => void;
  onClose: () => void;
}

export function TagPicker({ onPick, onClose }: Props) {
  const [tags, setTags] = useState<TagCount[]>([]);
  const [filter, setFilter] = useState("");

  useEffect(() => {
    listTags().then(setTags).catch(() => {});
  }, []);

  const shown = useMemo(
    () => tags.filter((t) => t.tag.includes(filter.toLowerCase())),
    [tags, filter]
  );

  return (
    <Overlay title="Filter by tag" onClose={onClose}>
      <input
        autoFocus
        value={filter}
        onChange={(e) => setFilter(e.target.value)}
        placeholder={`Search ${tags.length} tags…`}
        className="mb-2 w-full rounded border border-zinc-200 px-3 py-2 text-sm outline-none focus:border-amber-400"
        onKeyDown={(e) => {
          if (e.key === "Enter" && shown[0]) onPick(shown[0].tag);
          if (e.key === "Escape") onClose();
        }}
      />
      <div className="flex-1 overflow-y-auto">
        {shown.map((t) => (
          <button
            key={t.tag}
            onClick={() => onPick(t.tag)}
            className="flex w-full items-center justify-between rounded px-3 py-1.5 text-left text-sm hover:bg-zinc-100"
          >
            <span className="text-zinc-700">🏷 {t.tag}</span>
            <span className="text-xs text-zinc-400">{t.count}</span>
          </button>
        ))}
        {shown.length === 0 && <p className="p-3 text-sm text-zinc-400">No tags</p>}
      </div>
    </Overlay>
  );
}

export function Overlay({
  title,
  onClose,
  children,
  wide,
}: {
  title: string;
  onClose: () => void;
  children: React.ReactNode;
  wide?: boolean;
}) {
  return (
    <div className="absolute inset-0 z-50 flex items-start justify-center bg-black/20 p-8" onClick={onClose}>
      <div
        className={`flex max-h-full w-full ${wide ? "max-w-3xl" : "max-w-md"} flex-col rounded-lg border border-zinc-200 bg-white p-4 shadow-xl`}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="mb-2 flex items-center justify-between">
          <h2 className="text-sm font-semibold text-zinc-700">{title}</h2>
          <button onClick={onClose} className="text-lg leading-none text-zinc-400 hover:text-zinc-600">×</button>
        </div>
        {children}
      </div>
    </div>
  );
}
