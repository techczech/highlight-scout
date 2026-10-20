// Sets (HS-2A): types, the Tauri calls, and the pure helpers the set page
// and the tray use. A set is a file in the archive (`sets/<id>.md`); the
// backend owns it (src-tauri/src/sets.rs). The page edits its own copy of
// the items at once and sends the same edit to the backend.
import { invoke } from "@tauri-apps/api/core";
import type { CitedPassage } from "./archive";

export type SetKind = "hand" | "saved-search";
export type ItemStatus = "ok" | "repaired" | "changed";

export interface SetSummary {
  id: string;
  title: string;
  kind: SetKind;
  items: number;
  updated: string;
}

export interface SetItem {
  passage: string;
  corpus: string;
  source_path: string;
  quote: string;
  pinned: boolean;
  note: string;
  /** What the user wrote in the item before its quote, kept in the file. */
  lead?: string;
  /** What the user wrote in the item after its quote, kept in the file. */
  extra?: string;
}

export interface ResolvedItem {
  item: SetItem;
  status: ItemStatus;
  cited: CitedPassage | null;
  /** A saved search's item the user unpinned: it leaves at the next update (unless it carries a note). */
  kept_out?: boolean;
}

export interface OpenedSet {
  id: string;
  title: string;
  kind: SetKind;
  query: string | null;
  items: ResolvedItem[];
}

export interface Suggestion {
  passage_id: string;
  corpus: string;
  rel_path: string;
  quote: string;
  title: string;
  author: string | null;
  date_display: string | null;
  shared_terms: string[];
  similar_to: string;
  score: number;
}

/**
 * An edit names its item by passage id, never by position alone: the file
 * may have changed since the page showed it (a saved search refreshed in the
 * background). A move also names where the page showed both ends; the
 * backend refuses an edit whose item is not where the page showed it, and
 * the page reloads.
 */
export type SetEdit =
  | { op: "move"; passage: string; from: number; to: number; to_passage: string }
  | { op: "note"; passage: string; note: string }
  | { op: "remove"; passage: string }
  | { op: "pin"; passage: string; pinned: boolean }
  | { op: "rename"; title: string };

/** The move of the item shown at `from` to where `to` was shown. */
export function moveEdit(items: ResolvedItem[], from: number, to: number): SetEdit | null {
  const a = items[from]?.item.passage;
  const b = items[to]?.item.passage;
  return a && b && from !== to ? { op: "move", passage: a, from, to, to_passage: b } : null;
}

export type ExportKind = "writeflex" | "talkweaver" | "citations";

export const EXPORTS: { kind: ExportKind; title: string; text: string; ext: string }[] = [
  { kind: "writeflex", title: "WriteFlex piece", text: "A Markdown piece: each quote with its citation line, notes as the paragraphs between.", ext: "md" },
  { kind: "talkweaver", title: "TalkWeaver quote slides", text: "One quote slide per item, in this order, with the source line on each slide.", ext: "md" },
  { kind: "citations", title: "Citation list", text: "One line per item, in this order, each with its link.", ext: "md" },
];

export const setsList = () => invoke<SetSummary[]>("sets_list");
export const setCreate = (title: string, query?: string) => invoke<SetSummary>("set_create", { title, query: query ?? null });
export const setOpen = (id: string) => invoke<OpenedSet>("set_open", { id });
export const setAdd = (id: string, passageId: string) => invoke<SetSummary>("set_add", { id, passageId });
/** A highlight from the classic list, found by its text within the selected work (its slug). */
export const setAddText = (id: string, corpus: string, text: string, workSlug: string) =>
  invoke<SetSummary>("set_add_text", { id, corpus, text, workSlug });
export const setEdit = (id: string, edit: SetEdit) => invoke<SetSummary>("set_edit", { id, edit });
export const setSuggest = (id: string) => invoke<Suggestion[]>("set_suggest", { id });
export const setDismiss = (id: string, passageId: string) => invoke<void>("set_dismiss", { id, passageId });
export const setRefresh = (id: string) => invoke<OpenedSet>("set_refresh", { id });
export const setExport = (id: string, kind: ExportKind) => invoke<{ file_name: string; text: string }>("set_export", { id, kind });
/** Save an export: the backend opens the save dialog and writes only where the user chose; null when cancelled. */
export const setExportSave = (id: string, kind: ExportKind) => invoke<string | null>("set_export_save", { id, kind });
/** Every saved-search set takes in its new matches (after an index build). */
export const setsRefreshSaved = () => invoke<string[]>("sets_refresh_saved");
export const archivescoutInstalled = () => invoke<boolean>("archivescout_installed");

/** Tauri rejects with a string (or an object with a message). */
export function errorText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object" && "message" in e) return String((e as { message: unknown }).message);
  return String(e);
}

// ---------- pure helpers ----------

/** "Metaphor talk · 7 items": the tray. */
export function trayLabel(title: string, count: number): string {
  return `${title} · ${count} item${count === 1 ? "" : "s"}`;
}

const MIX_ORDER: Array<[string, string, string]> = [["writing", "writing", "writing"], ["tweets", "tweet", "tweets"], ["highlights", "highlight", "highlights"]];

