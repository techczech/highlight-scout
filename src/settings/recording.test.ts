import { describe, expect, test } from "vitest";
import { createSuspendController, isRecorder } from "./recording";

describe("menu accelerators while a shortcut is recorded (Sol r4)", () => {
  test("recording a chord or Escape removes the recorder without a blur, and that alone restores; so do window blur and hiding", () => {
    const sent: boolean[] = [];
    const c = createSuspendController((v) => sent.push(v));
    const f = { recorder: true, focused: true, visible: true };
    c.update(f);
    c.update(f);
    c.update({ ...f, recorder: false }); // the recorder unmounted: no focus event needed
    expect(sent).toEqual([true, false]);
    c.update(f);
    c.update({ ...f, focused: false }); // the Settings window lost focus
    c.update(f);
    c.update({ ...f, visible: false }); // the page hid
    c.update(f);
    c.end(); // page unloaded or the hook unmounted
    c.end();
    expect(sent).toEqual([true, false, true, false, true, false, true, false]);
  });

  test("the recorder is recognised by its label", () => {
    const el = (tagName: string, label: string) => ({ tagName, getAttribute: () => label });
    expect(isRecorder(el("BUTTON", "New shortcut for Change the sort order"))).toBe(true);
    expect(isRecorder(el("BUTTON", "Change the sort order"))).toBe(false);
    expect(isRecorder(el("DIV", "New shortcut for x"))).toBe(false);
    expect(isRecorder(null)).toBe(false);
  });
});
