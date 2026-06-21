import { useEffect, useMemo, useState } from "react";
import { getSettings, saveSettings, setAutostart } from "../lib/api";
import type { Settings } from "../types";
import { Overlay } from "./TagPicker";
import { APP_VERSION, RELEASE_NOTES } from "../version";
import {
  COMMANDS,
  comboLabel,
  eventToCombo,
  resetBindings,
  resolveBindings,
  setBinding,
  type CommandId,
} from "../lib/keybindings";
import { ImportButtons } from "./ImportMenu";
import type { ImportAction } from "./Toolbar";
import { TEXT_SIZES, getTextSize, setTextSize as applySize } from "../lib/textsize";

interface Props {
  onClose: () => void;
  onSaved: (shortcutChanged: boolean) => void;
  onImport: (a: ImportAction) => void;
  initialTab?: Tab;
}

type Tab = "import" | "sync" | "sources" | "view" | "shortcuts" | "about";

const field = "w-full rounded border border-zinc-200 px-3 py-2 text-sm outline-none focus:border-amber-400";
const label = "mb-1 block text-xs font-semibold uppercase tracking-wide text-zinc-500";

export function SettingsPanel({ onClose, onSaved, onImport, initialTab }: Props) {
  const [tab, setTab] = useState<Tab>(initialTab ?? "import");
  const [settings, setSettings] = useState<Settings | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const [textSize, setTextSizeState] = useState(getTextSize);

  useEffect(() => {
    getSettings().then(setSettings).catch((e) => setError(String(e)));
  }, []);

  const update = (patch: Partial<Settings>) => setSettings((s) => (s ? { ...s, ...patch } : s));

  const save = async () => {
    if (!settings) return;
    setSaving(true);
    setError("");
    try {
      const result = await saveSettings(settings);
      if (settings) setAutostart(settings.autostart_enabled).catch(() => {});
      onSaved(Boolean(result));
    } catch (e) {
      setError(String(e));
      setSaving(false);
    }
  };

  const tabs: Array<{ id: Tab; label: string }> = [
    { id: "import", label: "Import" },
    { id: "sync", label: "Sync" },
    { id: "sources", label: "Sources" },
    { id: "view", label: "Search & view" },
    { id: "shortcuts", label: "Shortcuts" },
    { id: "about", label: "About" },
  ];

  return (
    <Overlay title="Settings" onClose={onClose} wide>
      <div className="mb-3 flex gap-1 border-b border-zinc-100">
        {tabs.map((t) => (
          <button
            key={t.id}
            onClick={() => setTab(t.id)}
            className={`px-3 py-1.5 text-sm ${tab === t.id ? "border-b-2 border-amber-400 font-medium text-zinc-800" : "text-zinc-500 hover:text-zinc-700"}`}
          >
            {t.label}
          </button>
        ))}
        <span className="ml-auto self-center text-xs text-zinc-300">v{APP_VERSION}</span>
      </div>

      {!settings && tab !== "shortcuts" && tab !== "about" && tab !== "import" ? (
        <p className="p-3 text-sm text-zinc-400">{error || "Loading…"}</p>
      ) : (
        <div className="flex max-h-[60vh] flex-col gap-3 overflow-y-auto">
          {tab === "import" && (
            <>
              <p className="text-xs text-zinc-400">Bring in highlights from a file or a connected source. CSV/Kindle/JSON need no account.</p>
              <ImportButtons onPick={onImport} />
              {settings && (
                <div className="mt-2 border-t border-zinc-100 pt-3">
                  <label className={label}>Remind me to import after (days)</label>
                  <input
                    type="number"
                    min={0}
                    max={365}
                    className={field}
                    value={settings.import_reminder_days}
                    onChange={(e) => update({ import_reminder_days: Number(e.target.value) || 0 })}
                  />
                  <p className="mt-1 text-xs text-zinc-400">0 = off. Nudges you on launch when it's been this long since your last import.</p>
                </div>
              )}
            </>
          )}
          {tab === "sync" && settings && (
            <>
              <p className="text-xs text-zinc-400">Run imports automatically while Highlight Scout is open. Enable "Launch at login" to keep it running.</p>
              {([
                ["Readwise highlights", "readwise_sync_enabled", "readwise_sync_interval_hours"],
                ["Readwise saved tweets", "readwise_tweets_sync_enabled", "readwise_tweets_sync_interval_hours"],
                ["Zotero", "zotero_sync_enabled", "zotero_sync_interval_hours"],
              ] as const).map(([name, enKey, ivKey]) => (
                <div key={enKey} className="flex items-center justify-between gap-2 border-b border-zinc-100 py-2">
                  <label className="flex items-center gap-2 text-sm text-zinc-700">
                    <input type="checkbox" checked={Boolean(settings[enKey])} onChange={(e) => update({ [enKey]: e.target.checked } as Partial<Settings>)} />
                    {name}
                  </label>
                  <select className="rounded border border-zinc-200 px-2 py-1 text-xs" value={Number(settings[ivKey]) || 0}
                    onChange={(e) => update({ [ivKey]: Number(e.target.value) } as Partial<Settings>)}>
                    <option value={0}>Off</option>
                    <option value={1}>Hourly</option>
                    <option value={6}>Every 6 hours</option>
                    <option value={24}>Daily</option>
                  </select>
                </div>
              ))}
              <label className="mt-2 flex items-center gap-2 text-sm text-zinc-700">
                <input type="checkbox" checked={settings.autostart_enabled}
                  onChange={(e) => update({ autostart_enabled: e.target.checked })} />
                Launch at login (enables background syncs)
              </label>
              <label className="mt-2 flex items-center gap-2 text-sm text-zinc-700">
                <input type="checkbox" checked={settings.ocr_on_import}
                  onChange={(e) => update({ ocr_on_import: e.target.checked })} />
                OCR image highlights after import (macOS only)
              </label>
            </>
          )}

          {tab === "sources" && settings && (
            <>
              <div>
                <label className={label}>Readwise API key</label>
                <input type="password" className={field} value={settings.readwise_api_key} onChange={(e) => update({ readwise_api_key: e.target.value })} placeholder="from readwise.io/access_token" />
              </div>
              <div>
                <label className={label}>Readwise archive (to seed from)</label>
                <input className={field} value={settings.readwise_archive_path} onChange={(e) => update({ readwise_archive_path: e.target.value })} />
              </div>
              <div>
                <label className={label}>Zotero database path</label>
                <input className={field} value={settings.zotero_db_path} onChange={(e) => update({ zotero_db_path: e.target.value })} />
              </div>
              <div>
                <label className={label}>Archive output path</label>
                <input className={field} value={settings.archive_path} onChange={(e) => update({ archive_path: e.target.value })} />
              </div>
            </>
          )}

          {tab === "view" && settings && (
            <>
              <div>
                <label className={label}>Text size</label>
                <select
                  className={field}
                  value={textSize}
                  onChange={(e) => { const v = Number(e.target.value); setTextSizeState(v); applySize(v); /* saves + applies */ }}
                >
                  {TEXT_SIZES.map((t) => (
                    <option key={t.value} value={t.value}>{t.label} ({t.value}%)</option>
                  ))}
                </select>
                <p className="mt-1 text-xs text-zinc-400">Scales all text in the app; applies immediately.</p>
              </div>
              <div className="w-32">
                <label className={label}>Result limit</label>
                <input type="number" min={1} max={300} className={field} value={settings.result_limit} onChange={(e) => update({ result_limit: Number(e.target.value) || 80 })} />
              </div>
              <div>
                <label className={label}>Global hotkey (show/hide window)</label>
                <input className={field} value={settings.shortcut} onChange={(e) => update({ shortcut: e.target.value })} placeholder="CmdOrCtrl+Shift+H" />
                <p className="mt-1 text-xs text-zinc-400">Takes effect after restart.</p>
              </div>
            </>
          )}

          {tab === "shortcuts" && <ShortcutsEditor />}

          {tab === "about" && (
            <div className="flex flex-col gap-3 text-sm">
              <p className="text-zinc-600">Highlight Scout v{APP_VERSION}</p>
              {RELEASE_NOTES.map((r) => (
                <div key={r.version}>
                  <p className="font-medium text-zinc-600">v{r.version}</p>
                  <ul className="ml-4 list-disc text-xs text-zinc-500">
                    {r.notes.map((n, i) => <li key={i}>{n}</li>)}
                  </ul>
                </div>
              ))}
            </div>
          )}

          {error && <p className="text-xs text-red-500">{error}</p>}
        </div>
      )}

      {(tab === "sources" || tab === "view" || tab === "sync") && (
        <div className="mt-3 flex justify-end gap-2 border-t border-zinc-100 pt-3">
          <button onClick={onClose} className="rounded px-3 py-1.5 text-sm text-zinc-500 hover:bg-zinc-100">Cancel</button>
          <button onClick={save} disabled={saving} className="rounded bg-amber-400 px-3 py-1.5 text-sm font-medium text-white hover:bg-amber-500 disabled:opacity-50">
            {saving ? "Saving…" : "Save"}
          </button>
        </div>
      )}
    </Overlay>
  );
}

