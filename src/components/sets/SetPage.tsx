// A set as its own page (HS-2A): ordered items with notes (left), export
// (right, 30%), and "More like these…" under the seeds. The page keeps its
// own copy of the items, applies each edit to it at once and sends the same
// edit to the set file (lib/sets.ts, src-tauri/src/sets.rs), one at a time
// and in order (`changeQueue`). Its keys are registered commands (the
// "Set page" and "Suggestions" groups of lib/keybindings.ts): the app's
// dispatcher runs them through `commandsRef` while this page is open, so the
// palette, settings and rebinding work here as everywhere.
import { useCallback, useEffect, useMemo, useRef, useState, type MutableRefObject } from "react";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";
import { copyText } from "../../lib/clipboard";
import { comboLabel, resolveBindings, type CommandId } from "../../lib/keybindings";
import type { CitedPassage } from "../../lib/archive";
import {
  acrossLine,
  applyLocal,
  changeQueue,
  errorText,
  mixLine,
  moveEdit,
  moveTarget,
  seedRange,
  selectionAfterRemove,
  setAdd,
  setDismiss,
  setEdit as saveEdit,
  setExport,
  setExportSave,
  setOpen,
  setRefresh,
  setSuggest,
  SkippedChange,
  type ExportKind,
  type OpenedSet,
  type SetEdit,
  type Suggestion,
  type SuggestionState,
} from "../../lib/sets";
import { Icon } from "../quickfinder/icons";
import { ExportPanel, ItemRow, SeedRow, SuggestionCard, keepPlainKeys } from "./SetParts";
import "./sets.css";

export interface SetPageProps {
  id: string;
  onBack: () => void;
  /** Something about the set changed (title, size): the rail and tray reload. */
  onChanged: () => void;
  onToast: (msg: string, ms?: number) => void;
  /** Where the page puts its command handlers for the app's dispatcher and palette. */
  commandsRef: MutableRefObject<PageCommands>;
  /** The page's keyboard scope changed: the items, or the suggestions. */
  onView: (view: "set" | "suggestions") => void;
  /** Bumped when saved searches were refreshed after an index build. */
  epoch: number;
}

export type PageCommands = Partial<Record<CommandId, () => void>>;

function openSource(c: CitedPassage | null, fallback: string, toast: (m: string) => void) {
  if (!c) { toast("The source changed: there is nothing to open for this item"); return; }
  if (c.public_url) openUrl(c.public_url).catch(() => toast("Could not open the link"));
  else if (c.link?.startsWith("writeflex://")) openUrl(c.link).catch(() => toast("Could not open WriteFlex"));
  else openPath(c.path || fallback).catch(() => toast("Could not open the file"));
}

