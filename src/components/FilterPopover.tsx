import { Fragment, useEffect, useMemo, useRef, useState } from "react";
import { EMPTY_FILTERS, type Filters, filtersActive } from "../lib/query";

interface Props {
  value: Filters;
  onChange: (f: Filters) => void;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

const TYPES: Array<{ value: string; label: string }> = [
  { value: "articles", label: "Articles" },
  { value: "books", label: "Books" },
  { value: "tweets", label: "Tweets" },
  { value: "pdfs", label: "PDFs" },
  { value: "podcasts", label: "Podcasts" },
];

const TIMES: Array<{ value: string; label: string }> = [
  { value: "", label: "Any" },
  { value: "t:30d", label: "30 days" },
  { value: "t:6m", label: "6 months" },
  { value: "t:12m", label: "Year" },
];

interface Item {
  key: string;
  label: string;
  kind: "check" | "radio" | "action";
  checked: boolean;
  section?: string;
  run: () => void;
}

/** Combinable filter popover, fully keyboard-navigable once open: ↑/↓ move
 * between options, Space/Enter toggles, Esc closes (handled by the app). Quick
 * toggles + multi-select types stack; time is a single recency window. */
export function FilterPopover({ value, onChange, open, onOpenChange }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const itemRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const [active, setActive] = useState(0);

  const count =
    (value.favorite ? 1 : 0) +
    (value.zotero ? 1 : 0) +
    (value.hasImage ? 1 : 0) +
    value.types.length +
    (value.time ? 1 : 0);

  const items = useMemo<Item[]>(() => {
    const set = (patch: Partial<Filters>) => onChange({ ...value, ...patch });
    const toggleType = (t: string) =>
      set({
        types: value.types.includes(t)
          ? value.types.filter((x) => x !== t)
          : [...value.types, t],
      });
    const list: Item[] = [
      { key: "fav", label: "★ Favorites", kind: "check", checked: value.favorite, section: "Quick", run: () => set({ favorite: !value.favorite }) },
      { key: "zo", label: "🔖 Zotero", kind: "check", checked: value.zotero, run: () => set({ zotero: !value.zotero }) },
      { key: "img", label: "🖼 Has image", kind: "check", checked: value.hasImage, run: () => set({ hasImage: !value.hasImage }) },
    ];
    TYPES.forEach((t, i) =>
      list.push({ key: `ty-${t.value}`, label: t.label, kind: "check", checked: value.types.includes(t.value), section: i === 0 ? "Type" : undefined, run: () => toggleType(t.value) }),
    );
    TIMES.forEach((t, i) =>
      list.push({ key: `tm-${t.value}`, label: t.label, kind: "radio", checked: value.time === t.value, section: i === 0 ? "Time" : undefined, run: () => set({ time: t.value }) }),
    );
    list.push({ key: "clear", label: "Clear all", kind: "action", checked: false, run: () => onChange({ ...EMPTY_FILTERS }) });
    return list;
  }, [value, onChange]);

  // Close on outside click.
  useEffect(() => {
    if (!open) return;
    const onDoc = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onOpenChange(false);
    };
    document.addEventListener("mousedown", onDoc);
    return () => document.removeEventListener("mousedown", onDoc);
  }, [open, onOpenChange]);

  // On open, start at the top; keep focus on the active row as it changes.
  useEffect(() => { if (open) setActive(0); }, [open]);
  useEffect(() => { if (open) itemRefs.current[active]?.focus(); }, [open, active]);

  // Arrow keys move; Space/Enter toggles. Stop these from reaching the global
  // shortcut handler (which would otherwise move the results list / open source).
  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") { e.preventDefault(); e.stopPropagation(); setActive((a) => Math.min(a + 1, items.length - 1)); }
    else if (e.key === "ArrowUp") { e.preventDefault(); e.stopPropagation(); setActive((a) => Math.max(a - 1, 0)); }
    else if (e.key === "Home") { e.preventDefault(); e.stopPropagation(); setActive(0); }
    else if (e.key === "End") { e.preventDefault(); e.stopPropagation(); setActive(items.length - 1); }
    else if (e.key === " " || e.key === "Enter") { e.preventDefault(); e.stopPropagation(); items[active]?.run(); }
  };

  const indicator = (it: Item) => {
    if (it.kind === "action") return "";
    if (it.kind === "radio") return it.checked ? "◉ " : "○ ";
    return it.checked ? "☑ " : "☐ ";
  };

  return (
    <div ref={ref} className="relative shrink-0">
      <button
        onClick={() => onOpenChange(!open)}
        title="Filter results (⌘⇧I)"
        className={`rounded border px-2 py-1 text-xs outline-none ${
          filtersActive(value)
            ? "border-amber-400 bg-amber-50 text-amber-700"
            : "border-zinc-200 bg-white text-zinc-600 hover:border-zinc-300"
        }`}
      >
        ⚲ Filters{count > 0 ? ` (${count})` : ""}
      </button>
      {open && (
        <div
          role="menu"
          aria-label="Filters"
          onKeyDown={onKeyDown}
          className="absolute right-0 z-30 mt-1 w-56 rounded border border-zinc-200 bg-white p-2 text-sm shadow-lg"
        >
          {items.map((it, i) => (
            <Fragment key={it.key}>
              {it.section && (
                <p className="mb-0.5 mt-1.5 px-2 text-[10px] font-semibold uppercase tracking-wide text-zinc-400">
                  {it.section}
                </p>
              )}
              {it.kind === "action" && <div className="my-1 border-t border-zinc-100" />}
              <button
                ref={(el) => { itemRefs.current[i] = el; }}
                role={it.kind === "radio" ? "menuitemradio" : it.kind === "check" ? "menuitemcheckbox" : "menuitem"}
                aria-checked={it.kind === "action" ? undefined : it.checked}
                tabIndex={i === active ? 0 : -1}
                onClick={it.run}
                onMouseEnter={() => setActive(i)}
                className={`block w-full rounded px-2 py-1 text-left outline-none ${
                  i === active ? "bg-amber-50" : "hover:bg-zinc-50"
                } ${it.kind === "action" ? "text-center text-xs text-zinc-500" : "text-zinc-700"}`}
              >
                <span className="text-zinc-400">{indicator(it)}</span>
                {it.label}
              </button>
            </Fragment>
          ))}
        </div>
      )}
    </div>
  );
}
