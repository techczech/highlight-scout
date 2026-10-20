// The ⌘/ cheat sheet: every command by group with its effective chord; an unbound command reads "None".
import type { Bindings } from "@bridle/commands";
import { registry } from "./registry";

export interface CheatRow { id: string; label: string; chord: string; bound: boolean; aliases: string[] }
export interface CheatGroup { group: string; rows: CheatRow[] }

export function cheatRows(bindings: Bindings, aliases: (id: string) => string[] = () => []): CheatGroup[] {
  const groups: CheatGroup[] = [];
  for (const d of registry.defs) {
    if (d.hidden) continue;
    let g = groups.find((x) => x.group === d.group);
    if (!g) groups.push((g = { group: d.group, rows: [] }));
    const chord = Object.prototype.hasOwnProperty.call(bindings, d.id) ? bindings[d.id] ?? "" : "";
    g.rows.push({ id: d.id, label: d.label, chord: chord || "None", bound: !!chord, aliases: aliases(d.id) });
  }
  return groups;
}
