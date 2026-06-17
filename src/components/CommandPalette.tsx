import { useMemo, useState } from "react";
import { COMMANDS, comboLabel, resolveBindings, type CommandId } from "../lib/keybindings";
import { Overlay } from "./TagPicker";

interface Props {
  onRun: (id: CommandId) => void;
  onClose: () => void;
}

/** Command palette + live keyboard-shortcut reference (⌘⇧P or ?). */
export function CommandPalette({ onRun, onClose }: Props) {
  const bindings = useMemo(() => resolveBindings(), []);
  const [filter, setFilter] = useState("");
  const [active, setActive] = useState(0);

  const shown = useMemo(() => {
    const f = filter.trim().toLowerCase();
    return COMMANDS.filter((c) => !f || c.label.toLowerCase().includes(f) || c.group.toLowerCase().includes(f));
  }, [filter]);

  const run = (i: number) => {
    const c = shown[i];
    if (c) onRun(c.id);
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") { e.preventDefault(); setActive((a) => Math.min(a + 1, shown.length - 1)); }
    else if (e.key === "ArrowUp") { e.preventDefault(); setActive((a) => Math.max(a - 1, 0)); }
    else if (e.key === "Enter") { e.preventDefault(); run(active); }
    else if (e.key === "Escape") { e.preventDefault(); onClose(); }
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
            <kbd className="rounded bg-zinc-100 px-1.5 py-0.5 text-xs text-zinc-500">{comboLabel(bindings[c.id])}</kbd>
          </button>
        ))}
        {shown.length === 0 && <p className="p-3 text-sm text-zinc-400">No matching command</p>}
      </div>
    </Overlay>
  );
}
