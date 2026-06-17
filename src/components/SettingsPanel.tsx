import { useEffect, useState } from "react";
import { getSettings, saveSettings } from "../lib/api";
import type { Settings } from "../types";
import { Overlay } from "./TagPicker";
import { APP_VERSION, RELEASE_NOTES } from "../version";

interface Props {
  onClose: () => void;
  onSaved: (shortcutChanged: boolean) => void;
}

const field = "w-full rounded border border-zinc-200 px-3 py-2 text-sm outline-none focus:border-amber-400";
const label = "mb-1 block text-xs font-semibold uppercase tracking-wide text-zinc-500";

export function SettingsPanel({ onClose, onSaved }: Props) {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    getSettings().then(setSettings).catch((e) => setError(String(e)));
  }, []);

  const update = (patch: Partial<Settings>) =>
    setSettings((s) => (s ? { ...s, ...patch } : s));

  const save = async () => {
    if (!settings) return;
    setSaving(true);
    setError("");
    try {
      const result = await saveSettings(settings);
      onSaved(Boolean(result));
    } catch (e) {
      setError(String(e));
      setSaving(false);
    }
  };

  return (
    <Overlay title={`Settings · v${APP_VERSION}`} onClose={onClose}>
      {!settings ? (
        <p className="p-3 text-sm text-zinc-400">{error || "Loading…"}</p>
      ) : (
        <div className="flex flex-col gap-3 overflow-y-auto">
          <div>
            <label className={label}>Readwise API key</label>
            <input
              type="password"
              className={field}
              value={settings.readwise_api_key}
              onChange={(e) => update({ readwise_api_key: e.target.value })}
              placeholder="from readwise.io/access_token"
            />
          </div>
          <div>
            <label className={label}>Archive path</label>
            <input className={field} value={settings.archive_path} onChange={(e) => update({ archive_path: e.target.value })} />
          </div>
          <div>
            <label className={label}>Zotero database path</label>
            <input className={field} value={settings.zotero_db_path} onChange={(e) => update({ zotero_db_path: e.target.value })} />
          </div>
          <div>
            <label className={label}>Readwise archive (to seed from)</label>
            <input className={field} value={settings.readwise_archive_path} onChange={(e) => update({ readwise_archive_path: e.target.value })} />
          </div>
          <div className="flex gap-3">
            <div className="flex-1">
              <label className={label}>Global shortcut</label>
              <input className={field} value={settings.shortcut} onChange={(e) => update({ shortcut: e.target.value })} placeholder="CmdOrCtrl+Shift+H" />
            </div>
            <div className="w-28">
              <label className={label}>Result limit</label>
              <input
                type="number"
                min={1}
                max={300}
                className={field}
                value={settings.result_limit}
                onChange={(e) => update({ result_limit: Number(e.target.value) || 80 })}
              />
            </div>
          </div>
          <p className="text-xs text-zinc-400">Changing the shortcut takes effect after restart.</p>

          <details className="rounded border border-zinc-100 bg-zinc-50 p-2 text-xs">
            <summary className="cursor-pointer font-semibold text-zinc-500">Release notes</summary>
            <div className="mt-2 flex flex-col gap-2">
              {RELEASE_NOTES.map((r) => (
                <div key={r.version}>
                  <p className="font-medium text-zinc-600">v{r.version}</p>
                  <ul className="ml-4 list-disc text-zinc-500">
                    {r.notes.map((n, i) => (
                      <li key={i}>{n}</li>
                    ))}
                  </ul>
                </div>
              ))}
            </div>
          </details>

          {error && <p className="text-xs text-red-500">{error}</p>}
          <div className="flex justify-end gap-2">
            <button onClick={onClose} className="rounded px-3 py-1.5 text-sm text-zinc-500 hover:bg-zinc-100">Cancel</button>
            <button onClick={save} disabled={saving} className="rounded bg-amber-400 px-3 py-1.5 text-sm font-medium text-white hover:bg-amber-500 disabled:opacity-50">
              {saving ? "Saving…" : "Save"}
            </button>
          </div>
        </div>
      )}
    </Overlay>
  );
}
