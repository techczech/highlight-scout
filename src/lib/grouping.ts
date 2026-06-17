import type { GroupMode, SearchResult, SortMode } from "../types";
import { uniqueTags } from "./format";

export interface Section {
  id: string;
  title?: string;
  subtitle?: string;
  rows: SearchResult[];
}

function groupKey(row: SearchResult, mode: GroupMode): string {
  if (mode === "work") return row.work_id || `t:${row.title}`;
  if (mode === "author") return row.author || "Unknown";
  return row.highlighted_at && row.highlighted_at.length >= 4
    ? row.highlighted_at.slice(0, 4)
    : "No date";
}

function groupByTag(rows: SearchResult[]): Section[] {
  const map = new Map<string, SearchResult[]>();
  const order: string[] = [];
  const untagged: SearchResult[] = [];
  for (const row of rows) {
    const rowTags = Array.from(new Set(uniqueTags(row).map((t) => t.toLowerCase())));
    if (!rowTags.length) {
      untagged.push(row);
      continue;
    }
    for (const tag of rowTags) {
      if (!map.has(tag)) {
        map.set(tag, []);
        order.push(tag);
      }
      map.get(tag)!.push(row);
    }
  }
  order.sort((a, b) => map.get(b)!.length - map.get(a)!.length || a.localeCompare(b));
  const sections: Section[] = order.map((tag) => ({
    id: `tag:${tag}`,
    title: tag,
    subtitle: String(map.get(tag)!.length),
    rows: map.get(tag)!,
  }));
  if (untagged.length) {
    sections.push({ id: "__untagged__", title: "Untagged", subtitle: String(untagged.length), rows: untagged });
  }
  return sections;
}

/** Group rows for display. Returns null for flat (ungrouped) display. */
export function groupRows(
  rows: SearchResult[],
  mode: GroupMode,
  sort: SortMode
): Section[] | null {
  if (mode === "none" || rows.length === 0) return null;
  if (mode === "tag") return groupByTag(rows);

  const map = new Map<string, SearchResult[]>();
  const order: string[] = [];
  for (const row of rows) {
    const key = groupKey(row, mode);
    if (!map.has(key)) {
      map.set(key, []);
      order.push(key);
    }
    map.get(key)!.push(row);
  }

  if (mode === "date") {
    const keys = [...order].sort((a, b) => {
      if (a === "No date") return 1;
      if (b === "No date") return -1;
      return sort === "oldest" ? a.localeCompare(b) : b.localeCompare(a);
    });
    return keys.map((key) => ({
      id: `date:${key}`,
      title: key,
      subtitle: String(map.get(key)!.length),
      rows: map.get(key)!,
    }));
  }

  // work / author: only break out groups that actually repeat.
  const multi = order.filter((key) => map.get(key)!.length >= 2);
  if (multi.length === 0) return null;
  multi.sort((a, b) => {
    const diff = map.get(b)!.length - map.get(a)!.length;
    return diff !== 0 ? diff : order.indexOf(a) - order.indexOf(b);
  });

  const sections: Section[] = multi.map((key) => {
    const group = map.get(key)!;
    const title = mode === "work" ? group[0].title || "Untitled" : key;
    const subtitle =
      mode === "work" ? `${group[0].author || "Unknown"} · ${group.length}` : String(group.length);
    return { id: `${mode}:${key}`, title, subtitle, rows: group };
  });

  const singles = order.filter((key) => map.get(key)!.length < 2).flatMap((key) => map.get(key)!);
  if (singles.length) {
    sections.push({ id: "__other__", title: "Other", subtitle: String(singles.length), rows: singles });
  }
  return sections;
}
