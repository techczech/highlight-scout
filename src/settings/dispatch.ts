// One dispatch path for commands. A command can arrive from the key handler or from a menu item; the menu shows its
// accelerator, so on macOS the same key press may reach both. The dispatcher drops the second arrival of one command
// from a different source inside a short window, so a chord never runs twice. Key repeat (same source) is untouched.
// Only the last arrival that ran is remembered: a dropped duplicate never renews the window.
export type Source = "key" | "menu" | "palette";

export interface Dispatcher {
  (id: string, source: Source): boolean;
}

export function createDispatcher(run: (id: string) => void, windowMs = 150, now: () => number = () => Date.now()): Dispatcher {
  let last: { id: string; source: Source; at: number } | null = null;
  return (id, source) => {
    const t = now();
    if (last && last.id === id && last.source !== source && t - last.at < windowMs) return false;
    last = { id, source, at: t };
    run(id);
    return true;
  };
}
