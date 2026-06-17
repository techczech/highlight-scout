import type { SearchResult } from "../types";

export function compact(text: string, max = 320): string {
  const clean = (text || "").replace(/\s+/g, " ").trim();
  return clean.length <= max ? clean : `${clean.slice(0, max - 1)}…`;
}

export function formatDate(iso: string | null): string {
  if (!iso) return "";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso.slice(0, 10);
  return date.toLocaleDateString("en-GB", {
    day: "numeric",
    month: "short",
    year: "numeric",
  });
}

export function isZotero(row: SearchResult): boolean {
  return row.source_system === "zotero" || /zotero\.org/i.test(row.url || "");
}

/** Emoji icon for a work type (Zotero overrides to a bookmark). */
export function typeIcon(row: SearchResult): string {
  if (isZotero(row)) return "🔖";
  switch ((row.work_type || "").toLowerCase().replace(/s$/, "")) {
    case "book": return "📖";
    case "article": return "📄";
    case "tweet": return "🐦";
    case "podcast": return "🎙️";
    case "pdf": return "📑";
    case "thesis": return "🎓";
    case "report": return "📊";
    default: return "❝";
  }
}

export function uniqueTags(row: SearchResult): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const tag of row.tags || []) {
    const key = tag.trim().toLowerCase();
    if (!key || seen.has(key)) continue;
    seen.add(key);
    out.push(tag.trim());
  }
  return out;
}

export function originalUrl(row: SearchResult): string | undefined {
  return row.url || undefined;
}

export function shortUrl(url: string): string {
  let short = url;
  try {
    const u = new URL(url);
    short = `${u.hostname}${u.pathname}`.replace(/\/$/, "");
  } catch {
    /* keep raw */
  }
  short = short.replace(/^www\./, "");
  return short.length > 44 ? `${short.slice(0, 43)}…` : short;
}

export function markdownQuote(row: SearchResult): string {
  const quote = `> ${(row.text || "").trim().replace(/\n/g, "\n> ")}`;
  return `${quote}\n\n— ${row.author || "Unknown"}, ${row.title}`;
}

export function workMarkdownPath(archiveRoot: string, slug: string): string {
  return `${archiveRoot.replace(/\/$/, "")}/readings/works/${slug}.md`;
}

/**
 * Split text into segments, marking matched terms. Used to bold matches in the
 * reading pane without dangerouslySetInnerHTML.
 */
export function emphasizeSegments(
  text: string,
  terms: string[]
): Array<{ text: string; match: boolean }> {
  const clean = terms.map((t) => t.trim()).filter((t) => t.length >= 2);
  if (!clean.length) return [{ text, match: false }];
  const escaped = clean
    .sort((a, b) => b.length - a.length)
    .map((t) => t.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
  const re = new RegExp(`(${escaped.join("|")})`, "gi");
  const lower = new Set(clean.map((t) => t.toLowerCase()));
  // split with a capture group keeps the matched delimiters as their own entries
  return text
    .split(re)
    .filter((p) => p !== "")
    .map((p) => ({ text: p, match: lower.has(p.toLowerCase()) }));
}
