import { describe, expect, test, vi } from "vitest";
import { isValidElement, type ReactElement, type ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { ExportPanel, ItemRow, SeedRow, SuggestionCard, itemHead, type ItemRowProps } from "./SetParts";
import { SetPicker } from "./SetPicker";
import type { CitedPassage } from "../../lib/archive";
import type { ResolvedItem, Suggestion } from "../../lib/sets";

type Host = { type: string; props: Record<string, unknown>; children: Host[] };
function expand(node: ReactNode): Host[] {
  if (node === null || node === undefined || typeof node === "boolean") return [];
  if (Array.isArray(node)) return node.flatMap(expand);
  if (!isValidElement(node)) return [];
  const el = node as ReactElement<Record<string, unknown>>;
  if (typeof el.type === "function") return expand((el.type as (p: unknown) => ReactNode)(el.props));
  if (typeof el.type !== "string") return expand(el.props.children as ReactNode);
  return [{ type: el.type, props: el.props, children: expand(el.props.children as ReactNode) }];
}
const all = (n: Host[], f: (h: Host) => boolean): Host[] => n.flatMap((h) => [...(f(h) ? [h] : []), ...all(h.children, f)]);
const act = (h: Host) => (h.props.onClick as (e: { stopPropagation: () => void }) => void)({ stopPropagation() {} });

const cited = (o: Partial<CitedPassage> = {}): CitedPassage => ({
  schema_version: 1, passage_id: "writing:a.md:3", corpus: "writing", rel_path: "a.md", path: "/x/a.md", line_start: 3, line_end: 3,
  quote: "But being open to repaving once new paths are trodden.", title: "Repaved paths and generative metaphors", author: "Dominik Lukeš",
  date: "2016-06-23", date_display: "23 June 2016", link: null, public_url: null, citation: { markdown: "m", plain: "p" }, ...o,
});
const item = (o: Partial<ResolvedItem["item"]> = {}, r: Partial<ResolvedItem> = {}): ResolvedItem => ({
  item: { id: "3f9a2c1b7d4e", passage: "writing:a.md:3", corpus: "writing", source_path: "a.md", quote: "Stored text", pinned: false, note: "", ...o },
  status: "ok", cited: cited(), ...r,
});

function rowProps(o: Partial<ItemRowProps> = {}): ItemRowProps {
  return {
    index: 0, row: item(), selected: true, editing: false, pinnable: false, dragOver: false,
    onSelect: vi.fn(), onNote: vi.fn(), onNoteSave: vi.fn(), onNoteCancel: vi.fn(), onPin: vi.fn(),
    onDragStart: vi.fn(), onDragOver: vi.fn(), onDrop: vi.fn(), onOpen: vi.fn(), ...o,
  };
}

describe("a set item", () => {
  test("writing shows its date and a bold title; highlights name author and work in the meta line", () => {
    expect(itemHead(item())).toEqual({ meta: ["23 June 2016"], title: "Repaved paths and generative metaphors", titleInMeta: false });
    const h = itemHead(item({ corpus: "highlights" }, { cited: cited({ corpus: "highlights", author: "Helen Sword", title: "Air & Light", date_display: null }) }));
    expect(h.meta).toEqual(["Helen Sword", "Air & Light"]);
    expect(h.title).toBe("");
  });

  test("the row shows the number, the original quote and its note", () => {
    const html = renderToStaticMarkup(<ItemRow {...rowProps({ row: item({ note: "Open the talk with this" }) })} />);
    expect(html).toContain('data-testid="set-item"');
    expect(html).toContain(">1<");
    expect(html).toContain("But being open to repaving once new paths are trodden.");
    expect(html).toContain("Open the talk with this");
    expect(html).toContain("WRITING".charAt(0) + "riting");
    expect(renderToStaticMarkup(<ItemRow {...rowProps()} />)).toContain("Add a note…");
  });

  test("a changed source keeps its stored quote and says so; a moved one says it was found again", () => {
    const changed = renderToStaticMarkup(<ItemRow {...rowProps({ row: item({ quote: "Stored text" }, { status: "changed", cited: null }) })} />);
    expect(changed).toContain("source changed");
    expect(changed).toContain("Stored text");
    expect(renderToStaticMarkup(<ItemRow {...rowProps({ row: item({}, { status: "repaired" }) })} />)).toContain("moved · found again");
  });

  test("the note field saves on Enter and drops on Esc; the pin shows only in saved-search sets", () => {
    const p = rowProps({ editing: true });
    const input = all(expand(<ItemRow {...p} />), (h) => h.type === "input")[0];
    const stop = vi.fn();
    (input.props.onKeyDown as (e: unknown) => void)({ key: "Enter", currentTarget: { value: "a note" }, stopPropagation: stop });
    expect(p.onNoteSave).toHaveBeenCalledWith("a note");
    (input.props.onKeyDown as (e: unknown) => void)({ key: "Escape", currentTarget: { value: "x" }, stopPropagation: stop });
    expect(p.onNoteCancel).toHaveBeenCalledOnce();
    expect(stop).toHaveBeenCalledTimes(2);
    expect(renderToStaticMarkup(<ItemRow {...rowProps()} />)).not.toContain('data-action="pin"');
    const pinned = rowProps({ pinnable: true });
    act(all(expand(<ItemRow {...pinned} />), (h) => h.props["data-action"] === "pin")[0]);
    expect(pinned.onPin).toHaveBeenCalledOnce();
  });

  test("in the note field plain keys stay in the field and ⌘ chords reach the app (palette, settings)", () => {
    const p = rowProps({ editing: true });
    const input = all(expand(<ItemRow {...p} />), (h) => h.type === "input")[0];
    const press = (o: Record<string, unknown>) => {
      const stop = vi.fn();
      (input.props.onKeyDown as (e: unknown) => void)({ currentTarget: { value: "" }, stopPropagation: stop, ...o });
      return stop.mock.calls.length;
    };
    expect(press({ key: "Backspace" })).toBe(1);
    expect(press({ key: "n" })).toBe(1);
    expect(press({ key: "P", metaKey: true, shiftKey: true })).toBe(0);
    expect(press({ key: ",", metaKey: true })).toBe(0);
    expect(press({ key: ",", ctrlKey: true })).toBe(0);
  });

  test("an unpinned saved-search item says it is kept out after updates", () => {
    expect(renderToStaticMarkup(<ItemRow {...rowProps({ pinnable: true, row: item({}, { kept_out: true }) })} />)).toContain("kept out after updates");
    expect(renderToStaticMarkup(<ItemRow {...rowProps({ pinnable: true })} />)).not.toContain("kept out");
  });

  test("dragging reports the row it starts on and the row it drops on", () => {
    const p = rowProps({ index: 2 });
    const root = all(expand(<ItemRow {...p} />), (h) => h.props["data-testid"] === "set-item")[0];
    expect(root.props.draggable).toBe(true);
    (root.props.onDragStart as () => void)();
    (root.props.onDragOver as (e: { preventDefault: () => void }) => void)({ preventDefault() {} });
    (root.props.onDrop as (e: { preventDefault: () => void }) => void)({ preventDefault() {} });
    expect(p.onDragStart).toHaveBeenCalledOnce();
    expect(p.onDragOver).toHaveBeenCalledOnce();
    expect(p.onDrop).toHaveBeenCalledOnce();
  });

  test("a seed is one compact line", () => {
    const html = renderToStaticMarkup(<SeedRow index={1} row={item({ corpus: "highlights" }, { cited: cited({ corpus: "highlights", author: "Helen Sword" }) })} />);
    expect(html).toContain("Helen Sword");
    expect(html).toContain(">2<");
  });
});

describe("a suggestion", () => {
  const s: Suggestion = {
    passage_id: "highlights:b.md:7", corpus: "highlights", rel_path: "b.md", quote: "The metaphors we choose to employ literally determine what we will see",
    title: "Questioning the OER Orthodoxy", author: "david", date_display: null, shared_terms: ["metaphors", "employ"], similar_to: "Hacking a metaphor in five steps", score: 0.4,
  };
  const props = (o = {}) => ({ s, state: "open" as const, selected: false, addedAs: null, onSelect: vi.fn(), onAdd: vi.fn(), onReject: vi.fn(), ...o });

  test("shows the passage with the shared words underlined and the seed it resembles", () => {
    const html = renderToStaticMarkup(<SuggestionCard {...props()} />);
    expect(html).toContain("<u>metaphors</u>");
    expect(html).toContain("<u>employ</u>");
    expect(html).toContain("Similar to <b>Hacking a metaphor in five steps</b>");
    expect(html).toContain("shared: metaphors, employ");
  });

  test("Add and Not this call their handlers", () => {
    const p = props();
    const tree = expand(<SuggestionCard {...p} />);
    act(all(tree, (h) => h.props["data-action"] === "add")[0]);
    act(all(tree, (h) => h.props["data-action"] === "reject")[0]);
    expect(p.onAdd).toHaveBeenCalledOnce();
    expect(p.onReject).toHaveBeenCalledOnce();
  });

  test("added says which item it became; rejected says it will not return", () => {
    const added = renderToStaticMarkup(<SuggestionCard {...props({ state: "added", addedAs: 8 })} />);
    expect(added).toContain("Added as item 8");
    expect(added).not.toContain('data-action="add"');
    const rej = renderToStaticMarkup(<SuggestionCard {...props({ state: "rejected" })} />);
    expect(rej).toContain("Rejected · will not be suggested again");
    expect(rej).not.toContain('data-action="reject"');
  });

  test("text from the archive is never HTML", () => {
    const html = renderToStaticMarkup(<SuggestionCard {...props({ s: { ...s, quote: "a <b>bold</b> metaphors claim" } })} />);
    expect(html).toContain("&lt;b&gt;bold&lt;/b&gt;");
  });
});

describe("export panel", () => {
  test("three exports, each with the engine's wording, off while the set is empty", () => {
    const p = { disabled: false, busy: null, onExport: vi.fn(), onCopy: vi.fn() };
    const html = renderToStaticMarkup(<ExportPanel {...p} />);
    for (const w of ["WriteFlex piece", "TalkWeaver quote slides", "Citation list", "Saved as a file in the archive."]) expect(html).toContain(w);
    const tree = expand(<ExportPanel {...p} />);
    act(all(tree, (h) => h.props["data-export"] === "talkweaver")[0]);
    act(all(tree, (h) => h.props["data-copy"] === "citations")[0]);
    expect(p.onExport).toHaveBeenCalledWith("talkweaver");
    expect(p.onCopy).toHaveBeenCalledWith("citations");
    const off = expand(<ExportPanel {...p} disabled />);
    expect(all(off, (h) => "data-export" in h.props).every((h) => h.props.disabled === true)).toBe(true);
  });
});

describe("set picker", () => {
  test("lists the sets to add to, and starts a new one", () => {
    const sets = [{ id: "metaphor-talk", title: "Metaphor talk", kind: "hand" as const, items: 7, updated: "d" }];
    const html = renderToStaticMarkup(<SetPicker mode="add" sets={sets} onPick={vi.fn()} onCreate={vi.fn()} onClose={vi.fn()} />);
    expect(html).toContain("Add to a set");
    expect(html).toContain("Metaphor talk · 7 items");
    expect(html).toContain("Or start a new set");
    const none = renderToStaticMarkup(<SetPicker mode="add" sets={[]} onPick={vi.fn()} onCreate={vi.fn()} onClose={vi.fn()} />);
    expect(none).toContain("Name the set");
    expect(none).toContain("Create and add");
  });

  test("saving a search explains what the set follows", () => {
    const html = renderToStaticMarkup(<SetPicker mode="search" sets={[]} query="scaffolding" defaultTitle="scaffolding" onPick={vi.fn()} onCreate={vi.fn()} onClose={vi.fn()} />);
    expect(html).toContain("Save this search as a set");
    expect(html).toContain("follows “scaffolding”");
    expect(html).toContain('value="scaffolding"');
  });
});
