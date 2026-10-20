import { emitTo } from "@tauri-apps/api/event";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";

/**
 * Open (or focus) a standalone window showing one work's highlights, so several
 * works can be lined up side by side. The label matches the `work-*` capability.
 */
export async function openWorkWindow(workId: string, title: string): Promise<void> {
  const label = `work-${workId.replace(/[^A-Za-z0-9_-]/g, "_")}`;

  const existing = await WebviewWindow.getByLabel(label);
  if (existing) {
    await existing.setFocus();
    return;
  }

  const win = new WebviewWindow(label, {
    url: `index.html?work=${encodeURIComponent(workId)}`,
    title: title || "Work",
    width: 520,
    height: 720,
    resizable: true,
  });
  win.once("tauri://error", (e) => console.error("work window error", e));
}

/** Open (or focus) a window showing highlights related to a source highlight. */
export async function openRelatedWindow(highlightId: string): Promise<void> {
  const label = `related-${highlightId.replace(/[^A-Za-z0-9_-]/g, "_")}`;
  const existing = await WebviewWindow.getByLabel(label);
  if (existing) {
    await existing.setFocus();
    return;
  }
  const win = new WebviewWindow(label, {
    url: `index.html?related=${encodeURIComponent(highlightId)}`,
    title: "Related highlights",
    width: 560,
    height: 760,
    resizable: true,
  });
  win.once("tauri://error", (e) => console.error("related window error", e));
}

/** Open (or focus) the Settings window, optionally on one section or page. */
export async function openSettingsWindow(section?: string, action?: "import"): Promise<void> {
  const existing = await WebviewWindow.getByLabel("settings");
  if (existing) {
    await existing.show();
    await existing.setFocus();
    if (section) await emitTo("settings", "settings:section", { section, action });
    return;
  }
  const win = new WebviewWindow("settings", {
    url: section ? `settings.html?section=${encodeURIComponent(section)}${action ? `&action=${action}` : ""}` : "settings.html",
    title: "Settings",
    width: 940,
    height: 680,
    minWidth: 760,
    minHeight: 520,
    resizable: true,
    center: true,
  });
  win.once("tauri://error", (e) => console.error("settings window error", e));
}
