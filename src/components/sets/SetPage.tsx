// A set as its own page (HS-2A): ordered items with notes (left), export
// (right, 30%), and "More like these…" under the seeds. The page keeps its
// own copy of the items, applies each edit to it at once and sends the same
// edit to the set file (lib/sets.ts, src-tauri/src/sets.rs). It owns its
// keyboard: ↑↓ select, ⌥↑ ⌥↓ move, ⏎ opens the source, N edits the note,
// ⌫ removes the item from the set, Esc goes back to the search; in the
// suggestions Y adds, N rejects, Esc stops.
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import { copyText } from "../../lib/clipboard";
import type { CitedPassage } from "../../lib/archive";
import {
  acrossLine,
  applyLocal,
  errorText,
  mixLine,
  moveTarget,
  seedRange,
  selectionAfterRemove,
  setAdd,
  setDismiss,
  setEdit as saveEdit,
  setExport,
  setExportTo,
  setOpen,
  setRefresh,
  setSuggest,
  type ExportKind,
  type OpenedSet,
  type SetEdit,
  type Suggestion,
  type SuggestionState,
} from "../../lib/sets";
import { Icon } from "../quickfinder/icons";
import { ExportPanel, ItemRow, SeedRow, SuggestionCard } from "./SetParts";
import "./sets.css";

export interface SetPageProps {
  id: string;
  onBack: () => void;
  /** Something about the set changed (title, size): the rail and tray reload. */
  onChanged: () => void;
  onToast: (msg: string, ms?: number) => void;
}

function openSource(c: CitedPassage | null, fallback: string, toast: (m: string) => void) {
  if (!c) { toast("The source changed: there is nothing to open for this item"); return; }
  if (c.public_url) openUrl(c.public_url).catch(() => toast("Could not open the link"));
  else if (c.link?.startsWith("writeflex://")) openUrl(c.link).catch(() => toast("Could not open WriteFlex"));
  else openPath(c.path || fallback).catch(() => toast("Could not open the file"));
}

