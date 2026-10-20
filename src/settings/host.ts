// The Settings window's host: the settings_* commands and the settings:changed event behind the actions the shared
// window expects. A key's value goes out once (settings_set_secret) and never comes back.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type { SettingsActions, Slot, AppInfo, Outcome } from "@bridle/settings-react";
import type { ImportPreview, SettingsState } from "@bridle/settings";

/** What settings_view returns and settings:changed carries. */
export interface SettingsView {
  state: SettingsState;
  slots: Slot[];
  appInfo: AppInfo;
  problem?: { code: string; error: string };
}

export const loadView = () => invoke<SettingsView>("settings_view");

export function onSettingsChanged(cb: (v: SettingsView) => void): Promise<UnlistenFn> {
  return listen<SettingsView>("settings:changed", (e) => cb(e.payload));
}

const out = (r: unknown): Outcome => r as Outcome;

/** The import the window is showing (its id in the app); importApply sends only this. */
let pendingPreviewId: string | null = null;

export const actions: SettingsActions = {
  set: async (key, value) => out(await invoke("settings_set", { key, value })),
  rebind: async (commandId, chord) => out(await invoke("settings_rebind", { commandId, chord })),
  rebindMany: async (pairs) => out(await invoke("settings_rebind_many", { pairs: pairs.map((p) => [p.commandId, p.chord]) })),
  reset: async (target) => out(await invoke("settings_reset", { target })),
  resetSection: async (section) => out(await invoke("settings_reset_section", { section })),
  resetAll: async () => out(await invoke("settings_reset_all")),
  resetEntry: async (entryId) => out(await invoke("settings_reset_entry", { entryId })),
  undoBatch: async (batchId) => out(await invoke("settings_undo_batch", { batchId })),
  // Export and import open their native panels in the app (Rust): the window never names a file. Apply names the preview the
  // window showed; the app reads and previews the file again before applying it.
  exportSettings: async () => out(await invoke("settings_export")),
  importPick: async () => {
    const r = await invoke<{ previewId: string; fileName: string; preview: ImportPreview } | { ok: false; error: string } | null>("settings_import_preview");
    if (!r) return null;
    if ("ok" in r) throw new Error(r.error);
    pendingPreviewId = r.previewId;
    return { fileName: r.fileName, preview: r.preview };
  },
  importApply: async (_preview, fileName) => {
    const previewId = pendingPreviewId;
    pendingPreviewId = null;
    return out(await invoke("settings_import_apply", { previewId: previewId ?? "", fileName }));
  },
  setSecret: async (slotId, value) => out(await invoke("settings_set_secret", { slotId, value })),
  removeSecret: async (slotId) => out(await invoke("settings_remove_secret", { slotId })),
  choosePath: async (key) => {
    const directory = key === "sources.archiveFolder";
    const p = await openDialog(directory ? { directory: true, multiple: false } : { multiple: false });
    return typeof p === "string" ? p : null;
  },
};
