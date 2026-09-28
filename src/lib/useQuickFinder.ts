// Quick finder state around the one search: the ⌘⇧C format setting, recent
// searches, and the copy actions (quote, quote + citation, link) on the
// selected result of either engine. A corpus-engine citation is the engine's;
// a highlight's is Classic's Markdown quote. The only decision here is
// Markdown or rich text for the plain flavour, from the app the user came
// from; the HTML flavour is always on the clipboard too.
import { useCallback, useEffect, useState } from "react";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { copyCitation } from "./clipboard";
import { frontmostOtherApp, quickFinderHide, type PassageView } from "./archive";
import { highlightFlavours, toPlainText } from "./copyFormats";
import { originalUrl } from "./format";
import type { SearchResult } from "../types";
import {
  appName,
  citationFlavours,
  copyLinkFor,
  loadRecent,
  pushRecent,
  resolveCopyFormat,
  saveRecent,
  type CopyFormat,
} from "./quickFinder";
import type { CopiedWhat } from "../components/quickfinder/GroupedResults";

function loadPref<T extends string>(key: string, fallback: T, valid: T[]): T {
  try {
    const v = localStorage.getItem(key);
    return v && (valid as string[]).includes(v) ? (v as T) : fallback;
  } catch {
    return fallback;
  }
}

function savePref(key: string, v: string) {
  try { localStorage.setItem(key, v); } catch { /* a convenience only */ }
}

export function useQuickFinderPrefs() {
  const [format, setFormat] = useState<CopyFormat>(() => loadPref("quickFinder.copyFormat", "auto", ["auto", "markdown", "rich"]));
  const [recent, setRecent] = useState<string[]>(() => loadRecent());
  useEffect(() => savePref("quickFinder.copyFormat", format), [format]);
  const remember = useCallback((q: string) => {
    setRecent((list) => {
      const next = pushRecent(list, q);
      if (next !== list) saveRecent(next);
      return next;
    });
  }, []);
  return { format, setFormat, recent, remember };
}

export type QuickFinderPrefs = ReturnType<typeof useQuickFinderPrefs>;

/** Copy actions on the selected passage or highlight, with the in-row confirmation. */
export function useQuickFinderCopy(opts: {
  passage: PassageView | null;
  /** The selected highlight when the highlight index answered; wins over `passage`. */
  row: SearchResult | null;
  activeKey: string | null;
  query: string;
  format: CopyFormat;
  remember: (q: string) => void;
  onToast: (m: string) => void;
}) {
  const { passage, row, activeKey, query, format, remember, onToast } = opts;
  const [copied, setCopied] = useState<{ what: CopiedWhat; key: string | null }>({ what: null, key: null });
  const [backTo, setBackTo] = useState<string | null>(null);

  // The confirmation belongs to the row it was made on, and fades.
  useEffect(() => {
    if (!copied.what) return;
    const t = setTimeout(() => setCopied({ what: null, key: null }), 5000);
    return () => clearTimeout(t);
  }, [copied]);
  const shown: CopiedWhat = copied.key === activeKey ? copied.what : null;

  const done = useCallback(async (what: Exclude<CopiedWhat, null>) => {
    setCopied({ what, key: activeKey });
    remember(query);
    const front = await frontmostOtherApp().catch(() => null);
    setBackTo(appName(front));
  }, [activeKey, query, remember]);

  const copyQuote = useCallback(async () => {
    const text = row ? toPlainText(row) : passage?.cited.quote;
    if (!text) return;
    try {
      await writeText(text);
      await done("quote");
    } catch { onToast("Couldn't copy"); }
  }, [row, passage, done, onToast]);

  const copyQuoteCitation = useCallback(async () => {
    if (!row && !passage) return;
    try {
      const front = format === "auto" ? await frontmostOtherApp().catch(() => null) : null;
      const how = resolveCopyFormat(format, front);
      await copyCitation(row ? highlightFlavours(row, how) : citationFlavours(passage!, how));
      await done("citation");
    } catch { onToast("Couldn't copy"); }
  }, [row, passage, format, done, onToast]);

  const copyLink = useCallback(async () => {
    const link = row ? originalUrl(row) ?? row.zotero_link ?? null : passage ? copyLinkFor(passage.cited) : null;
    if (!link) { onToast("No link for this passage"); return; }
    try {
      await writeText(link);
      await done("link");
    } catch { onToast("Couldn't copy"); }
  }, [row, passage, done, onToast]);

  const hide = useCallback(() => {
    remember(query);
    quickFinderHide().catch(() => {});
  }, [query, remember]);

  return { copied: shown, backTo, copyQuote, copyQuoteCitation, copyLink, hide };
}
