import { writeText } from "@tauri-apps/plugin-clipboard-manager";

/** Copy text to the clipboard via the Tauri clipboard plugin. */
export async function copyText(text: string): Promise<void> {
  await writeText(text);
}
