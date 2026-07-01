import { useEffect, useMemo, useState } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import {
  getSettings,
  r2BackupNow,
  r2RestoreNow,
  saveR2Credentials,
  saveSettings,
  setAutostart,
  testR2Connection,
} from "../lib/api";
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
  const [r2AccessKey, setR2AccessKey] = useState("");
  const [r2Secret, setR2Secret] = useState("");
  const [r2Status, setR2Status] = useState("");
  const [r2Busy, setR2Busy] = useState(false);

  useEffect(() => {
    getSettings().then(setSettings).catch((e) => setError(String(e)));
  }, []);

  const update = (patch: Partial<Settings>) => setSettings((s) => (s ? { ...s, ...patch } : s));

  const persistCurrentSettings = async () => {
    if (!settings) return false;
    await saveSettings(settings);
    return true;
  };

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

  const chooseArchiveFolder = async () => {
    const folder = await openDialog({ directory: true, multiple: false });
    if (typeof folder === "string") update({ archive_path: folder });
  };

  const runR2 = async (label: string, fn: () => Promise<{ message: string }>, refreshSettings = false) => {
    if (!settings) return;
    setR2Busy(true);
    setR2Status(`${label}…`);
    setError("");
    try {
      await persistCurrentSettings();
      const result = await fn();
      setR2Status(result.message);
      if (refreshSettings) {
        const next = await getSettings();
        setSettings(next);
      }
    } catch (e) {
      setR2Status(`Failed: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setR2Busy(false);
    }
  };

  const saveR2Keys = async () => {
    if (!r2AccessKey || !r2Secret) {
      setR2Status("Paste both the access key id and secret access key.");
      return;
    }
    await runR2(
      "Saving credentials",
      async () => {
        const result = await saveR2Credentials(r2AccessKey, r2Secret);
        setR2AccessKey("");
        setR2Secret("");
        return result;
      },
      true,
    );
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
                <label className={label}>Local highlights folder</label>
                <div className="flex gap-2">
                  <input className={field} value={settings.archive_path} onChange={(e) => update({ archive_path: e.target.value })} />
                  <button
                    type="button"
                    onClick={chooseArchiveFolder}
                    className="shrink-0 rounded border border-zinc-200 px-3 py-2 text-sm text-zinc-600 hover:bg-zinc-50"
                  >
                    Choose…
                  </button>
                </div>
                <p className="mt-1 text-xs text-zinc-400">This folder is the archive Highlight Scout loads and writes automatically.</p>
              </div>
              <div>
                <label className={label}>Zotero database path</label>
                <input className={field} value={settings.zotero_db_path} onChange={(e) => update({ zotero_db_path: e.target.value })} />
              </div>
              <div className="border-t border-zinc-100 pt-3">
                <label className="flex items-center gap-2 text-sm text-zinc-700">
                  <input type="checkbox" checked={settings.r2_enabled} onChange={(e) => update({ r2_enabled: e.target.checked })} />
                  Local + R2 backup
                </label>
                <p className="mt-1 text-xs text-zinc-400">The full local folder stays primary. R2 stores a portable backup of the archive and local search index.</p>
              </div>
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className={label}>Cloudflare account id</label>
                  <input className={field} value={settings.r2_account_id} onChange={(e) => update({ r2_account_id: e.target.value })} />
                </div>
                <div>
                  <label className={label}>Bucket</label>
                  <input className={field} value={settings.r2_bucket} onChange={(e) => update({ r2_bucket: e.target.value })} placeholder="highlight-scout" />
                </div>
              </div>
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className={label}>Key prefix</label>
                  <input className={field} value={settings.r2_prefix} onChange={(e) => update({ r2_prefix: e.target.value })} placeholder="highlight-scout" />
                </div>
                <div>
                  <label className={label}>Endpoint (optional)</label>
                  <input
                    className={field}
                    value={settings.r2_endpoint}
                    onChange={(e) => update({ r2_endpoint: e.target.value })}
                    placeholder={settings.r2_account_id ? `https://${settings.r2_account_id}.r2.cloudflarestorage.com` : "derived from account id"}
                  />
                </div>
              </div>
              <div>
                <label className={label}>S3 credentials{settings.r2_has_credentials ? " · saved" : ""}</label>
                <div className="grid grid-cols-2 gap-2">
                  <input className={field} value={r2AccessKey} onChange={(e) => setR2AccessKey(e.target.value)} placeholder={settings.r2_has_credentials ? "access key id saved" : "access key id"} />
                  <input type="password" className={field} value={r2Secret} onChange={(e) => setR2Secret(e.target.value)} placeholder="secret access key" />
                </div>
                <p className="mt-1 text-xs text-zinc-400">Write-only; saved in macOS Keychain. Leave blank to keep the saved credentials.</p>
              </div>
              <div className="flex flex-wrap items-center gap-2 border-t border-zinc-100 pt-3">
                <button type="button" disabled={r2Busy} onClick={saveR2Keys} className="rounded bg-zinc-100 px-3 py-1.5 text-sm text-zinc-600 hover:bg-zinc-200 disabled:opacity-50">
                  Save credentials
                </button>
                <button type="button" disabled={r2Busy} onClick={() => runR2("Testing R2", testR2Connection)} className="rounded bg-zinc-100 px-3 py-1.5 text-sm text-zinc-600 hover:bg-zinc-200 disabled:opacity-50">
                  Test connection
                </button>
                <button type="button" disabled={r2Busy || !settings.r2_enabled || !settings.r2_has_credentials} onClick={() => runR2("Backing up to R2", r2BackupNow)} className="rounded bg-amber-400 px-3 py-1.5 text-sm font-medium text-white hover:bg-amber-500 disabled:opacity-50">
                  Back up now
                </button>
                <button type="button" disabled={r2Busy || !settings.r2_enabled || !settings.r2_has_credentials} onClick={() => runR2("Restoring from R2", r2RestoreNow)} className="rounded border border-zinc-200 px-3 py-1.5 text-sm text-zinc-600 hover:bg-zinc-50 disabled:opacity-50">
                  Restore from R2
                </button>
                {r2Status && <p className="basis-full text-xs text-zinc-500">{r2Status}</p>}
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
