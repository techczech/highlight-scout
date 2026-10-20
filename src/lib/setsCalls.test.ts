import { beforeEach, describe, expect, test, vi } from "vitest";

const invoke = vi.fn(async (..._a: unknown[]) => null);
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

const sets = await import("./sets");

describe("the set calls the page makes", () => {
  beforeEach(() => invoke.mockClear());

  test("saving an export names no path: the backend asks the user", async () => {
    await sets.setExportSave("talk", "citations");
    expect(invoke).toHaveBeenCalledWith("set_export_save", { id: "talk", kind: "citations" });
    expect("setExportTo" in sets).toBe(false);
  });

  test("a classic highlight is added with its work, so the same words in another work are not taken", async () => {
    await sets.setAddText("talk", "highlights", "Same words", "schon-abc");
    expect(invoke).toHaveBeenCalledWith("set_add_text", { id: "talk", corpus: "highlights", text: "Same words", workSlug: "schon-abc" });
  });
});

describe("saved searches follow the index", () => {
  test("a finished build refreshes them; progress and failures do not", async () => {
    let handler: ((e: { payload: { phase: string; builds: { error?: string | null }[] } }) => void) | null = null;
    const listen = vi.fn(async (event: string, cb: typeof handler) => { expect(event).toBe("corpus:index"); handler = cb; return () => {}; });
    const refresh = vi.fn();
    await sets.followIndexBuilds(listen as never, refresh);
    handler!({ payload: { phase: "building", builds: [] } });
    handler!({ payload: { phase: "current", builds: [] } });
    handler!({ payload: { phase: "failed", builds: [{ error: "x" }] } });
    expect(refresh).not.toHaveBeenCalled();
    handler!({ payload: { phase: "built", builds: [{}] } });
    expect(refresh).toHaveBeenCalledOnce();
  });
});

describe("the page's changes reach the file one at a time", () => {
  const deferred = () => {
    let resolve!: () => void;
    let reject!: (e: unknown) => void;
    const p = new Promise<void>((a, b) => { resolve = a; reject = b; });
    return { p, resolve, reject };
  };
  const tick = () => new Promise((r) => setTimeout(r, 0));

  test("a second change starts only after the first finished, in order", async () => {
    const run = sets.changeQueue();
    const first = deferred();
    const started: string[] = [];
    const a = run(() => { started.push("move"); return first.p; });
    const b = run(async () => { started.push("note"); });
    await tick();
    expect(started).toEqual(["move"]);
    first.resolve();
    await a;
    await b;
    expect(started).toEqual(["move", "note"]);
  });

  test("after a failure the waiting changes are skipped, and later ones run", async () => {
    const run = sets.changeQueue();
    const first = deferred();
    const sent: string[] = [];
    const a = run(() => { sent.push("remove"); return first.p; });
    const b = run(async () => { sent.push("move"); });
    first.reject(new Error("disk full"));
    await expect(a).rejects.toThrow("disk full");
    await expect(b).rejects.toBeInstanceOf(sets.SkippedChange);
    await run(async () => { sent.push("rename"); });
    expect(sent).toEqual(["remove", "rename"]);
  });
});
