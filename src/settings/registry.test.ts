import { describe, expect, test } from "vitest";
import { validateMenuCoverage, type KeyLike } from "@bridle/commands";
import commands from "./commands.json";
import schemaJson from "./schema.json";
import { MENUS, commandForEvent, plainCopyCommand, registry, schema } from "./registry";
import { cheatRows } from "./cheat";
import { createDispatcher } from "./dispatch";
import { menuTemplate, planMenu } from "./menu";

const key = (code: string, mods: Partial<KeyLike> = {}): KeyLike => ({ code, metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, ...mods });
const ctx = { inField: false, onControl: false, overlay: false };
const bindings = registry.bindingsOf({});

describe("registry over commands.json", () => {
  test("registry.defs matches commands.json (same ids, labels, keys; frame commands included once)", () => {
    const fromJson = commands.map((c) => [c.id, c.label, c.key]);
    const fromRegistry = registry.defs.map((d) => [d.id, d.label, d.key]);
    expect(fromRegistry).toEqual(fromJson);
  });

  test("the settings schema builds and holds the three key slots", () => {
    const secrets = schemaJson.settings.filter((s) => s.type === "secret").map((s) => s.key);
    expect(secrets).toEqual(["accounts.readwise", "accounts.r2AccessKeyId", "accounts.r2SecretAccessKey"]);
    expect(schema.app).toBe("highlight-scout");
  });

  test("every menu command is in exactly one menu", () => {
    expect(validateMenuCoverage(registry, MENUS)).toEqual([]);
  });
});

describe("menu bar", () => {
  const template = menuTemplate("Highlight Scout", bindings, () => {});
  const plan = planMenu(template);
  const labels = (items: ReturnType<typeof planMenu>): string[] => items.flatMap((i) => (i.kind === "submenu" ? labels(i.items) : i.kind === "item" ? [i.text] : []));

  test("lists every menu command once, with effective chords", () => {
    const all = labels(plan);
    for (const d of registry.defs.filter((x) => !x.noMenu)) expect(all.filter((l) => l === d.label).length).toBe(1);
    const app = plan[0];
    expect(app.kind === "submenu" && app.items.some((i) => i.kind === "item" && i.text === "Settings…" && i.accelerator === "Cmd+,")).toBe(true);
  });

  test("a rebound chord shows in the menu", () => {
    const b = registry.bindingsOf({ "cycle-sort": "⌘⌥S" });
    const t = planMenu(menuTemplate("x", b, () => {}));
    const view = t.find((m) => m.kind === "submenu" && m.text === "View");
    expect(view && view.kind === "submenu" && view.items.find((i) => i.kind === "item" && i.text === "Change the sort order")).toMatchObject({ accelerator: "Cmd+Alt+S" });
  });

  test("clicking an item runs its command id", () => {
    const ran: string[] = [];
    const t = menuTemplate("x", bindings, (id) => ran.push(id));
    const find = (items: typeof t): (typeof t)[number] | undefined => { for (const i of items) { if (i.label === "Open the source") return i; const f = i.submenu && find(i.submenu); if (f) return f; } };
    find(t)!.click!();
    expect(ran).toEqual(["open-source"]);
  });
});

describe("keys and dispatch", () => {
  test("⌘C copies the quote only when no text is selected; with a selection the native copy runs", () => {
    const c = key("KeyC", { metaKey: true });
    expect(plainCopyCommand(c, null, "", { overlay: false })).toBe("copy-highlight");
    expect(plainCopyCommand(c, null, "some words", { overlay: false })).toBeNull();
    expect(plainCopyCommand(c, { selectionStart: 2, selectionEnd: 5 }, "", { overlay: false })).toBeNull();
    expect(plainCopyCommand(c, { selectionStart: 3, selectionEnd: 3 }, "", { overlay: false })).toBe("copy-highlight");
    expect(plainCopyCommand(c, null, "", { overlay: true })).toBeNull();
    expect(plainCopyCommand(key("KeyC", { metaKey: true, shiftKey: true }), null, "", { overlay: false })).toBeNull();
  });

  test("the import log, OCR and the meaning index are commands in the Import menu", () => {
    const plan = planMenu(menuTemplate("x", bindings, () => {}));
    const imp = plan.find((m) => m.kind === "submenu" && m.text === "Import");
    const texts = imp && imp.kind === "submenu" ? imp.items.filter((i) => i.kind === "item").map((i) => (i as { text: string }).text) : [];
    for (const l of ["View import log…", "OCR images", "Build meaning index"]) expect(texts).toContain(l);
  });

  test("the registry's chords run their commands; typing rules apply", () => {
    expect(commandForEvent(key("KeyL", { metaKey: true }), bindings, ctx)).toBe("focus-search");
    expect(commandForEvent(key("Comma", { metaKey: true }), bindings, ctx)).toBe("settings");
    expect(commandForEvent(key("Slash", { metaKey: true }), bindings, ctx)).toBe("cheat");
    expect(commandForEvent({ ...key("Slash", { shiftKey: true }), key: "?" }, bindings, ctx)).toBe("cheat");
    expect(commandForEvent({ ...key("Slash", { shiftKey: true }), key: "?" }, bindings, { ...ctx, inField: true })).toBeNull();
    expect(commandForEvent(key("KeyL", { metaKey: true }), bindings, { ...ctx, overlay: true })).toBeNull();
  });

  test("a chord reaching both the menu and the key handler runs once", () => {
    const ran: string[] = [];
    let t = 1000;
    const d = createDispatcher((id) => ran.push(id), 150, () => t);
    expect(d("open-filters", "menu")).toBe(true);
    t += 20;
    expect(d("open-filters", "key")).toBe(false);
    t += 500;
    expect(d("open-filters", "key")).toBe(true);
    t += 20;
    expect(d("open-filters", "key")).toBe(true); // key repeat is the same source
    expect(ran).toEqual(["open-filters", "open-filters", "open-filters"]);
  });
});

describe("dispatch dedupe and menu ids", () => {
  test("a dropped duplicate never renews the window: menu/key pairs at 0/20, 80/100, 160/180 run three times", () => {
    const ran: string[] = [];
    let t = 0;
    const d = createDispatcher((id) => ran.push(id), 150, () => t);
    for (const [m, k] of [[0, 20], [80, 100], [160, 180]]) {
      t = m; d("cycle-sort", "menu");
      t = k; d("cycle-sort", "key");
    }
    expect(ran).toEqual(["cycle-sort", "cycle-sort", "cycle-sort"]);
  });

  test("menu items carry their command id, so the host can suspend and restore their accelerators", () => {
    const ids = (items: ReturnType<typeof planMenu>): string[] => items.flatMap((i) => (i.kind === "submenu" ? ids(i.items) : i.kind === "item" && i.id ? [i.id] : []));
    const all = ids(planMenu(menuTemplate("x", bindings, () => {})));
    expect(all).toContain("cycle-sort");
    expect(all).toContain("settings");
    expect(new Set(all).size).toBe(all.length);
    for (const id of all) expect(registry.byId(id)).toBeDefined();
  });
});

describe("cheat sheet", () => {
  test("lists every command, unbound as None", () => {
    const groups = cheatRows(bindings);
    const rows = groups.flatMap((g) => g.rows);
    for (const d of registry.defs.filter((x) => !x.hidden)) expect(rows.map((r) => r.id)).toContain(d.id);
    expect(rows.find((r) => r.id === "copy-image")).toMatchObject({ chord: "None", bound: false });
    expect(rows.find((r) => r.id === "settings")).toMatchObject({ chord: "⌘,", bound: true });
  });
});