export function SetPage({ id, onBack, onChanged, onToast, commandsRef, onView, epoch }: SetPageProps) {
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

  // A saved search took in new matches after an index build: show them.
  const savedRef = useRef(saved);
  savedRef.current = saved;
  useEffect(() => { if (epoch && savedRef.current) load(); }, [epoch, load]);

  // Every change to the set file waits for the one before it.
  const queue = useMemo(() => changeQueue(), [id]);

  /** Apply an edit to the page's copy at once and to the set file after the edits before it. */
  const edit = useCallback((e: SetEdit) => {
    setSet((s) => (s ? { ...s, items: applyLocal(s.items, e), title: e.op === "rename" ? e.title.trim() || s.title : s.title } : s));
    queue(() => saveEdit(id, e)).then(onChanged).catch((err) => {
      if (err instanceof SkippedChange) return;
      onToast(`Could not save the set: ${errorText(err)}`, 4000);
      load();
    });
  }, [id, load, onChanged, onToast, queue]);

  const move = (from: number, to: number) => {
    const e = moveEdit(items, from, to);
    if (!e) return;
    edit(e);
    setSel(to);
  };

  const remove = (i: number) => {
    const p = items[i]?.item.passage;
    if (!p) return;
    edit({ op: "remove", passage: p });
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
      await queue(() => setAdd(id, s.passage_id));
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
    try { await queue(() => setDismiss(id, s.passage_id)); } catch (e) { onToast(`Could not remember that: ${errorText(e)}`, 4000); }
  };

  const doExport = async (kind: ExportKind) => {
    setBusy(kind);
    try {
      // The app asks where to save and writes only there.
      const path = await setExportSave(id, kind);
      if (path) onToast(`Saved ${path.split("/").pop()}`, 3000);
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

  // The page's commands (registered in lib/keybindings.ts; the app dispatches them).
  const suggOpen = () => {
    const cur = sugg?.[suggSel];
    return cur && (states[cur.passage_id]?.state ?? "open") === "open" ? cur : null;
  };
  const advance = () => setSuggSel((i) => Math.min((sugg?.length ?? 1) - 1, i + 1));
  const at = (f: (i: number) => void) => () => { if (items.length) f(sel); };
  commandsRef.current = {
    setNext: at(() => setSel((i) => Math.min(items.length - 1, i + 1))),
    setPrev: at(() => setSel((i) => Math.max(0, i - 1))),
    setMoveDown: at((i) => { const to = moveTarget(i, 1, items.length); if (to !== null) move(i, to); }),
    setMoveUp: at((i) => { const to = moveTarget(i, -1, items.length); if (to !== null) move(i, to); }),
    setOpenSource: at((i) => openSource(items[i]?.cited ?? null, items[i]?.item.source_path ?? "", onToast)),
    setEditNote: at((i) => setEditing(i)),
    setRemove: at((i) => remove(i)),
    setRename: () => setRenaming(true),
    setMoreLike: () => { if (!suggesting) void startSuggest(); },
    setUpdateMatches: () => { if (saved) void refresh(); },
    setBack: onBack,
    suggestNext: advance,
    suggestPrev: () => setSuggSel((i) => Math.max(0, i - 1)),
    suggestAdd: () => { const cur = suggOpen(); if (cur) void add(cur).then(advance); },
    suggestReject: () => { const cur = suggOpen(); if (cur) void reject(cur).then(advance); },
    suggestStop: () => setSuggesting(false),
  };
  useEffect(() => () => { commandsRef.current = {}; }, [commandsRef]);
  useEffect(() => { onView(suggesting ? "suggestions" : "set"); }, [suggesting, onView]);
  useEffect(() => () => onView("set"), [onView]);

  // Hints show each command's current chord (rebindings included).
  const bindings = resolveBindings();
  const key = (c: CommandId) => (bindings[c] ? comboLabel(bindings[c]) : "");

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
            <button className="back" data-action="back" onClick={onBack} title={`Back to the search (${key("setBack")})`}>← Search</button>
            {renaming ? (
              <input
                className="title-edit"
                autoFocus
                defaultValue={set?.title}
                aria-label="Set title"
                onKeyDown={(e) => {
                  keepPlainKeys(e);
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
                  addKey={key("suggestAdd")}
                  rejectKey={key("suggestReject")}
                />
              ))}
            </div>
          ) : (
            <div className="sp-scroll" role="listbox" aria-label="Items" data-testid="set-items">
              {set && items.length === 0 && (
                <div className="qf-empty">
                  <p>This set is empty.</p>
                  <p className="hint">{saved ? "Update matches to fill it from the search." : key("addToSet") ? `Select a result and press ${key("addToSet")} (or + Set) to add it.` : "Select a result and press + Set to add it."}</p>
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
                  onNoteSave={(note) => { if (editing === i) { setEditing(null); if (note.trim() !== r.item.note) edit({ op: "note", passage: r.item.passage, note }); } }}
                  onNoteCancel={() => setEditing(null)}
                  onPin={() => edit({ op: "pin", passage: r.item.passage, pinned: !r.item.pinned })}
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
          <><span>{key("suggestAdd")} adds · {key("suggestReject")} rejects · the underlined words are what made it similar</span><span className="r"><kbd>{key("suggestAdd")}</kbd> add <kbd>{key("suggestReject")}</kbd> not this <kbd>{key("suggestStop")}</kbd> stop</span></>
        ) : (
          <><span>Drag or {key("setMoveUp")} {key("setMoveDown")} to reorder · {key("setOpenSource")} opens the source · {key("setEditNote")} edits the note</span><span className="r"><kbd>{key("setMoveUp")}</kbd><kbd>{key("setMoveDown")}</kbd> move <kbd>{key("setEditNote")}</kbd> note <kbd>{key("setRemove")}</kbd> remove <kbd>{key("setBack")}</kbd> back to search</span></>
        )}
      </div>
    </div>
  );
}
