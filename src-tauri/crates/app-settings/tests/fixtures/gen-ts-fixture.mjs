// Writes ts-script.json: a scripted run of the real @bridle/settings and @bridle/commands packages (deterministic clock), with the
// settings file text after every step and the export file text. The Rust crate replays the same script and must produce the same bytes.
// Run from the app's root: `bun src-tauri/crates/app-settings/tests/fixtures/gen-ts-fixture.mjs`.
import { writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { defineRegistry } from "@bridle/commands";
import {
  applyImport, commandsAdapter, defineSchema, emptyState, exportFile, loadState, previewImport, rebind, rebindMany, recordSecret, reset,
  resetAll, resetEntry, resetSection, set, undoBatch,
} from "@bridle/settings";

const schemaInput = {
  app: "writeflex",
  sections: [
    { id: "editor", label: "Editor", icon: "pen-line" },
    { id: "library", label: "Library", icon: "library" },
  ],
  settings: [
    { key: "appearance.theme", label: "Appearance", section: "appearance", type: "choice", default: "light", options: [{ value: "light", label: "Light" }, { value: "dark", label: "Dark" }] },
    { key: "editor.proseSize", label: "Prose size", section: "editor", group: "Text", type: "number", default: 16, min: 10, max: 32, unit: "pt", help: "Font size of the prose" },
    { key: "editor.focusMode", label: "Reading focus", section: "editor", type: "toggle", default: false },
    { key: "editor.spacing", label: "Line spacing", section: "editor", type: "number", default: 1 },
    { key: "editor.skipPrefix", label: "Skip lines starting with", section: "editor", type: "text", default: "%%" },
    { key: "library.folder", label: "Workspace folder", section: "library", type: "path", default: "~/Documents/Writing" },
    { key: "shortcuts.globalHotkey", label: "Show the app from anywhere", section: "shortcuts", type: "chord", default: "" },
    { key: "accounts.speech", label: "Speech service key", section: "accounts", type: "secret", default: null },
  ],
};
const commandDefs = [
  { id: "open", group: "File", label: "Open", key: "⌘O", icon: "folder-open" },
  { id: "pin", group: "File", label: "Pin or unpin", key: "⌘D", icon: "pin" },
  { id: "rename", group: "File", label: "Rename", key: "⌘R", icon: "pencil" },
  { id: "next", group: "Navigation", label: "Next", key: "↓", icon: "arrow-down", noMenu: true, noMenuReason: "movement key" },
];
const schema = defineSchema(schemaInput);
const registry = defineRegistry(commandDefs);
const commands = commandsAdapter(registry);
let t = 1_760_000_000_000;
let n = 0;
const clock = { now: () => t++, newId: () => `id${++n}` };
const ctx = (who) => ({ schema, commands, who, ...clock });
const you = { kind: "you" };
const agent = { kind: "agent", name: "Claude Code" };

const steps = [];
let state = emptyState("writeflex");
const text = (s) => JSON.stringify(s, null, 2) + "\n";
const run = (op, args, r) => {
  if (r.ok) state = r.state;
  steps.push({ op, args, ok: r.ok, ...(r.ok ? { entries: r.entries } : { code: r.code, error: r.error }), ...(r.skipped ? { skipped: r.skipped } : {}), file: text(state) });
};

run("set", { key: "editor.proseSize", value: 18, who: "you" }, set(state, "editor.proseSize", 18, ctx(you)));
run("set", { key: "appearance.theme", value: "dark", who: "agent" }, set(state, "appearance.theme", "dark", ctx(agent)));
run("set", { key: "editor.proseSize", value: 99, who: "you" }, set(state, "editor.proseSize", 99, ctx(you)));
run("rebind", { id: "open", chord: "⌘D", who: "you" }, rebind(state, "open", "⌘D", ctx(you)));
run("rebindMany", { pairs: [["open", "⌘D"], ["pin", "⌘O"]], who: "you" }, rebindMany(state, [{ commandId: "open", chord: "⌘D" }, { commandId: "pin", chord: "⌘O" }], ctx(you)));
run("recordSecret", { key: "accounts.speech", from: "missing", to: "connected", who: "you" }, recordSecret(state, "accounts.speech", "missing", "connected", ctx(you)));
run("set", { key: "editor.focusMode", value: true, who: "you" }, set(state, "editor.focusMode", true, ctx(you)));
run("reset", { key: "editor.proseSize", who: "you" }, reset(state, "editor.proseSize", ctx(you)));
const themeEntry = state.changes.find((e) => e.key === "appearance.theme").id;
run("resetEntry", { id: themeEntry, who: "you" }, resetEntry(state, themeEntry, ctx(you)));
const exported = exportFile(state, { appVersion: "0.31.0", hostName: "Test Mac", now: 1_760_000_500_000, schema, commands });
const importText = JSON.stringify({
  format: "bridle-settings-export", version: 1, app: "writeflex", appVersion: "0.30.0", exportedFrom: "Other Mac", exportedAt: "2026-10-17T18:04:00.000Z",
  settings: { "editor.proseSize": 20, "editor.focusMode": true, "nope.unknown": 1, "accounts.speech": "secret!", "library.folder": "~/Dropbox/W", "editor.skipPrefix": 5 },
  keys: { rename: "⌘E", open: "⌘,", ghost: "⌘G" },
});
const preview = previewImport(state, importText, { schema, commands });
run("applyImport", { text: importText, file: "WriteFlex Settings 2026-10-17.json", who: "agent", preview }, applyImport(state, preview, { file: "WriteFlex Settings 2026-10-17.json" }, { ...ctx(agent), by: agent }));
const batch = state.changes[0].batch;
run("set", { key: "library.folder", value: "~/Elsewhere", who: "you" }, set(state, "library.folder", "~/Elsewhere", ctx(you)));
run("undoBatch", { batch, who: "you" }, undoBatch(state, batch, ctx(you)));
// Numbers JavaScript writes in plain decimal and in exponent form (the edges of Number::toString).
for (const value of [0.000001, 1e-7, 1e21, 123.456, 2.5e-6, 1e20, -0.5]) run("set", { key: "editor.spacing", value, who: "you" }, set(state, "editor.spacing", value, ctx(you)));
run("resetSection", { section: "shortcuts", who: "you" }, resetSection(state, "shortcuts", ctx(you)));
run("resetAll", { who: "agent" }, resetAll(state, ctx(agent)));

// Loader: a damaged mix and a legacy file through migrate.
const messy = {
  format: "bridle-settings", version: 2, app: "writeflex",
  settings: { "editor.proseSize": "big", "appearance.theme": "dark", "accounts.speech": "leak", "gone.key": 1, "editor.focusMode": false },
  keys: { open: "⌘,", pin: "⌘J", ghost: "⌘G" },
  changes: [{ id: "old1", at: 5, key: "editor.proseSize", from: 16, to: 18, who: { kind: "you" }, extra: "dropped" }, { bogus: true }],
};
const loadCtx = { schema, commands, appVersion: "0.32.0", ...clock, migrate: (old) => (old && old.legacy ? { settings: { "editor.proseSize": old.legacy.size, "appearance.theme": "light" }, keys: { rename: "⌘E" } } : null) };
const c0 = { t, n };
const loadedMessy = loadState(messy, loadCtx);
const c1 = { t, n };
const loadedLegacy = loadState({ legacy: { size: 20 } }, loadCtx);

// JSON.stringify over source texts (numbers at the exponent edges, integer-like keys, escapes): what the Rust writer must match.
const jsonSources = [
  "[0.000001, 1e-7, 1e21, 1e20, 1.5e300, 5e-324, 123.456, -0, 0.1, 100, 12345678901234567890, 2.5e-6, -1e-7, 1e-6]",
  '{"b": 1, "10": 2, "a": {"2": [], "01": {}, "1": null}, "2": true, "4294967295": 0, "4294967294": 0}',
  JSON.stringify("quote \" backslash \\ nl \n tab \t bs \b ff \f cr \r c1 \u0001 del \u007f c2 \u0085 ls \u2028 e \u00e9 emoji \ud83d\ude00"),
  "[]", "{}", '{"x": [[], {}, [1, [2, {"y": 0.000001}]]]}',
];
const json = jsonSources.map((src) => {
  const v = JSON.parse(src);
  return { src, compact: JSON.stringify(v), pretty: JSON.stringify(v, null, 2) + "\n" };
});
const numberStrings = [0.000001, 1e-7, 1e21, 1e20, 123.456, -0, 5e-324, 1.7976931348623157e308, 0.1 + 0.2, 1 / 3, 9007199254740993].map((n) => ({ n, s: String(n) }));

const out = {
  schema: schemaInput,
  commands: registry.defs.map((d) => ({ id: d.id, label: d.label, key: d.key, ...(d.fixed ? { fixed: true } : {}) })),
  start: { t: 1_760_000_000_000 },
  steps,
  export: { now: 1_760_000_500_000, text: text(exported) },
  preview,
  loads: [
    { raw: messy, clock: c0, ...summary(loadedMessy) },
    { raw: { legacy: { size: 20 } }, clock: c1, ...summary(loadedLegacy) },
  ],
  json,
  numberStrings,
  clockEnd: { t, n },
};
function summary(r) {
  return { file: text(r.state), entries: r.entries.length, dropped: r.dropped, migrated: r.migrated, problem: r.problem ?? null };
}
writeFileSync(join(dirname(fileURLToPath(import.meta.url)), "ts-script.json"), JSON.stringify(out, null, 2) + "\n");
console.log(`wrote ${steps.length} steps`);
