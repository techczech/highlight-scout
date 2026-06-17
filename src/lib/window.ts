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
