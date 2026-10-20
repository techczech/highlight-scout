// The Settings window: the shared @bridle/settings-react window over this app's schema and registry. The view comes from
// settings_view and is replaced by every settings:changed event, so a change made by the CLI shows at once.
import { useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { SettingsWindow, type CustomPage, type RowHint, type SheetState } from "@bridle/settings-react";
import { MENUS, registry, schema } from "./registry";
import { actions, loadView, onSettingsChanged, type SettingsView } from "./host";
import { applyTextSize, textSizeOf } from "./appearance";
import { SyncBackupPage } from "./SyncBackupPage";
import { createSuspendController, isRecorder } from "./recording";

const ROW_HINTS: Record<string, RowHint> = {
  "appearance.textSize": { control: "segmented", summary: "Scales all text in the app", example: "120" },
  "sync.intervalHours": { control: "segmented", summary: "How often a background sync runs", example: "6" },
  "sync.onLaunch": { summary: "Readwise, saved tweets and Zotero", example: "on" },
  "search.resultLimit": { summary: "How many results one search shows", example: "80" },
  "sources.archiveFolder": { summary: "Where the Markdown files live" },
  "sources.zoteroDatabase": { summary: "Read only" },
  "backup.r2Endpoint": { summary: "Leave empty to use the account id" },
};

const SEEN_KEY = "settingsChangesSeenAt";

/** While a shortcut is being recorded here the host suspends the app menu's accelerators (only this window may ask), so the key press
 *  reaches the recorder. Driven by the recorder's presence in the page (a MutationObserver: it lives inside the settings package), so
 *  recording a chord or Escape, which remove it without a blur, restore them; so do the window losing focus and the page hiding. */
function useMenuSuspendWhileRecording(): void {
  useEffect(() => {
    const c = createSuspendController((v) => void invoke(v ? "menu_shortcuts_suspend" : "menu_shortcuts_restore").catch(() => {}));
    let focused = document.hasFocus();
    const check = () => c.update({
      recorder: Array.from(document.querySelectorAll("button")).some((b) => isRecorder(b)),
      focused,
      visible: document.visibilityState === "visible",
    });
    const obs = new MutationObserver(check);
    obs.observe(document.body, { childList: true, subtree: true, attributes: true, attributeFilter: ["aria-label"] });
    const onFocus = () => { focused = true; check(); };
    const onBlur = () => { focused = false; check(); };
    const hide = () => c.end();
    window.addEventListener("focus", onFocus);
    window.addEventListener("blur", onBlur);
    document.addEventListener("visibilitychange", check);
    window.addEventListener("pagehide", hide);
    check();
    return () => {
      obs.disconnect();
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("blur", onBlur);
      document.removeEventListener("visibilitychange", check);
      window.removeEventListener("pagehide", hide);
      c.end();
    };
  }, []);
}

export function SettingsApp({ initialSection, initialAction }: { initialSection?: string; initialAction?: string }) {
  const [view, setView] = useState<SettingsView | null>(null);
  const [opened, setOpened] = useState<{ section: string; sheet?: SheetState; n: number }>({ section: initialSection ?? "general", n: 0 });
  const [seenAt, setSeenAt] = useState(() => Number(localStorage.getItem(SEEN_KEY)) || Date.now());
  useMenuSuspendWhileRecording();

  useEffect(() => {
    // Import Settings… from the menu: pick the file here, then open Changes with the preview sheet.
    const open = async (section: string, action?: string) => {
      let sheet: SheetState | undefined;
      if (action === "import") {
        const picked = await actions.importPick();
        if (picked) sheet = { kind: picked.preview.refused ? "refused" : "import", fileName: picked.fileName, preview: picked.preview };
      }
      setOpened((o) => ({ section, ...(sheet ? { sheet } : {}), n: o.n + 1 }));
    };
    loadView().then((v) => { setView(v); if (initialAction) void open(initialSection ?? "general", initialAction); }).catch(() => {});
    const a = onSettingsChanged(setView);
    const b = listen<{ section: string; action?: string }>("settings:section", (e) => void open(e.payload.section, e.payload.action));
    return () => { a.then((f) => f()); b.then((f) => f()); };
  }, []);

  useEffect(() => { if (view) applyTextSize(textSizeOf(view.state.settings)); }, [view]);

  const pages = useMemo<CustomPage[]>(
    () => [{ id: "sync-backup", label: "Sync and backup", icon: "refresh-cw", render: () => (view ? <SyncBackupPage view={view} /> : null) }],
    [view],
  );

  if (!view) return null;
  return (
    <SettingsWindow
      key={opened.n}
      className="hs-settings"
      schema={schema}
      state={view.state}
      registry={registry}
      menus={MENUS}
      appInfo={view.appInfo}
      actions={actions}
      slots={view.slots}
      rowHints={ROW_HINTS}
      customPages={pages}
      changesSeenAt={seenAt}
      onChangesSeen={(t) => { localStorage.setItem(SEEN_KEY, String(t)); setSeenAt(t); }}
      initial={{ section: opened.section, ...(opened.sheet ? { sheet: opened.sheet } : {}) }}
    />
  );
}