export function SetPage({ id, onBack, onChanged, onToast }: SetPageProps) {
  const [set, setSet] = useState<OpenedSet | null>(null);
  const [error, setError] = useState("");
  const [sel, setSel] = useState(0);
  const [editing, setEditing] = useState<number | null>(null);
  const [dragFrom, setDragFrom] = useState<number | null>(null);
  const [dragOver, setDragOver] = useState<number | null>(null);
  const [renaming, setRenaming] = useState(false);
  const [busy, setBusy] = useState<ExportKind | null>(null);
  // "More like these…"
  const [suggesting, setSuggesting] = useState(false);
  const [sugg, setSugg] = useState<Suggestion[] | null>(null);
  const [suggErr, setSuggErr] = useState("");
  const [suggLoading, setSuggLoading] = useState(false);
  const [states, setStates] = useState<Record<string, { state: SuggestionState; as: number | null }>>({});
  const [suggSel, setSuggSel] = useState(0);
  const seedCount = useRef(0);

  const load = useCallback(async () => {
    try {
      const s = await setOpen(id);
      setSet(s);
      setError("");
      return s;
    } catch (e) {
      setError(errorText(e));
      return null;
    }
  }, [id]);

  useEffect(() => { setSet(null); load(); }, [load]);

  const items = set?.items ?? [];
  const saved = set?.kind === "saved-search";

  /** Apply an edit to the page's copy at once and to the set file after it. */
  const edit = useCallback((e: SetEdit) => {
    setSet((s) => (s ? { ...s, items: applyLocal(s.items, e), title: e.op === "rename" ? e.title.trim() || s.title : s.title } : s));
    saveEdit(id, e).then(onChanged).catch((err) => { onToast(`Could not save the set: ${errorText(err)}`, 4000); load(); });
  }, [id, load, onChanged, onToast]);

  const move = (from: number, to: number) => {
    if (from === to) return;
    edit({ op: "move", from, to });
    setSel(to);
  };

  const remove = (i: number) => {
    edit({ op: "remove", index: i });
    setSel(selectionAfterRemove(i, items.length));
    onToast(`Removed from ${set?.title ?? "the set"}`);
  };

  const startSuggest = async () => {
    if (!items.length) return;
    setSuggesting(true);
    setSuggLoading(true);
    setSuggErr("");
    setSugg(null);
    setStates({});
    setSuggSel(0);
    seedCount.current = items.length;
    try {
      setSugg(await setSuggest(id));
    } catch (e) {
      setSuggErr(errorText(e));
    } finally {
      setSuggLoading(false);
    }
  };

  const add = async (s: Suggestion) => {
    try {
      await setAdd(id, s.passage_id);
      const n = (set?.items.length ?? 0) + 1;
      setStates((m) => ({ ...m, [s.passage_id]: { state: "added", as: n } }));
      await load();
      onChanged();
    } catch (e) {
      onToast(`Could not add: ${errorText(e)}`, 4000);
    }
  };

  const reject = async (s: Suggestion) => {
    setStates((m) => ({ ...m, [s.passage_id]: { state: "rejected", as: null } }));
    try { await setDismiss(id, s.passage_id); } catch (e) { onToast(`Could not remember that: ${errorText(e)}`, 4000); }
  };

  const doExport = async (kind: ExportKind) => {
    setBusy(kind);
    try {
      const out = await setExport(id, kind);
      const path = await saveDialog({ defaultPath: out.file_name, filters: [{ name: "Markdown", extensions: ["md"] }] });
      if (typeof path !== "string") return;
      await setExportTo(id, kind, path);
      onToast(`Saved ${path.split("/").pop()}`, 3000);
    } catch (e) {
      onToast(`Export failed: ${errorText(e)}`, 4000);
    } finally {
      setBusy(null);
    }
  };

  const doCopy = async (kind: ExportKind) => {
    setBusy(kind);
    try {
      await copyText((await setExport(id, kind)).text);
      onToast("Copied");
    } catch (e) {
      onToast(`Export failed: ${errorText(e)}`, 4000);
    } finally {
      setBusy(null);
    }
  };

  const refresh = async () => {
    try {
      const s = await setRefresh(id);
      setSet(s);
      onChanged();
      onToast("Matches updated");
    } catch (e) {
      onToast(`Could not update: ${errorText(e)}`, 4000);
    }
  };

  // The page's keyboard, on the window (the app's own handler stands down while a set is open).
  const keyRef = useRef<(e: KeyboardEvent) => void>(() => {});
  keyRef.current = (e: KeyboardEvent) => {
    const t = e.target as HTMLElement | null;
    const typing = !!t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable);
    if (e.key === "Escape") {
      if (typing) return; // the field's own Esc handler
      if (suggesting) setSuggesting(false);
      else onBack();
      return;
    }
    if (typing || e.metaKey || e.ctrlKey) return;
    if (suggesting) {
      const cur = sugg?.[suggSel];
      const isOpen = !!cur && (states[cur.passage_id]?.state ?? "open") === "open";
      const advance = () => setSuggSel((i) => Math.min((sugg?.length ?? 1) - 1, i + 1));
      if (e.key === "ArrowDown") { e.preventDefault(); advance(); }
      else if (e.key === "ArrowUp") { e.preventDefault(); setSuggSel((i) => Math.max(0, i - 1)); }
      else if ((e.key === "y" || e.key === "Y") && cur && isOpen) { e.preventDefault(); add(cur).then(advance); }
      else if ((e.key === "n" || e.key === "N") && cur && isOpen) { e.preventDefault(); reject(cur).then(advance); }
      return;
    }
    if (!items.length) return;
    if (e.altKey && (e.key === "ArrowUp" || e.key === "ArrowDown")) {
      e.preventDefault();
      const to = moveTarget(sel, e.key === "ArrowUp" ? -1 : 1, items.length);
      if (to !== null) move(sel, to);
    } else if (e.key === "ArrowDown") { e.preventDefault(); setSel((i) => Math.min(items.length - 1, i + 1)); }
    else if (e.key === "ArrowUp") { e.preventDefault(); setSel((i) => Math.max(0, i - 1)); }
    else if (e.key === "Enter") { e.preventDefault(); openSource(items[sel]?.cited ?? null, items[sel]?.item.source_path ?? "", onToast); }
    else if (e.key === "n" || e.key === "N") { e.preventDefault(); setEditing(sel); }
    else if (e.key === "Backspace" || e.key === "Delete") { e.preventDefault(); remove(sel); }
  };
  useEffect(() => {
    const h = (e: KeyboardEvent) => keyRef.current(e);
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, []);

  const across = useMemo(() => acrossLine(sugg ?? []), [sugg]);
  const seeds = items.slice(0, seedCount.current || 10);

  if (error) {
    return (
      <div className="qf sp" data-testid="set-page">
        <div className="sp-head"><button className="back" onClick={onBack}>← Search</button></div>
        <div className="qf-empty"><p className="warn">{error}</p></div>
      </div>
    );
  }

  return (
    <div className="qf sp" data-testid="set-page">
      <div className="sp-main">
        <div className="sp-left">
          <div className="sp-head">
            <button className="back" data-action="back" onClick={onBack} title="Back to the search (Esc)">← Search</button>
            {renaming ? (
              <input
                className="title-edit"
                autoFocus
                defaultValue={set?.title}
                aria-label="Set title"
                onKeyDown={(e) => {
                  e.stopPropagation();
                  if (e.key === "Enter") { edit({ op: "rename", title: e.currentTarget.value }); setRenaming(false); }
                  else if (e.key === "Escape") setRenaming(false);
                }}
                onBlur={(e) => { if (e.currentTarget.value.trim() && e.currentTarget.value.trim() !== set?.title) edit({ op: "rename", title: e.currentTarget.value }); setRenaming(false); }}
              />
            ) : (
              <h1 title="Click to rename" onClick={() => setRenaming(true)}>{set?.title ?? "…"}</h1>
            )}
            <span className="mix">{suggesting ? `${items.length} items` : mixLine(items.map((r) => r.item))}</span>
            <span className="sp-gap" />
            {saved && !suggesting && <button className="tool" data-action="refresh" onClick={refresh} title={`Follows “${set?.query}”: pinned items stay, the others follow the search`}>Update matches</button>}
            {!suggesting && <button className="tool" data-action="more-like" disabled={!items.length} onClick={startSuggest}><Icon name="spark" size="sm" />More like these…</button>}
            <button className="tool" data-action="export-top" onClick={() => document.querySelector<HTMLButtonElement>('[data-export="writeflex"]')?.focus()}>Export ⌄</button>
          </div>

          {suggesting ? (
            <div className="sp-scroll" data-testid="suggestions">
              <div className="sp-seedhead"><span>Seeds</span><span className="r">items 1–{seeds.length} of {items.length}</span></div>
              {seeds.map((r, i) => <SeedRow key={r.item.passage} index={i} row={r} />)}
              <div className="sp-morehead"><Icon name="spark" size="sm" />More like {seedRange(seeds.length)}{across ? ` · ${across}` : ""}<span className="r">each joins only when you add it</span></div>
              {suggLoading && <div className="qf-empty"><p>Looking for passages like these…</p></div>}
              {suggErr && <div className="qf-empty"><p className="warn">{suggErr}</p></div>}
              {sugg && sugg.length === 0 && <div className="qf-empty"><p>Nothing else in the archive shares enough words with these items.</p></div>}
              {(sugg ?? []).map((s, i) => (
                <SuggestionCard
                  key={s.passage_id}
                  s={s}
                  state={states[s.passage_id]?.state ?? "open"}
                  addedAs={states[s.passage_id]?.as ?? null}
                  selected={i === suggSel}
                  onSelect={() => setSuggSel(i)}
                  onAdd={() => add(s)}
                  onReject={() => reject(s)}
                />
              ))}
            </div>
          ) : (
            <div className="sp-scroll" role="listbox" aria-label="Items" data-testid="set-items">
              {set && items.length === 0 && (
                <div className="qf-empty">
                  <p>This set is empty.</p>
                  <p className="hint">{saved ? "Update matches to fill it from the search." : "Select a result and press ⌘S (or + Set) to add it."}</p>
                </div>
              )}
              {items.map((r, i) => (
                <ItemRow
                  key={r.item.passage}
                  index={i}
                  row={r}
                  selected={i === sel}
                  editing={editing === i}
                  pinnable={saved}
                  dragOver={dragOver === i && dragFrom !== null && dragFrom !== i}
                  onSelect={() => setSel(i)}
                  onNote={() => { setSel(i); setEditing(i); }}
                  onNoteSave={(note) => { if (editing === i) { setEditing(null); if (note.trim() !== r.item.note) edit({ op: "note", index: i, note }); } }}
                  onNoteCancel={() => setEditing(null)}
                  onPin={() => edit({ op: "pin", index: i, pinned: !r.item.pinned })}
                  onDragStart={() => setDragFrom(i)}
                  onDragOver={() => setDragOver(i)}
                  onDrop={() => { if (dragFrom !== null) move(dragFrom, i); setDragFrom(null); setDragOver(null); }}
                  onOpen={() => openSource(r.cited, r.item.source_path, onToast)}
                />
              ))}
            </div>
          )}
        </div>
        <ExportPanel disabled={!items.length} busy={busy} onExport={doExport} onCopy={doCopy} />
      </div>
      <div className="sp-foot">
        {suggesting ? (
          <><span>Y adds · N rejects · the underlined words are what made it similar</span><span className="r"><kbd>Y</kbd> add <kbd>N</kbd> not this <kbd>esc</kbd> stop</span></>
        ) : (
          <><span>Drag or ⌥↑ ⌥↓ to reorder · ⏎ opens the source · N edits the note</span><span className="r"><kbd>⌥↑</kbd><kbd>⌥↓</kbd> move <kbd>N</kbd> note <kbd>⌫</kbd> remove <kbd>esc</kbd> back to search</span></>
        )}
      </div>
    </div>
  );
}
