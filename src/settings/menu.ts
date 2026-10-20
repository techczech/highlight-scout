// The macOS menu bar, generated from the registry (buildMenuTemplate) and installed through Tauri's JS menu API.
// `planMenu` is pure (a template to Tauri item descriptions) so it is tested without a window.
import { buildMenuTemplate, type Bindings } from "@bridle/commands";
import { invoke } from "@tauri-apps/api/core";
import { Menu, MenuItem, PredefinedMenuItem, Submenu } from "@tauri-apps/api/menu";
import { MENUS, registry } from "./registry";

type Role = "about" | "hide" | "hideOthers" | "unhide" | "quit" | "undo" | "redo" | "cut" | "copy" | "paste" | "selectAll" | "togglefullscreen" | "minimize" | "zoom" | "close";
interface TemplateItem { label?: string; role?: Role; type?: "separator"; accelerator?: string; enabled?: boolean; click?: () => void; submenu?: TemplateItem[] }

/** An item that is not a registry command (the import log, which has no command id). */
export interface ExtraItem { label: string; click: () => void }

export type Planned =
  | { kind: "predefined"; item: string; text?: string }
  | { kind: "item"; id?: string; text: string; click: () => void; accelerator?: string; enabled: boolean }
  | { kind: "submenu"; text: string; items: Planned[] };

const PREDEFINED: Record<Role, string> = {
  about: "About", hide: "Hide", hideOthers: "HideOthers", unhide: "ShowAll", quit: "Quit", undo: "Undo", redo: "Redo", cut: "Cut",
  copy: "Copy", paste: "Paste", selectAll: "SelectAll", togglefullscreen: "Fullscreen", minimize: "Minimize", zoom: "Maximize", close: "CloseWindow",
};

/** The registry's menus with the effective chords. Item ids are command ids, so one `run(id)` serves them all. */
export function menuTemplate(appName: string, bindings: Bindings, run: (id: string) => void, appItems?: { help?: ExtraItem[] }): TemplateItem[] {
  return buildMenuTemplate({ appName, registry, menus: MENUS, bindings, run, ctx: undefined, ...(appItems ? { appItems } : {}) }) as TemplateItem[];
}

export function planMenu(template: TemplateItem[]): Planned[] {
  return template.map((t): Planned => {
    if (t.type === "separator") return { kind: "predefined", item: "Separator" };
    if (t.submenu) return { kind: "submenu", text: t.label ?? "", items: planMenu(t.submenu) };
    if (t.role) return { kind: "predefined", item: PREDEFINED[t.role] };
    const id = registry.defs.find((d) => d.label === t.label)?.id;
    return { kind: "item", ...(id ? { id } : {}), text: t.label ?? "", click: t.click ?? (() => {}), ...(t.accelerator ? { accelerator: t.accelerator } : {}), enabled: t.enabled !== false };
  });
}

async function build(p: Planned): Promise<MenuItem | Submenu | PredefinedMenuItem> {
  if (p.kind === "predefined") return PredefinedMenuItem.new({ item: p.item as never });
  if (p.kind === "item") return MenuItem.new({ ...(p.id ? { id: p.id } : {}), text: p.text, enabled: p.enabled, ...(p.accelerator ? { accelerator: p.accelerator } : {}), action: p.click });
  return Submenu.new({ text: p.text, items: await Promise.all(p.items.map((i) => build(i))) });
}

/** Installs (or replaces) the app menu. Call again when a chord changes. */
export async function installAppMenu(appName: string, bindings: Bindings, run: (id: string) => void, appItems?: { help?: ExtraItem[] }): Promise<void> {
  const plan = planMenu(menuTemplate(appName, bindings, run, appItems));
  const menu = await Menu.new({ items: await Promise.all(plan.map((p) => build(p))) });
  // The host sets it (the webviews may not call setAsAppMenu), so a menu installed while a shortcut is being recorded comes up with its
  // accelerators still suspended.
  await invoke("menu_install", { rid: menu.rid });
}