/** "7 items · 3 writing · 1 tweet · 3 highlights": under the set's title. */
export function mixLine(items: { corpus: string }[]): string {
  const n = items.length;
  const parts = [`${n} item${n === 1 ? "" : "s"}`];
  for (const [c, one, many] of MIX_ORDER) {
    const k = items.filter((i) => i.corpus === c).length;
    if (k) parts.push(c === "writing" ? `${k} writing` : `${k} ${k === 1 ? one : many}`);
  }
  return parts.join(" · ");
}

/** Where ⌥↑ / ⌥↓ moves the item at `index`; null at an end. */
export function moveTarget(index: number, delta: -1 | 1, length: number): number | null {
  const to = index + delta;
  return to < 0 || to >= length ? null : to;
}

/** The same edit the backend applies, for the page's own copy of the items. */
export function applyLocal(items: ResolvedItem[], e: SetEdit): ResolvedItem[] {
  const out = [...items];
  const at = (p: string) => out.findIndex((r) => r.item.passage === p);
  switch (e.op) {
    case "move": {
      if (out[e.from]?.item.passage !== e.passage || out[e.to]?.item.passage !== e.to_passage) return out;
      const [x] = out.splice(e.from, 1);
      out.splice(e.to, 0, x);
      return out;
    }
    case "note": {
      const i = at(e.passage);
      if (i >= 0) out[i] = { ...out[i], item: { ...out[i].item, note: e.note.trim().replace(/\n/g, " ") } };
      return out;
    }
    case "pin": {
      const i = at(e.passage);
      if (i >= 0) {
        const r = out[i];
        out[i] = { ...r, item: { ...r.item, pinned: e.pinned }, kept_out: e.pinned ? false : r.item.pinned || !!r.kept_out };
      }
      return out;
    }
    case "remove": {
      const i = at(e.passage);
      if (i >= 0) out.splice(i, 1);
      return out;
    }
    default: return out;
  }
}

/** The index the selection lands on after removing `index` from `length` items. */
export function selectionAfterRemove(index: number, length: number): number {
  return Math.max(0, Math.min(index, length - 2));
}

export type SuggestionState = "open" | "added" | "rejected";

/** "More like items 1–3": which items seeded the suggestions (the engine takes ten). */
export function seedRange(itemCount: number, max = 10): string {
  const n = Math.min(itemCount, max);
  return n <= 1 ? "item 1" : `items 1–${n}`;
}

/** "ACROSS WRITING AND HIGHLIGHTS": the corpora the suggestions come from. */
export function acrossLine(suggestions: { corpus: string }[]): string {
  const names: Record<string, string> = { writing: "writing", tweets: "tweets", highlights: "highlights" };
  const seen = MIX_ORDER.map(([c]) => c).filter((c) => suggestions.some((s) => s.corpus === c)).map((c) => names[c]);
  if (!seen.length) return "";
  return `across ${seen.length === 1 ? seen[0] : `${seen.slice(0, -1).join(", ")} and ${seen[seen.length - 1]}`}`;
}

/** The set "+ Set" adds to: the remembered one if it still exists. */
export function currentSet(sets: SetSummary[], remembered: string | null): SetSummary | null {
  return sets.find((s) => s.id === remembered) ?? null;
}

export const CURRENT_SET_KEY = "hs.currentSet";

export function loadCurrentSet(store: Pick<Storage, "getItem">): string | null {
  try { return store.getItem(CURRENT_SET_KEY); } catch { return null; }
}

export function saveCurrentSet(id: string | null, store: Pick<Storage, "setItem" | "removeItem">): void {
  try { id ? store.setItem(CURRENT_SET_KEY, id) : store.removeItem(CURRENT_SET_KEY); } catch { /* a convenience only */ }
}

/** "Added to Metaphor talk · item 8" */
export function addedLine(title: string, count: number, already: boolean): string {
  return already ? `Already in ${title}` : `Added to ${title} · item ${count}`;
}

// ---------- ordering the page's changes ----------

/** A change that was not sent because an earlier one failed (the page reloads the set). */
export class SkippedChange extends Error {
  constructor() { super("an earlier change to the set failed"); }
}

/**
 * One queue per set page: each change to the set file starts only after the
 * previous one finished, in the order the user made them. After a failure the
 * changes still waiting are skipped (they were made on a copy that no longer
 * matches the file); the page reloads instead.
 */
export function changeQueue() {
  let tail: Promise<unknown> = Promise.resolve();
  let epoch = 0;
  return function run<T>(send: () => Promise<T>): Promise<T> {
    const mine = epoch;
    const p = tail.then(() => {
      if (mine !== epoch) throw new SkippedChange();
      return send();
    });
    tail = p.then(
      () => undefined,
      (err) => { if (!(err instanceof SkippedChange)) epoch++; },
    );
    return p;
  };
}

/** The index-keeper's progress event, as far as saved searches care. */
export interface IndexJobLike { phase: string; builds: { error?: string | null }[] }

/**
 * Saved-search sets follow their search: whenever the index keeper reports a
 * finished build (which follows every sync), `refresh` runs. Returns the
 * unsubscribe promise from `listen`.
 */
export function followIndexBuilds(
  listen: (event: string, cb: (e: { payload: IndexJobLike }) => void) => Promise<() => void>,
  refresh: () => void,
): Promise<() => void> {
  return listen("corpus:index", (e) => {
    const j = e.payload;
    if (j.phase === "built" || (j.phase === "failed" && j.builds.some((b) => !b.error))) refresh();
  });
}
