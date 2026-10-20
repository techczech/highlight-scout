import { useMemo, useState } from "react";
import type { Bindings } from "@bridle/commands";
import { registry } from "../settings/registry";
import { Overlay } from "./TagPicker";

interface Props {
  bindings: Bindings;
  initialFilter?: string;
  onRun: (id: string) => void;
  onClose: () => void;
}

/** Command palette (⌘⇧P): every registry command with its effective chord; unbound ones show no chord. */
export function CommandPalette({ bindings, initialFilter = "", onRun, onClose }: Props) {
  const [filter, setFilter] = useState(initialFilter);
  const [active, setActive] = useState(0);

  const shown = useMemo(() => registry.paletteList(undefined, filter).map((r) => r.def), [filter]);

  const run = (i: number) => {
    const c = shown[i];
    if (c) onRun(c.id);
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") { e.preventDefault(); setActive((a) => Math.min(a + 1, shown.length - 1)); }
    else if (e.key === "ArrowUp") { e.preventDefault(); setActive((a) => Math.max(a - 1, 0)); }
    else if (e.key === "Enter") { e.preventDefault(); run(active); }
    // Escape is handled by App's single global key handler (closes the overlay).
    // A second handler here raced it and could hit the window-hide path.
  };

  return (
    <Overlay title="Command palette" onClose={onClose}>
      <input
        autoFocus
        value={filter}
        onChange={(e) => { setFilter(e.target.value); setActive(0); }}
        onKeyDown={onKeyDown}
        placeholder="Type a command…  (↑↓ to move, ↵ to run, esc to close)"
        className="mb-2 w-full rounded border border-zinc-200 px-3 py-2 text-sm outline-none focus:border-amber-400"
      />
      <div className="max-h-80 flex-1 overflow-y-auto">
        {shown.map((c, i) => (
          <button
            key={c.id}
            onMouseMove={() => setActive(i)}
            onClick={() => run(i)}
            className={`flex w-full items-center justify-between rounded px-3 py-1.5 text-left text-sm ${
              i === active ? "bg-amber-50" : "hover:bg-zinc-50"
            }`}
          >
            <span className="text-zinc-700">
              <span className="text-zinc-400">{c.group} · </span>
              {c.label}
            </span>
            {bindings[c.id] && <kbd className="rounded bg-zinc-100 px-1.5 py-0.5 text-xs text-zinc-500">{bindings[c.id]}</kbd>}
          </button>
        ))}
        {shown.length === 0 && <p className="p-3 text-sm text-zinc-400">No matching command</p>}
      </div>
    </Overlay>
  );
}
