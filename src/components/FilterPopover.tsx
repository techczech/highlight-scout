import { useEffect, useRef } from "react";
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

/** Combinable filter popover: Quick toggles + multi-select types stack; time is
 * a single recency window. Controlled open state so a keyboard command can open it. */
export function FilterPopover({ value, onChange, open, onOpenChange }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const count =
    (value.favorite ? 1 : 0) +
    (value.zotero ? 1 : 0) +
    (value.hasImage ? 1 : 0) +
    value.types.length +
    (value.time ? 1 : 0);

  useEffect(() => {
    if (!open) return;
    const onDoc = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onOpenChange(false);
    };
    document.addEventListener("mousedown", onDoc);
    return () => document.removeEventListener("mousedown", onDoc);
  }, [open, onOpenChange]);

  const toggleType = (t: string) => {
    const types = value.types.includes(t)
      ? value.types.filter((x) => x !== t)
      : [...value.types, t];
    onChange({ ...value, types });
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
        <div className="absolute right-0 z-30 mt-1 w-56 rounded border border-zinc-200 bg-white p-3 text-sm shadow-lg">
          <Section title="Quick">
            <Check label="★ Favorites" checked={value.favorite} onChange={(v) => onChange({ ...value, favorite: v })} />
            <Check label="🔖 Zotero" checked={value.zotero} onChange={(v) => onChange({ ...value, zotero: v })} />
            <Check label="🖼 Has image" checked={value.hasImage} onChange={(v) => onChange({ ...value, hasImage: v })} />
          </Section>
          <Section title="Type">
            {TYPES.map((t) => (
              <Check
                key={t.value}
                label={t.label}
                checked={value.types.includes(t.value)}
                onChange={() => toggleType(t.value)}
              />
            ))}
          </Section>
          <Section title="Time">
            {TIMES.map((t) => (
              <label key={t.value} className="flex cursor-pointer items-center gap-2 py-0.5 text-zinc-700">
                <input
                  type="radio"
                  name="filter-time"
                  checked={value.time === t.value}
                  onChange={() => onChange({ ...value, time: t.value })}
                />
                {t.label}
              </label>
            ))}
          </Section>
          <button
            onClick={() => onChange({ ...EMPTY_FILTERS })}
            className="mt-2 w-full rounded bg-zinc-100 py-1 text-xs text-zinc-600 hover:bg-zinc-200"
          >
            Clear all
          </button>
        </div>
      )}
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="mb-2">
      <p className="mb-1 text-[10px] font-semibold uppercase tracking-wide text-zinc-400">{title}</p>
      {children}
    </div>
  );
}

function Check({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <label className="flex cursor-pointer items-center gap-2 py-0.5 text-zinc-700">
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} />
      {label}
    </label>
  );
}
