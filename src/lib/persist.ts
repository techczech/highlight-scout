// Lightweight persistence for view preferences (localStorage). Sort and group
// persist between launches; scope persists only briefly so stale filters don't
// linger (mirrors the Raycast extension's 60s scope window).

const SCOPE_TTL_MS = 60_000;

export function load<T extends string>(key: string, fallback: T, valid: T[]): T {
  const v = localStorage.getItem(key);
  return v && (valid as string[]).includes(v) ? (v as T) : fallback;
}

export function save(key: string, value: string): void {
  localStorage.setItem(key, value);
}

export function loadScope(): string {
  const touched = Number(localStorage.getItem("scopeTouched") || "0");
  if (!Number.isFinite(touched) || Date.now() - touched > SCOPE_TTL_MS) return "";
  return localStorage.getItem("scope") || "";
}

export function saveScope(scope: string): void {
  localStorage.setItem("scope", scope);
  localStorage.setItem("scopeTouched", String(Date.now()));
}

export function touchScope(): void {
  localStorage.setItem("scopeTouched", String(Date.now()));
}
