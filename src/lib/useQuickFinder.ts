// Quick finder state around archive search: the rail's corpus selection,
// the result sort, the ⌘⇧C format setting, recent searches, and the copy
// actions (quote, quote + citation, link). The citation is the engine's; the
// only decision here is Markdown or rich text, from the app the user came from.
import { useCallback, useEffect, useState } from "react";
import { writeHtml, writeText } from "@tauri-apps/plugin-clipboard-manager";
import { frontmostOtherApp, quickFinderHide, type PassageView } from "./archive";
import {
  ALL,
  appName,
  copyLinkFor,
  loadRecent,
  pushRecent,
  resolveCopyFormat,
  saveRecent,
  type CopyFormat,
  type CorpusFilter,
  type ResultSort,
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
  const [filter, setFilter] = useState<CorpusFilter>(ALL);
  const [sort, setSort] = useState<ResultSort>(() => loadPref("quickFinder.sort", "best", ["best", "newest", "oldest"]));
  const [format, setFormat] = useState<CopyFormat>(() => loadPref("quickFinder.copyFormat", "auto", ["auto", "markdown", "rich"]));
  const [recent, setRecent] = useState<string[]>(() => loadRecent());
  useEffect(() => savePref("quickFinder.sort", sort), [sort]);
  useEffect(() => savePref("quickFinder.copyFormat", format), [format]);
  const remember = useCallback((q: string) => {
    setRecent((list) => {
      const next = pushRecent(list, q);
      if (next !== list) saveRecent(next);
      return next;
    });
  }, []);
  return { filter, setFilter, sort, setSort, format, setFormat, recent, remember };
}

export type QuickFinderPrefs = ReturnType<typeof useQuickFinderPrefs>;

/** Copy actions on the selected passage, with the in-row confirmation. */
export function useQuickFinderCopy(opts: {
  passage: PassageView | null;
  activeKey: string | null;
  query: string;
  format: CopyFormat;
  remember: (q: string) => void;
  onToast: (m: string) => void;
}) {
  const { passage, activeKey, query, format, remember, onToast } = opts;
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
    if (!passage) return;
    try {
      await writeText(passage.cited.quote);
      await done("quote");
    } catch { onToast("Couldn't copy"); }
  }, [passage, done, onToast]);

  const copyQuoteCitation = useCallback(async () => {
    if (!passage) return;
    try {
      const front = format === "auto" ? await frontmostOtherApp().catch(() => null) : null;
      const how = resolveCopyFormat(format, front);
      const c = passage.cited.citation;
      if (how === "markdown") await writeText(c.markdown.trimEnd());
      else await writeHtml(passage.html, c.plain.trimEnd());
      await done("citation");
    } catch { onToast("Couldn't copy"); }
  }, [passage, format, done, onToast]);

  const copyLink = useCallback(async () => {
    const link = passage ? copyLinkFor(passage.cited) : null;
    if (!link) { onToast("No link for this passage"); return; }
    try {
      await writeText(link);
      await done("link");
    } catch { onToast("Couldn't copy"); }
  }, [passage, done, onToast]);

  const hide = useCallback(() => {
    remember(query);
    quickFinderHide().catch(() => {});
  }, [query, remember]);

  return { copied: shown, backTo, copyQuote, copyQuoteCitation, copyLink, hide };
}
