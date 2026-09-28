// Which search the window opens in, remembered across launches (localStorage).
// Kept apart from persist.ts so it has no @scout/query import and tests run in node.

/** The storage the preferences live in; localStorage in the app. */
export type PrefStore = Pick<Storage, "getItem" | "setItem">;

/** Which search the window opens in: the three-corpus quick finder or the classic highlight search. */
export type Scope = "archive" | "highlights";

/**
 * The last-used scope, remembered across launches. A new key: 0.6.0-preview.1
 * wrote "scope" = "highlights" on every launch whether or not it was chosen,
 * so that value is not a choice and the new default (the quick finder) wins.
 */
const SCOPE_KEY = "searchScope";
export const DEFAULT_SCOPE: Scope = "archive";

export function loadScope(store: PrefStore = localStorage): Scope {
  const v = store.getItem(SCOPE_KEY);
  return v === "archive" || v === "highlights" ? v : DEFAULT_SCOPE;
}

export function saveScope(scope: Scope, store: PrefStore = localStorage): void {
  store.setItem(SCOPE_KEY, scope);
}
