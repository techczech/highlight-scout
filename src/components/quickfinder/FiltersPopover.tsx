// HS-M1B: the one "Filters · Group" button and its popover. Every Classic
// tool that is not the search box, the rail or the pane lives here: View
// (Group, then, Sort, Rows, Match), Any corpus (Time) and Highlights
// (colour, Quick, Type, Tags). The Highlights section shows only while
// Highlights or one of its sources is ticked. Keyboard, as Classic's
// popover: ↑↓ move, Space / Enter toggles, Home / End, Esc closes (the app).
// State lives in the app; this holds only the tag field's text.
import { useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { SORT_LABEL, type ResultSort } from "../../lib/quickFinder";
import {
  DENSITIES,
  DENSITY_LABEL,
  GROUP_LABEL,
  SORTS,
  TIME_OPTIONS,
  TYPE_OPTIONS,
  buttonLabel,
  chips,
  clearFilters,
  effectiveGroup,
  effectiveSubgroup,
  groupOptions,
  subgroupOptions,
  type GroupBy,
  type SearchState,
} from "../../lib/searchModel";
import { resolveColor, type Density, type TagCount } from "../../types";
import { Icon } from "./icons";

export interface FiltersPopoverProps {
  state: SearchState;
  query: string;
  onState: (s: SearchState) => void;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Annotation colours in the library, most used first. */
  colors: string[];
  /** The library's tags with counts, for the tag field. */
  tags: TagCount[];
  /** Bumped by ⌘⇧T: focus the tag field. */
  tagFocus: number;
  onPickTag: (tag: string) => void;
  /** "X, Readwise, Zotero": the ticked highlight sources. */
  sourcesLabel: string;
}

/** Move focus among the popover's controls. */
function focusStep(root: HTMLElement, how: "next" | "prev" | "first" | "last") {
  const items = [...root.querySelectorAll<HTMLElement>("[data-fi]")].filter((el) => !(el as HTMLButtonElement).disabled);
  if (!items.length) return;
  const i = items.indexOf(document.activeElement as HTMLElement);
  const n = how === "first" ? 0 : how === "last" ? items.length - 1 : how === "next" ? Math.min(items.length - 1, i + 1) : Math.max(0, i - 1);
  items[n].focus();
}

function Check({ on, label, onClick, testid }: { on: boolean; label: ReactNode; onClick: () => void; testid?: string }) {
  return (
    <button className={`fp-check${on ? " on" : ""}`} role="menuitemcheckbox" aria-checked={on} data-fi data-testid={testid} onClick={onClick}>
      <span className="box" aria-hidden="true">{on ? "☑" : "☐"}</span>{label}
    </button>
  );
}

export function FiltersPopover(p: FiltersPopoverProps) {
  const ref = useRef<HTMLDivElement>(null);
  const popRef = useRef<HTMLDivElement>(null);
  const tagRef = useRef<HTMLInputElement>(null);
  const [tagText, setTagText] = useState("");
  const s = p.state;
  const f = s.filters;
  const set = (patch: Partial<SearchState>) => p.onState({ ...s, ...patch });
  const setF = (patch: Partial<typeof f>) => p.onState({ ...s, filters: { ...f, ...patch } });
  const active = chips(s, p.query).length > 0;

  // Close on a click outside.
  useEffect(() => {
    if (!p.open) return;
    const onDoc = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) p.onOpenChange(false);
    };
    document.addEventListener("mousedown", onDoc);
    return () => document.removeEventListener("mousedown", onDoc);
  }, [p.open, p.onOpenChange]);

  // On open, focus the first control, or the tag field after ⌘⇧T.
  useEffect(() => {
    if (!p.open) return;
    const t = setTimeout(() => {
      if (p.tagFocus && tagRef.current) tagRef.current.focus();
      else if (popRef.current) focusStep(popRef.current, "first");
    }, 0);
    return () => clearTimeout(t);
  }, [p.open, p.tagFocus]);

  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Escape") return; // the app closes the popover
    if (e.metaKey || e.ctrlKey) return; // ⌘⇧S, ⌘⇧G … still reach the app
    e.stopPropagation();
    const root = popRef.current;
    if (!root) return;
    const inText = (e.target as HTMLElement).tagName === "INPUT";
    if (e.key === "ArrowDown") { e.preventDefault(); focusStep(root, "next"); }
    else if (e.key === "ArrowUp") { e.preventDefault(); focusStep(root, "prev"); }
    else if (e.key === "Home" && !inText) { e.preventDefault(); focusStep(root, "first"); }
    else if (e.key === "End" && !inText) { e.preventDefault(); focusStep(root, "last"); }
  };

  const groups = groupOptions(s, p.query);
  const subs = subgroupOptions(s, p.query);
  const g = effectiveGroup(s, p.query);
  const semantic = s.mode === "semantic";
  const showHighlights = s.corpora.includes("highlights");
  const wider = s.corpora.length > 1;
  const q = tagText.trim().toLowerCase();
  const tagMatches = p.tags.filter((t) => !q || t.tag.toLowerCase().includes(q)).slice(0, 6);
  const pick = (tag: string) => { p.onPickTag(tag); setTagText(""); };

  return (
    <div ref={ref} className="qf-filters">
      <button
        className={`qf-fbtn${active ? " on" : ""}`}
        data-testid="filters-button"
        aria-haspopup="menu"
        aria-expanded={p.open}
        onClick={() => p.onOpenChange(!p.open)}
        title="Filters, colour, tags, group, sort, rows (⌘⇧I)"
      >
        <Icon name="filter" size="sm" />{buttonLabel(s, p.query)}
      </button>
      {p.open && (
        <div ref={popRef} className="qf-pop" role="menu" aria-label="Filters" data-testid="filters-popover" onKeyDown={onKeyDown}>
          <div className="cols">
            <div className="col">
              <section data-section="view">
                <div className="h">View</div>
                <label className="fp-row"><span>Group</span>
                  <select data-fi aria-label="Group" value={g} onChange={(e) => set({ group: e.target.value as GroupBy })}>
                    {groups.map((o) => <option key={o} value={o}>{GROUP_LABEL[o]}</option>)}
                  </select>
                  <span className="then">then</span>
                  <select data-fi aria-label="then" value={effectiveSubgroup(s, p.query)} disabled={!subs.length || g === "none"} onChange={(e) => set({ subgroup: e.target.value as GroupBy })}>
                    {(subs.length ? subs : (["none"] as GroupBy[])).map((o) => <option key={o} value={o}>{o === "none" ? "—" : GROUP_LABEL[o]}</option>)}
                  </select>
                </label>
                <label className="fp-row"><span>Sort</span>
                  <select data-fi aria-label="Sort" value={s.sort} onChange={(e) => set({ sort: e.target.value as ResultSort })}>
                    {SORTS.map((o) => <option key={o} value={o}>{SORT_LABEL[o]}</option>)}
                  </select>
                </label>
                <label className="fp-row"><span>Rows</span>
                  <select data-fi aria-label="Rows" value={s.density} onChange={(e) => set({ density: e.target.value as Density })}>
                    {DENSITIES.map((o) => <option key={o} value={o}>{DENSITY_LABEL[o]}</option>)}
                  </select>
                </label>
                <div className={`fp-row${semantic ? " off" : ""}`} data-testid="match-row"><span>Match</span>
                  <span className="seg">
                    <button data-fi disabled={semantic} className={!s.partial ? "on" : ""} onClick={() => set({ partial: false })}>Whole word</button>
                    <button data-fi disabled={semantic} className={s.partial ? "on" : ""} onClick={() => set({ partial: true })}>Partial</button>
                  </span>
                </div>
                <p className="note">Match applies to keyword search of highlights only. Group offers Corpus only when more than one corpus is searched; “then” sub-groups highlights.</p>
              </section>
              <section data-section="any">
                <div className="h">Any corpus</div>
                {TIME_OPTIONS.map((t) => (
                  <button key={t.value || "any"} className={`fp-radio${f.time === t.value ? " on" : ""}`} role="menuitemradio" aria-checked={f.time === t.value} data-fi onClick={() => setF({ time: t.value })}>
                    <span className="box" aria-hidden="true">{f.time === t.value ? "◉" : "○"}</span>{t.label}
                  </button>
                ))}
              </section>
            </div>
            {showHighlights && (
              <div className="col" data-section="highlights">
                <div className="h">Highlights{p.sourcesLabel ? <span className="src"> · {p.sourcesLabel}</span> : null}</div>
                {wider && <p className="note narrow">These filters narrow the results to highlights.</p>}
                {p.colors.length > 0 && (
                  <div className="fp-row colours"><span>Colour</span>
                    <span className="sw">
                      {p.colors.slice(0, 14).map((c) => (
                        <button key={c} data-fi title={c} aria-label={`Colour ${c}`} aria-pressed={s.color === c} className={s.color === c ? "on" : ""} style={{ backgroundColor: resolveColor(c) ?? "#fff" }} onClick={() => set({ color: s.color === c ? null : c })} />
                      ))}
                    </span>
                  </div>
                )}
                <div className="two">
                  <div>
                    <Check on={f.favorite} label="★ Favorites" onClick={() => setF({ favorite: !f.favorite })} />
                    <Check on={f.zotero} label="🔖 Zotero" onClick={() => setF({ zotero: !f.zotero })} />
                    <Check on={f.hasImage} label="🖼 Has image" onClick={() => setF({ hasImage: !f.hasImage })} />
                  </div>
                  <div>
                    {TYPE_OPTIONS.map((t) => (
                      <Check key={t.value} on={f.types.includes(t.value)} label={t.label} onClick={() => setF({ types: f.types.includes(t.value) ? f.types.filter((x) => x !== t.value) : [...f.types, t.value] })} />
                    ))}
                  </div>
                </div>
                <div className="fp-tags">
                  <div className="lab">Tags <kbd>⌘⇧T</kbd></div>
                  <div className="field">
                    <Icon name="search" size="sm" />
                    <input
                      ref={tagRef}
                      data-fi
                      data-testid="tag-field"
                      value={tagText}
                      onChange={(e) => setTagText(e.target.value)}
                      placeholder={`Filter by tag… (${p.tags.length})`}
                      onKeyDown={(e) => { if (e.key === "Enter" && tagMatches[0]) { e.preventDefault(); pick(tagMatches[0].tag); } }}
                      aria-label="Filter by tag"
                    />
                  </div>
                  <div className="tagsug">
                    {tagMatches.map((t) => (
                      <button key={t.tag} data-fi onClick={() => pick(t.tag)} title={`Add tag:"${t.tag}" to the search (${t.count})`}>{t.tag}</button>
                    ))}
                  </div>
                </div>
              </div>
            )}
          </div>
          <div className="foot">
            <span>↑↓ move</span><span>Space toggles</span><span>Esc closes</span>
            <button data-fi className="clear" onClick={() => p.onState(clearFilters(s))}>Clear all</button>
          </div>
        </div>
      )}
    </div>
  );
}
