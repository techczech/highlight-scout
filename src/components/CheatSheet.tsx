import { useMemo } from "react";
import type { Bindings } from "@bridle/commands";
import { cheatRows } from "../settings/cheat";
import { Overlay } from "./TagPicker";

/** The ⌘/ cheat sheet: every command with its effective chord. A command with no chord reads "None". */
export function CheatSheet({ bindings, aliases, onClose }: { bindings: Bindings; aliases: (id: string) => string[]; onClose: () => void }) {
  const groups = useMemo(() => cheatRows(bindings, aliases), [bindings, aliases]);
  return (
    <Overlay title="Keyboard shortcuts" onClose={onClose} wide>
      <div className="max-h-[70vh] overflow-y-auto" data-testid="cheat-sheet">
        {groups.map((g) => (
          <div key={g.group} className="mb-3">
            <p className="mb-1 text-xs font-semibold uppercase tracking-wide text-zinc-400">{g.group}</p>
            {g.rows.map((r) => (
              <div key={r.id} className="flex items-center justify-between px-2 py-1 text-sm">
                <span className="text-zinc-700">{r.label}</span>
                <span>
                  {r.aliases.map((a) => <kbd key={a} className="mr-1 rounded bg-zinc-100 px-1.5 py-0.5 text-xs text-zinc-500">{a}</kbd>)}
                  {r.bound
                    ? <kbd className="rounded bg-zinc-100 px-1.5 py-0.5 text-xs text-zinc-500">{r.chord}</kbd>
                    : <span className="text-xs text-zinc-400">None</span>}
                </span>
              </div>
            ))}
          </div>
        ))}
      </div>
    </Overlay>
  );
}
