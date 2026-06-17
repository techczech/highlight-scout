import type { GroupMode, SearchResult, SortMode } from "../types";
import { authorLabel, uniqueTags } from "./format";

export interface Section {
  id: string;
  title?: string;
  subtitle?: string;
  rows: SearchResult[];
  subs?: Section[]; // present when a secondary grouping is applied
}

function groupKey(row: SearchResult, mode: GroupMode): string {
  if (mode === "work") return row.work_id || `t:${row.title}`;
  if (mode === "author") return authorLabel(row);
  return row.highlighted_at && row.highlighted_at.length >= 4
    ? row.highlighted_at.slice(0, 4)
    : "No date";
}

function titleFor(rows: SearchResult[], mode: GroupMode, key: string): string {
  if (mode === "work") return rows[0].title || "Untitled";
  if (mode === "author") return key; // already the full author label
  return key;
}

function subtitleFor(rows: SearchResult[], mode: GroupMode): string {
  if (mode === "work") return `${authorLabel(rows[0])} · ${rows.length}`;
  return String(rows.length);
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

/** One level of grouping. Always returns sections (used for both top and nested levels). */
function groupOneLevel(rows: SearchResult[], mode: GroupMode, sort: SortMode, idPrefix: string): Section[] {
  if (mode === "tag") return groupByTag(rows).map((s) => ({ ...s, id: `${idPrefix}${s.id}` }));

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
      id: `${idPrefix}date:${key}`,
      title: key,
      subtitle: String(map.get(key)!.length),
      rows: map.get(key)!,
    }));
  }

  // work / author: sort by size, keep all groups (nested level keeps singles too).
  order.sort((a, b) => {
    const diff = map.get(b)!.length - map.get(a)!.length;
    return diff !== 0 ? diff : order.indexOf(a) - order.indexOf(b);
  });
  return order.map((key) => {
    const group = map.get(key)!;
    return {
      id: `${idPrefix}${mode}:${key}`,
      title: titleFor(group, mode, key),
      subtitle: subtitleFor(group, mode),
      rows: group,
    };
  });
}

/**
 * Group rows for display, with an optional secondary (nested) grouping.
 * Returns null for flat display. At the top level, work/author collapse
 * non-repeating groups into "Other" (as before); the nested level keeps all.
 */
export function groupRows(
  rows: SearchResult[],
  mode: GroupMode,
  sub: GroupMode,
  sort: SortMode
): Section[] | null {
  if (mode === "none" || rows.length === 0) return null;

  let top: Section[];
  if (mode === "tag" || mode === "date") {
    top = groupOneLevel(rows, mode, sort, "");
  } else {
    // work/author: only break out repeats; rest → Other.
    const all = groupOneLevel(rows, mode, sort, "");
    const multi = all.filter((s) => s.rows.length >= 2);
    if (multi.length === 0 && sub === "none") return null;
    const singles = all.filter((s) => s.rows.length < 2).flatMap((s) => s.rows);
    top = multi;
    if (singles.length) {
      top.push({ id: "__other__", title: "Other", subtitle: String(singles.length), rows: singles });
    }
  }

  if (sub && sub !== "none") {
    top = top.map((section) => ({
      ...section,
      subs: groupOneLevel(section.rows, sub, sort, `${section.id}>`),
    }));
  }

  return top;
}

/** Flatten sections (including nested subs) into display order, for keyboard nav. */
export function flattenSections(sections: Section[] | null, rows: SearchResult[]): SearchResult[] {
  if (!sections) return rows;
  const out: SearchResult[] = [];
  for (const s of sections) {
    if (s.subs) for (const sub of s.subs) out.push(...sub.rows);
    else out.push(...s.rows);
  }
  return out;
}
