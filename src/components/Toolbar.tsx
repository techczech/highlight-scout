import type { Density, GroupMode, SortMode, SearchMode } from "../types";

const SORTS: Array<{ value: SortMode; label: string }> = [
  { value: "matches", label: "Most matches" },
  { value: "recent", label: "Most recent" },
  { value: "oldest", label: "Oldest" },
];

const GROUPS: Array<{ value: GroupMode; label: string }> = [
  { value: "work", label: "Work" },
  { value: "author", label: "Author" },
  { value: "date", label: "Date (year)" },
  { value: "tag", label: "Tag" },
  { value: "none", label: "None" },
];

const DENSITIES: Array<{ value: Density; label: string }> = [
  { value: "compact", label: "Compact" },
  { value: "comfortable", label: "Comfortable" },
  { value: "full", label: "Full quotes" },
];

const SUBGROUPS: Array<{ value: GroupMode; label: string }> = [
  { value: "none", label: "—" },
  { value: "work", label: "Work" },
  { value: "author", label: "Author" },
  { value: "date", label: "Date (year)" },
  { value: "tag", label: "Tag" },
];

interface Props {
  sort: SortMode;
  group: GroupMode;
  subgroup: GroupMode;
  mode: SearchMode;
  density: Density;
  showPane: boolean;
  onSort: (s: SortMode) => void;
  onGroup: (g: GroupMode) => void;
  onSubgroup: (g: GroupMode) => void;
  onMode: (m: SearchMode) => void;
  onDensity: (d: Density) => void;
  onTogglePane: () => void;
  onOpenTags: () => void;
  onOpenSettings: () => void;
  onImport: (which: "readwise" | "readwise-seed" | "zotero" | "qmd-reindex") => void;
  importing: boolean;
}

const selectClass =
  "rounded border border-zinc-200 bg-white px-1.5 py-0.5 text-xs text-zinc-600 outline-none hover:border-zinc-300";

export function Toolbar(props: Props) {
  return (
    <div className="flex items-center gap-2 border-b border-zinc-100 bg-zinc-50 px-3 py-1.5 text-xs overflow-x-auto">
      <div className="flex rounded bg-zinc-200 p-0.5">
        <button
          onClick={() => props.onMode("keyword")}
          className={`rounded px-2 py-0.5 ${props.mode === "keyword" ? "bg-white text-zinc-800 shadow-sm" : "text-zinc-500"}`}
        >
          Keyword
        </button>
        <button
          onClick={() => props.onMode("semantic")}
          title="Semantic search (QMD) — coming soon"
          className={`rounded px-2 py-0.5 ${props.mode === "semantic" ? "bg-white text-zinc-800 shadow-sm" : "text-zinc-400"}`}
        >
          Semantic
        </button>
      </div>

      <label className="flex items-center gap-1 text-zinc-400">
        Sort
        <select className={selectClass} value={props.sort} onChange={(e) => props.onSort(e.target.value as SortMode)}>
          {SORTS.map((s) => (
            <option key={s.value} value={s.value}>{s.label}</option>
          ))}
        </select>
      </label>

      <label className="flex items-center gap-1 text-zinc-400">
        Group
        <select className={selectClass} value={props.group} onChange={(e) => props.onGroup(e.target.value as GroupMode)}>
          {GROUPS.map((g) => (
            <option key={g.value} value={g.value}>{g.label}</option>
          ))}
        </select>
      </label>

      <label className="flex items-center gap-1 text-zinc-400" title="Secondary grouping within each group">
        then
        <select
          className={selectClass}
          value={props.subgroup}
          onChange={(e) => props.onSubgroup(e.target.value as GroupMode)}
          disabled={props.group === "none"}
        >
          {SUBGROUPS.map((g) => (
            <option key={g.value} value={g.value}>{g.label}</option>
          ))}
        </select>
      </label>

      <label className="flex items-center gap-1 text-zinc-400">
        Rows
        <select className={selectClass} value={props.density} onChange={(e) => props.onDensity(e.target.value as Density)}>
          {DENSITIES.map((d) => (
            <option key={d.value} value={d.value}>{d.label}</option>
          ))}
        </select>
      </label>

      <button onClick={props.onTogglePane} className="rounded px-2 py-0.5 text-zinc-500 hover:bg-zinc-200" title="Toggle reading pane (⌘⇧P)">
        {props.showPane ? "Hide pane" : "Show pane"}
      </button>
      <button onClick={props.onOpenTags} className="rounded px-2 py-0.5 text-zinc-500 hover:bg-zinc-200" title="Filter by tag (⌘⇧T)">
        Tags
      </button>

      <span className="ml-auto flex items-center gap-1 shrink-0">
        <select
          className={selectClass}
          disabled={props.importing}
          value=""
          onChange={(e) => {
            const v = e.target.value;
            if (v) props.onImport(v as "readwise" | "readwise-seed" | "zotero" | "qmd-reindex");
            e.target.value = "";
          }}
        >
          <option value="">{props.importing ? "Working…" : "Import ▾"}</option>
          <option value="readwise">Update from Readwise (API)</option>
          <option value="readwise-seed">Seed from Readwise archive</option>
          <option value="zotero">Import Zotero</option>
          <option value="qmd-reindex">Rebuild semantic index (QMD)</option>
        </select>
        <button onClick={props.onOpenSettings} className="rounded px-2 py-0.5 text-zinc-500 hover:bg-zinc-200" title="Settings (⌘,)">
          ⚙
        </button>
      </span>
    </div>
  );
}

export function ScopeDropdown(props: { value: string; onChange: (v: string) => void }) {
  return (
    <select
      value={props.value}
      onChange={(e) => props.onChange(e.target.value)}
      title="Filter by quick scope, time, or type"
      className="shrink-0 rounded border border-zinc-200 bg-white px-2 py-1 text-xs text-zinc-600 outline-none hover:border-zinc-300"
    >
      <option value="">All highlights</option>
      <optgroup label="Quick">
        <option value="fav">★ Favorites</option>
        <option value="zo">🔖 Zotero</option>
      </optgroup>
      <optgroup label="Time">
        <option value="t:30d">Last 30 days</option>
        <option value="t:6m">Last 6 months</option>
        <option value="t:12m">Last year</option>
      </optgroup>
      <optgroup label="Type">
        <option value="ty:articles">Articles</option>
        <option value="ty:books">Books</option>
        <option value="ty:tweets">Tweets</option>
        <option value="ty:pdfs">PDFs</option>
        <option value="ty:podcasts">Podcasts</option>
      </optgroup>
    </select>
  );
}
