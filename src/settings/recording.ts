// Whether the app menu's accelerators should be suspended, from the recording state itself (never from focus events alone: recording a
// chord or pressing Escape removes the focused recorder and WebKit then fires no blur). Pure, so it is tested without a DOM.

/** The Shortcuts page's recorder (the "Press new shortcut…" button the package renders while a row records). */
export function isRecorder(el: { tagName?: string; getAttribute?(n: string): string | null } | null): boolean {
  return !!el && el.tagName === "BUTTON" && (el.getAttribute?.("aria-label") ?? "").startsWith("New shortcut for ");
}

export interface RecordingFacts {
  /** A recorder is in the page. */
  recorder: boolean;
  /** The Settings window has focus. */
  focused: boolean;
  /** The page is visible (not hidden or being unloaded). */
  visible: boolean;
}

/** Calls `send(true)` when suspension should start and `send(false)` when it should end; repeated facts send nothing. */
export function createSuspendController(send: (suspend: boolean) => void): { update(f: RecordingFacts): void; end(): void } {
  let on = false;
  const set = (v: boolean) => { if (v !== on) { on = v; send(v); } };
  return {
    update: (f) => set(f.recorder && f.focused && f.visible),
    end: () => set(false),
  };
}
