import { writeHtml, writeText } from "@tauri-apps/plugin-clipboard-manager";
import { invoke } from "@tauri-apps/api/core";

/** Copy plain text to the clipboard. */
export async function copyText(text: string): Promise<void> {
  await writeText(text);
}

/** Copy rich text (HTML) — pastes as formatted content into Word/Pages/Gmail. */
export async function copyHtml(html: string): Promise<void> {
  await writeHtml(html);
}

/** Copy an image (by remote URL or local path) to the clipboard as a bitmap. */
export async function copyImage(source: string): Promise<void> {
  await invoke("copy_image", { source });
}