function ShortcutsEditor() {
  const [bindings, setBindings] = useState(() => resolveBindings());
  const [recording, setRecording] = useState<CommandId | null>(null);

  const groups = useMemo(() => {
    const m = new Map<string, typeof COMMANDS>();
    for (const c of COMMANDS) {
      if (!m.has(c.group)) m.set(c.group, []);
      m.get(c.group)!.push(c);
    }
    return Array.from(m.entries());
  }, []);

  const onCapture = (id: CommandId, e: React.KeyboardEvent) => {
    e.preventDefault();
    if (e.key === "Escape") { setRecording(null); return; }
    const combo = eventToCombo(e);
    if (!combo) return; // modifier-only; keep listening
    setBinding(id, combo);
    setBindings(resolveBindings());
    setRecording(null);
  };

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <p className="text-xs text-zinc-400">Click a shortcut, then press the new key combination. Esc cancels.</p>
        <button
          onClick={() => { resetBindings(); setBindings(resolveBindings()); }}
          className="rounded bg-zinc-100 px-2 py-1 text-xs text-zinc-600 hover:bg-zinc-200"
        >
          Reset to defaults
        </button>
      </div>
      {groups.map(([group, cmds]) => (
        <div key={group}>
          <p className="mb-1 text-xs font-semibold uppercase tracking-wide text-zinc-400">{group}</p>
          <div className="flex flex-col">
            {cmds.map((c) => (
              <div key={c.id} className="flex items-center justify-between border-b border-zinc-50 py-1">
                <span className="text-sm text-zinc-600">{c.label}</span>
                <button
                  tabIndex={0}
                  onClick={() => setRecording(c.id)}
                  onKeyDown={(e) => recording === c.id && onCapture(c.id, e)}
                  className={`min-w-24 rounded px-2 py-1 text-center text-xs ${
                    recording === c.id ? "bg-amber-100 text-amber-700" : "bg-zinc-100 text-zinc-600 hover:bg-zinc-200"
                  }`}
                >
                  {recording === c.id ? "Press keys…" : comboLabel(bindings[c.id])}
                </button>
              </div>
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}
