import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { openUrl, openPath } from "@tauri-apps/plugin-opener";
import { WorkView } from "./components/WorkView";
import { SettingsPanel } from "./components/SettingsPanel";
import { CommandPalette } from "./components/CommandPalette";
import { ImportLogPanel } from "./components/ImportLogPanel";
import { CsvMappingPanel } from "./components/CsvMappingPanel";
import type { ImportAction } from "./components/ImportMenu";
import { QuickFinder } from "./components/quickfinder/QuickFinder";
import { GroupedResults } from "./components/quickfinder/GroupedResults";
import { HighlightResults } from "./components/quickfinder/HighlightResults";
import { HighlightPane } from "./components/quickfinder/HighlightPane";
import { FiltersPopover } from "./components/quickfinder/FiltersPopover";
import { Pane } from "./components/quickfinder/Pane";
import { Welcome } from "./components/quickfinder/Welcome";
import { Icon } from "./components/quickfinder/icons";
import "./components/quickfinder/quickfinder.css";
import { ARCHIVE_LIMIT, useArchiveSearch } from "./lib/useArchiveSearch";
import { useHighlightSearch } from "./lib/useHighlightSearch";
import { useQuickFinderCopy, useQuickFinderPrefs } from "./lib/useQuickFinder";
import { useRailCounts } from "./lib/useRailCounts";
import { railCountJob, tickedHighlightTotal } from "./lib/railCounts";
import { SOURCE_LABEL, countsLine, sourceOrder, type ArchiveGroupBy } from "./lib/quickFinder";
import {
  GROUP_LABEL,
  archiveRequests,
  chips as chipsOf,
  clearAll,
  clearChip,
  cycleDensity,
  cycleGroup,
  cycleSort,
  effectiveCorpora,
  effectiveGroup,
  effectiveSubgroup,
  engineFor,
  highlightSearchable,
  loadView,
  onlyCorpus,
  saveView,
  scopeNote,
  scopeWords,
  setMode,
  sourceTicked,
  tickedSources,
  toggleCorpus,
  toggleSource,
  withTag,
  type SearchState,
} from "./lib/searchModel";
import { jobVisible } from "./lib/archive";
import { meaningNotice } from "./lib/meaning";
import { useMeaning } from "./lib/useMeaning";
import { MeaningBanner } from "./components/quickfinder/MeaningBanner";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import {
  ocrImages,
  runImport,
  importReadwiseTweets,
  runZoteroImport,
  importKindle,
  importJson,
  importX,
  exportJson,
  getStats,
  getFacets,
  getConfig,
  getSettings,
  getImportLog,
  listTags,
  syncNow,
  getSyncStatus,
} from "./lib/api";
import { failureLines, lastSyncedLines } from "./lib/sync";
import { parseSearch } from "@scout/query";
import { copyHtml, copyImage, copyText } from "./lib/clipboard";
import { imageText, imageSources, toHtml, toPlainText } from "./lib/copyFormats";
import { openWorkWindow, openRelatedWindow } from "./lib/window";
import { workMarkdownPath } from "./lib/format";
import { comboMap, eventToCombo, type CommandId } from "./lib/keybindings";
import { APP_VERSION } from "./version";
import * as persist from "./lib/persist";
import type { Stats, Config, Facets, SearchResult, SyncStatus, TagCount } from "./types";

export default function App() {
  const [query, setQuery] = useState("");
  // The one search's choices: ticked corpora, Keyword / Semantic and every
  // Classic tool (searchModel.ts). Everything below derives from it.
  const [sq, setSq] = useState<SearchState>(() => loadView(localStorage, persist.loadFilters()));
  const [filtersOpen, setFiltersOpen] = useState(false);
  const [tagFocus, setTagFocus] = useState(0);
  const [tags, setTags] = useState<TagCount[]>([]);
  const [showPane, setShowPane] = useState(true);
  const qf = useQuickFinderPrefs();

  const [stats, setStats] = useState<Stats | null>(null);
  const [facets, setFacets] = useState<Facets | null>(null);
  const [config, setConfig] = useState<Config | null>(null);
  const [pageSize, setPageSize] = useState(80);

  const [importing, setImporting] = useState(false);
  const [status, setStatus] = useState("");
  const [progress, setProgress] = useState<{ current: number; total: number } | null>(null);
  const [toast, setToast] = useState("");
  const [toastTitle, setToastTitle] = useState("");
  const [toastDetails, setToastDetails] = useState(false);
  const toastTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [syncStatus, setSyncStatus] = useState<SyncStatus | null>(null);
  const seenSyncSeq = useRef(0);

  const [overlay, setOverlay] = useState<null | "settings" | "palette" | "importlog">(null);
  const [settingsTab, setSettingsTab] = useState<"import" | "sync" | undefined>(undefined);
  const [dataVersion, setDataVersion] = useState(0);
  const [workView, setWorkView] = useState<SearchResult | null>(null);
  const [csvPath, setCsvPath] = useState<string | null>(null);
  const [bindingsVersion, setBindingsVersion] = useState(0);

  const inputRef = useRef<HTMLInputElement>(null);

  // ---- what the choices mean: engine, groups, requests ----
  const engine = engineFor(sq, query);
  const eff = effectiveCorpora(sq, query);
  const group = effectiveGroup(sq, query);
  const subgroup = effectiveSubgroup(sq, query);
  const terms = useMemo(() => parseSearch(query).positive_terms, [query]);

  // The archive (not the classic index) knows the rail's sources and sizes.
  const [counts, setCounts] = useState<ReturnType<typeof useArchiveSearch>["counts"]>([]);
  const knownSources = useMemo(() => sourceOrder(counts.find((c) => c.corpus === "highlights")?.sources ?? {}), [counts]);
  const requests = useMemo(() => archiveRequests(sq, query, ARCHIVE_LIMIT, knownSources), [sq, query, knownSources]);
  const semantic = sq.mode === "semantic";
  const archive = useArchiveSearch(query, engine === "archive", requests, {
    by: (group === "tag" ? "none" : group) as ArchiveGroupBy,
    multi: eff.length > 1,
    sort: sq.sort,
  }, semantic);
  // The meaning index behind Semantic: its state and the one build button.
  const meaning = useMeaning(semantic);
  const notice = semantic ? meaningNotice(meaning.state, meaning.job, sq.corpora) : null;
  useEffect(() => setCounts(archive.counts), [archive.counts]);
  const hl = useHighlightSearch({ query, enabled: engine === "highlights", state: sq, group, subgroup, pageSize, knownSources, dataVersion });
  const onHighlights = engine === "highlights";
  // The rail's result counts for this query (sizes when there is none).
  const listed = useMemo(() => (counts.length ? counts.map((c) => c.corpus) : ["writing", "tweets", "highlights"]), [counts]);
  const countJob = useMemo(() => railCountJob(sq, query, listed, knownSources), [sq, query, listed, knownSources]);
  const countEpoch = useMemo(() => ({}), [dataVersion, archive.counts]);
  const railResults = useRailCounts(countJob, countEpoch);
  const activeRow = onHighlights ? hl.activeRow : null;

  const update = useCallback((next: SearchState) => setSq(next), []);

  const showToast = useCallback((msg: string, ms = 1800, title = "", details = false) => {
    if (toastTimer.current) clearTimeout(toastTimer.current);
    setToast(msg);
    setToastTitle(title);
    setToastDetails(details);
    toastTimer.current = setTimeout(() => { setToast(""); setToastTitle(""); setToastDetails(false); }, ms);
  }, []);

  const qfCopy = useQuickFinderCopy({
    passage: archive.passage,
    row: activeRow,
    activeKey: onHighlights ? hl.activeId : archive.activeKey,
    query,
    format: qf.format,
    remember: qf.remember,
    onToast: showToast,
  });

  const openSyncSettings = useCallback(() => {
    setSettingsTab("sync");
    setOverlay("settings");
  }, []);

  // Background sync (launch / every few hours / Sync now): keep the per-source
  // status for the red failure lines, and toast each new report's summary.
  const onSyncStatus = useCallback((st: SyncStatus) => {
    setSyncStatus(st);
    const r = st.last_report;
    if (!r || r.seq <= seenSyncSeq.current) return;
    seenSyncSeq.current = r.seq;
    if (r.results.length === 0 && r.trigger !== "manual") return;
    const lines = lastSyncedLines(st.sources);
    showToast(r.summary, 6000, lines.length ? `Last synced\n${lines.join("\n")}` : "", true);
  }, [showToast]);

  useEffect(() => {
    getSyncStatus().then(onSyncStatus).catch(() => {});
    const un = listen<SyncStatus>("sync:finished", (e) => onSyncStatus(e.payload));
    return () => { un.then((f) => f()); };
  }, [onSyncStatus]);

  const syncFailures = useMemo(() => failureLines(syncStatus?.sources ?? []), [syncStatus]);

  const refreshMeta = useCallback(() => {
    getStats().then(setStats).catch(() => {});
    getFacets().then(setFacets).catch(() => {});
  }, []);

  useEffect(() => {
    refreshMeta();
    getConfig().then(setConfig).catch(() => {});
    getSettings().then((s) => setPageSize(s.result_limit || 80)).catch(() => {});
  }, [refreshMeta]);

  // Optional import reminder (Settings → Import): nudge on launch if it's been
  // longer than `import_reminder_days` since the last import. 0 = off.
  useEffect(() => {
    getSettings()
      .then((s) => {
        const days = s.import_reminder_days || 0;
        if (days <= 0) return;
        getImportLog()
          .then((log) => {
            const last = log.reduce((mx, e) => Math.max(mx, Date.parse(e.timestamp) || 0), 0);
            if (Date.now() - last > days * 86_400_000) {
              setToast(`📌 It's been over ${days} day${days === 1 ? "" : "s"} since your last import — time to sync.`);
              window.setTimeout(() => setToast(""), 9000);
            }
          })
          .catch(() => {});
      })
      .catch(() => {});
  }, []);

  useEffect(() => saveView(sq, localStorage), [sq]);
  useEffect(() => { try { persist.saveFilters(sq.filters); } catch { /* a convenience only */ } }, [sq.filters]);

  // The tag field's list, when the popover opens.
  useEffect(() => {
    if (filtersOpen) listTags().then(setTags).catch(() => {});
  }, [filtersOpen]);

  // Refocus search box + auto-refresh counts when shown via the global hotkey.
  useEffect(() => {
    const un = getCurrentWindow().onFocusChanged(({ payload }) => {
      if (payload) {
        inputRef.current?.focus();
        inputRef.current?.select();
        refreshMeta();
      }
    });
    return () => { un.then((f) => f()); };
  }, []);

  // Import progress events (structured: message + current/total).
  useEffect(() => {
    const a = listen<{ message: string; current: number; total: number }>("import:progress", (e) => {
      setStatus(e.payload.message);
      setProgress(e.payload.total > 0 ? { current: e.payload.current, total: e.payload.total } : null);
    });
    const b = listen<{ message: string }>("import:complete", (e) => {
      setStatus(e.payload.message);
      setImporting(false);
      setProgress(null);
      refreshMeta();
      setDataVersion((v) => v + 1); // auto-refresh: re-run the current search
    });
    return () => { a.then((f) => f()); b.then((f) => f()); };
  }, [refreshMeta]);

  // A highlight-only tool used while Highlights is unticked ticks it first.
  const ensureHighlights = useCallback(() => {
    setSq((s) => (s.corpora.includes("highlights") ? s : toggleCorpus(s, "highlights")));
  }, []);

  const needsHighlight = (what: string) => {
    showToast(`${what} works on a highlight: tick Highlights alone, or use a highlight-only filter`, 3200);
  };

  const move = (delta: number) => (onHighlights ? hl.move(delta) : archive.move(delta));

  const copyHighlight = async () => { await qfCopy.copyQuote(); };
  const copyMarkdown = async () => { await qfCopy.copyQuoteCitation(); };
  const copyRichText = async () => {
    if (!activeRow) { needsHighlight("Copy as rich text"); return; }
    try { await copyHtml(toHtml(activeRow)); showToast("Copied as rich text"); }
    catch { await copyText(toPlainText(activeRow)); showToast("Copied as text"); }
  };
  const copyImageCmd = async () => {
    if (!activeRow) { needsHighlight("Copy image"); return; }
    const imgs = imageSources(activeRow);
    if (!imgs.length) { showToast("No image to copy"); return; }
    const src = imgs[0].path ?? imgs[0].url!;
    try { await copyImage(src); showToast(imgs.length > 1 ? `Copied image 1 of ${imgs.length}` : "Copied image"); }
    catch { showToast("Couldn't copy image"); }
  };
  const copyCitationCmd = async () => {
    if (!onHighlights) { await qfCopy.copyQuoteCitation(); return; }
    if (activeRow?.citation) { await copyText(activeRow.citation); showToast("Citation copied"); }
    else showToast("No citation for this highlight");
  };
  const copyImageTextCmd = async () => {
    if (!activeRow) { needsHighlight("Copy text from image"); return; }
    const t = imageText(activeRow);
    if (t) { await copyText(t); showToast("Copied image text"); }
    else showToast("No image text");
  };
  const openSource = () => {
    qf.remember(query);
    if (onHighlights) {
      if (activeRow?.zotero_link) openUrl(activeRow.zotero_link);
      else if (activeRow?.url) openUrl(activeRow.url);
      return;
    }
    const c = archive.cited;
    if (c?.public_url) openUrl(c.public_url);
    else if (c?.link?.startsWith("writeflex://")) openUrl(c.link).catch(() => showToast("Could not open WriteFlex"));
    else if (c) openPath(c.path).catch(() => showToast("Could not open the file"));
  };
  const openWorkMd = () => {
    if (!activeRow) { needsHighlight("Open work Markdown file"); return; }
    if (config) openPath(workMarkdownPath(config.archive_path, activeRow.slug)).catch(() => showToast("Markdown not found"));
  };
  const openWorkWin = (row: SearchResult | null = activeRow) => {
    if (!row) { needsHighlight("Open work in new window"); return; }
    openWorkWindow(row.work_id, row.title).catch(() => showToast("Could not open window"));
  };
  const findRelated = (row: SearchResult | null = activeRow) => {
    if (!row) { needsHighlight("Find related"); return; }
    openRelatedWindow(row.highlight_id).catch(() => showToast("Could not open window"));
  };
  const showWork = (row: SearchResult | null = activeRow) => {
    if (!row) { needsHighlight("Show work highlights"); return; }
    setWorkView(row);
  };

  const pickTag = (tag: string) => {
    setQuery((c) => withTag(c, tag));
    setFiltersOpen(false);
    inputRef.current?.focus();
  };

  const onMode = (m: SearchState["mode"]) => {
    setSq((s) => setMode(s, m));
    inputRef.current?.focus();
  };


  const doImport = async (which: ImportAction) => {
    // Non-import actions and file pickers first.
    if (which === "log") { setOverlay("importlog"); return; }
    if (which === "sync-all") {
      setImporting(true);
      setStatus("Syncing all sources…");
      try {
        const r = await syncNow();
        setStatus(r.summary);
      } catch (e) {
        setStatus(`Sync: ${e instanceof Error ? e.message : String(e)}`);
      } finally {
        setImporting(false);
        setProgress(null);
      }
      return;
    }
    if (which === "csv") {
      const f = await openDialog({ filters: [{ name: "CSV", extensions: ["csv", "tsv", "txt"] }] });
      if (typeof f === "string") setCsvPath(f);
      return;
    }
    if (which === "kindle") {
      const f = await openDialog({ filters: [{ name: "Kindle clippings", extensions: ["txt"] }] });
      if (typeof f !== "string") return;
      await withImport("Reading Kindle clippings…", () => importKindle(f));
      return;
    }
    if (which === "json") {
      const f = await openDialog({ filters: [{ name: "JSON", extensions: ["json"] }] });
      if (typeof f !== "string") return;
      await withImport("Reading JSON…", () => importJson(f));
      return;
    }
    if (which === "x") {
      const f = await openDialog({ filters: [{ name: "Saved tweets", extensions: ["jsonl", "json"] }] });
      if (typeof f !== "string") return;
      await withImport("Reading saved tweets…", () => importX(f));
      return;
    }
    if (which === "export-json") {
      const f = await saveDialog({ defaultPath: "highlight-scout-export.json", filters: [{ name: "JSON", extensions: ["json"] }] });
      if (typeof f !== "string") return;
      try {
        const n = await exportJson(f);
        showToast(`Exported ${n.toLocaleString()} highlights`);
      } catch (e) {
        showToast(`Export failed: ${e instanceof Error ? e.message : String(e)}`);
      }
      return;
    }

    if (which === "meaning-build") {
      meaning.build();
      showToast("Building the meaning index in the background");
      return;
    }

    if (which === "ocr") {
      try {
        const n = await ocrImages();
        showToast(`OCR'd ${n} image(s)`);
      } catch (e) {
        showToast(`OCR failed: ${e instanceof Error ? e.message : String(e)}`);
      }
      return;
    }

    if (which === "readwise-tweets") {
      await withImport("Importing saved tweets from Readwise…", () => importReadwiseTweets());
      return;
    }

    const label = which === "readwise" ? "Updating from Readwise…" : "Starting Zotero import…";
    await withImport(label, () => (which === "readwise" ? runImport() : runZoteroImport()));
  };

  // Manual refresh: reload counts/facets and re-run the current search.
  const manualRefresh = () => {
    refreshMeta();
    if (onHighlights) setDataVersion((v) => v + 1);
    else archive.rerun();
    showToast("Refreshed");
  };

  // Run an import call with the busy flag + status, surfacing errors.
  const withImport = async (label: string, fn: () => Promise<unknown>) => {
    setImporting(true);
    setStatus(label);
    try {
      await fn();
    } catch (e) {
      setStatus(`Failed: ${e instanceof Error ? e.message : String(e)}`);
      setImporting(false);
    }
  };

  const commands = useMemo<Record<CommandId, () => void>>(() => ({
    focusSearch: () => { inputRef.current?.focus(); inputRef.current?.select(); },
    nextResult: () => move(1),
    prevResult: () => move(-1),
    openSource,
    copyHighlight,
    copyMarkdown,
    copyRichText,
    copyImage: copyImageCmd,
    copyImageText: copyImageTextCmd,
    copyCitation: copyCitationCmd,
    openWorkView: () => showWork(),
    openWorkWindow: () => openWorkWin(),
    openWorkMarkdown: openWorkMd,
    findRelated: () => findRelated(),
    togglePane: () => setShowPane((s) => !s),
    cycleSort: () => setSq((s) => cycleSort(s)),
    cycleGroup: () => setSq((s) => cycleGroup(s, query)),
    cycleDensity: () => setSq((s) => cycleDensity(s)),
    // ⌘⇧T: the popover with its tag field focused (Highlights ticked first).
    openTags: () => { ensureHighlights(); setTagFocus((n) => n + 1); setFiltersOpen(true); },
    openFilters: () => { setTagFocus(0); setFiltersOpen((o) => !o); },
    openPalette: () => setOverlay("palette"),
    openHelp: () => setOverlay("palette"),
    openSettings: () => setOverlay("settings"),
    importUpdate: () => doImport("readwise"),
    importZotero: () => doImport("zotero"),
    clearColor: () => setSq((s) => ({ ...s, color: null })),
    nextGroup: () => (onHighlights ? hl.moveGroup(1) : archive.moveGroup(1)),
    prevGroup: () => (onHighlights ? hl.moveGroup(-1) : archive.moveGroup(-1)),
    rowActions: () => {
      document.querySelector<HTMLButtonElement>('.qf-row.on [data-action="quote-citation"]')?.focus();
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }), [activeRow, config, onHighlights, archive, hl, qfCopy, qf, query, sq]);

  // Recomputed when the user remaps shortcuts (bindingsVersion bumps).
  const keymap = useMemo(() => comboMap(), [bindingsVersion]);

  // App-wide keyboard handling on a GLOBAL listener (a div onKeyDown only fires
  // when focus is inside it — unreliable after the window shows). A ref keeps
  // the latest closures without re-binding the listener.
  const handleKeyRef = useRef<(e: KeyboardEvent) => void>(() => {});
  handleKeyRef.current = (e: KeyboardEvent) => {
    const target = e.target as HTMLElement | null;
    const inEditable =
      !!target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable);

    if (e.key === "Escape") {
      if (filtersOpen) { setFiltersOpen(false); inputRef.current?.focus(); }
      else if (workView) setWorkView(null);
      else if (overlay) setOverlay(null);
      // Esc hides the window and focus returns to the app the user came
      // from; the query stays for next time.
      else qfCopy.hide();
      return;
    }
    // Overlays manage their own keys (capture fields, nav).
    if (overlay || workView) return;

    // In semantic mode, Enter in the search box runs the search (it embeds
    // the query and ranks every ticked corpus by meaning).
    if (semantic && e.key === "Enter" && inEditable) {
      e.preventDefault();
      archive.run();
      return;
    }

    // "?" opens the palette unless typing into a non-empty query.
    if (e.key === "?" && !(inEditable && query)) {
      e.preventDefault();
      setOverlay("palette");
      return;
    }

    const combo = eventToCombo(e);
    if (!combo) return;
    const cmd = keymap[combo];
    if (!cmd) return;
    if (cmd === "copyHighlight" && window.getSelection()?.toString()) return;
    e.preventDefault();
    commands[cmd]?.();
  };

  useEffect(() => {
    const h = (e: KeyboardEvent) => handleKeyRef.current(e);
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, []);

  const runCommand = (id: CommandId) => {
    setOverlay(null);
    commands[id]?.();
  };

  const alerts = (
    <>
      {progress && (
        <div className="h-1 w-full bg-zinc-100">
          <div
            className="h-full bg-blue-400 transition-all"
            style={{ width: `${Math.min(100, Math.round((progress.current / Math.max(1, progress.total)) * 100))}%` }}
          />
        </div>
      )}
      {syncFailures.map((line) => (
        <div key={line} role="alert" className="redline">
          <Icon name="alert" size="sm" />
          <b>{line}</b>
          <span className="soft">stays here until a sync succeeds</span>
          <button className="b" onClick={() => doImport("sync-all")} disabled={importing}>
            <Icon name="refresh" size="sm" />{importing ? "Syncing…" : "Retry"}
          </button>
          <button className="b" onClick={openSyncSettings}>Sync settings</button>
        </div>
      ))}
    </>
  );

  // ---- footer ----
  const job = archive.job;
  const busyLine = status || hl.status;
  const hlTotal = tickedHighlightTotal(railResults, sq.offSources);
  const shownLine = onHighlights && hl.rows.length
    ? `${hlTotal !== null ? `${hlTotal.toLocaleString()} highlight${hlTotal === 1 ? "" : "s"}` : countsLine(counts.filter((c) => c.corpus === "highlights")) || "Highlights"} · ${hl.rows.length.toLocaleString()} shown${hl.hasMore ? "+" : ""}`
    : null;
  const footLeft = qfCopy.copied
    ? <>Copied{qfCopy.backTo ? ` · esc returns to ${qfCopy.backTo}` : " · esc hides"}</>
    : busyLine
      ? <span className={importing ? "busy" : ""}>{busyLine}{progress && ` (${Math.round((progress.current / Math.max(1, progress.total)) * 100)}%)`}</span>
      : job && jobVisible(job)
        ? <span className={archive.indexBusy ? "busy" : job.phase === "built" ? "" : "bad"}>{job.message}</span>
        : shownLine
          ? <>{shownLine}</>
          : !onHighlights && (archive.summary || archive.notes.length)
            ? <>{[archive.summary, ...archive.notes].filter(Boolean).join(" · ")}</>
            : <>{countsLine(counts) || (stats ? `${stats.highlights.toLocaleString()} highlights · ${stats.works.toLocaleString()} works` : "")}</>;
  const groupWord = group === "corpus" ? "corpus" : group === "none" ? "group" : GROUP_LABEL[group].split(" ")[0].toLowerCase();
  const footHints = (
    <>
      <span>↑↓ nav</span>
      {group !== "none" && <span className="hint"><kbd>⌥↓</kbd> next {groupWord}</span>}
      {onHighlights && <span className="hint"><kbd>↵</kbd> source</span>}
      <span className="hint"><kbd>⌘C</kbd> quote</span>
      <span className="hint"><kbd>⌘⇧C</kbd> + citation</span>
      {onHighlights && <span className="hint"><kbd>⌘⇧L</kbd> work</span>}
      <span className="hint"><kbd>⌘\</kbd> pane</span>
      <span className="hint"><kbd>esc</kbd> hide</span>
    </>
  );

  // ---- results and pane, from whichever engine answers ----
  const chipList = chipsOf(sq, query);
  const groupLabel = `Group: ${GROUP_LABEL[group]}`;
  const empty = !query.trim() && !(onHighlights && highlightSearchable(sq, query));
  const rowActions = {
    onQuote: qfCopy.copyQuote,
    onQuoteCitation: qfCopy.copyQuoteCitation,
    onLink: qfCopy.copyLink,
    copied: qfCopy.copied,
    backTo: qfCopy.backTo,
    ready: onHighlights ? !!activeRow : !!archive.passage,
  };
  const sourcesLabel = (() => {
    const t = tickedSources(sq, knownSources);
    return (t.length ? t : knownSources).map((x) => SOURCE_LABEL[x] ?? x).join(", ");
  })();

  const highlightEmpty = hl.loading
    ? <p>Searching…</p>
    : <p>{query.trim() ? `No results for “${query.trim()}”` : "No highlights match these filters"}</p>;

  const results = empty ? (
    <Welcome
      counts={countsLine(counts) || (stats ? `${stats.highlights.toLocaleString()} highlights · ${stats.works.toLocaleString()} works` : "")}
      noHighlights={stats?.highlights === 0}
      onImport={() => setOverlay("settings")}
    />
  ) : onHighlights ? (
    <HighlightResults
      query={query}
      terms={terms}
      rows={hl.rows}
      sections={hl.sections}
      density={sq.density}
      semantic={false}
      showPane={showPane}
      groupLabel={groupLabel}
      sort={sq.sort}
      onSort={(sort) => setSq((s) => ({ ...s, sort }))}
      activeId={hl.activeId}
      onActivate={hl.setActiveId}
      onOpenDetail={(id) => { const r = hl.rows.find((x) => x.highlight_id === id); if (r) setWorkView(r); }}
      onScrollEnd={hl.loadMore}
      hasMore={hl.hasMore}
      actions={rowActions}
      empty={highlightEmpty}
    />
  ) : (
    <GroupedResults
      query={query}
      terms={terms}
      results={archive.results}
      groups={archive.groups}
      groupLabel={groupLabel}
      density={sq.density}
      sort={sq.sort}
      onSort={(sort) => setSq((s) => ({ ...s, sort }))}
      activeKey={archive.activeKey}
      onSelect={archive.setActiveKey}
      onShowAll={(c) => { setSq((s) => onlyCorpus(s, c as "writing" | "tweets" | "highlights")); inputRef.current?.focus(); }}
      actions={rowActions}
      loading={archive.loading}
      error={archive.error}
      summary={archive.summary}
      waiting={semantic && query.trim() ? <p>Press <kbd>↵</kbd> to search {scopeWords(eff)} by meaning for “{query.trim()}”</p> : undefined}
    />
  );

  const pane = empty ? null : onHighlights ? (
    <HighlightPane
      row={activeRow}
      terms={terms}
      position={hl.position}
      format={qf.format}
      onFormat={qf.setFormat}
      copyPreview={qf.copyPreview}
      onCopyPreview={qf.toggleCopyPreview}
      onOpenUrl={(u) => { qf.remember(query); openUrl(u).catch(() => showToast("Could not open the link")); }}
      onFindRelated={(r) => findRelated(r)}
      onShowWork={(r) => showWork(r)}
      onNewWindow={(r) => openWorkWin(r)}
      onToast={showToast}
    />
  ) : (
    <Pane
      doc={archive.active}
      passage={archive.passage}
      citeError={archive.citeError}
      terms={terms}
      passageId={archive.passageId}
      onPassage={archive.setPassageId}
      format={qf.format}
      onFormat={qf.setFormat}
      copyPreview={qf.copyPreview}
      onCopyPreview={qf.toggleCopyPreview}
      onOpenPiece={(u) => { qf.remember(query); openUrl(u).catch(() => showToast("Could not open the link")); }}
      onOpenFile={(f) => openPath(f).catch(() => showToast("Could not open the file"))}
    />
  );

  return (
    <div className="qf relative flex h-screen flex-col overflow-hidden">
      <QuickFinder
        ref={inputRef}
        query={query}
        onQuery={setQuery}
        placeholder={`Search ${scopeWords(eff)}…`}
        loading={onHighlights ? hl.loading : archive.loading}
        mode={sq.mode}
        onMode={onMode}
        filters={
          <FiltersPopover
            state={sq}
            query={query}
            onState={update}
            open={filtersOpen}
            onOpenChange={setFiltersOpen}
            colors={facets?.colors ?? []}
            tags={tags}
            tagFocus={tagFocus}
            onPickTag={pickTag}
            sourcesLabel={sourcesLabel}
          />
        }
        showPane={showPane && !empty}
        onTogglePane={() => setShowPane((s) => !s)}
        chips={chipList}
        chipColor={sq.color}
        onClearChip={(id) => setSq((s) => clearChip(s, id))}
        onClearAll={() => setSq((s) => clearAll(s, query))}
        scopeNote={scopeNote(sq, query)}
        banner={<MeaningBanner notice={notice} onBuild={meaning.build} />}
        rail={{
          counts,
          results: railResults,
          corpora: sq.corpora,
          sourceOn: (src) => sourceTicked(sq, src),
          onCorpus: (c) => { setSq((s) => toggleCorpus(s, c)); inputRef.current?.focus(); },
          onSource: (src) => { setSq((s) => toggleSource(s, src, knownSources)); inputRef.current?.focus(); },
          recent: qf.recent,
          onRecent: (q) => { setQuery(q); inputRef.current?.focus(); },
        }}
        results={results}
        pane={pane}
        alerts={alerts}
        footLeft={footLeft}
        footHints={footHints}
        version={APP_VERSION}
        onRefresh={manualRefresh}
        onSettings={() => setOverlay("settings")}
      />

      {toast && (
        <div title={toastTitle || undefined} className="hs-toast" role="status">
          <svg className="icon" viewBox="0 0 24 24" aria-hidden="true"><path d="m5 12 5 5 9-10" /></svg>
          <span className="msg">{toast}</span>
          {toastDetails && (
            <>
              <span className="x">·</span>
              <button className="lk" onClick={() => { setToast(""); openSyncSettings(); }}>Details</button>
            </>
          )}
        </div>
      )}

      {overlay === "palette" && <CommandPalette onRun={runCommand} onClose={() => setOverlay(null)} />}
      {overlay === "importlog" && <ImportLogPanel onClose={() => setOverlay(null)} />}
      {csvPath && (
        <CsvMappingPanel
          path={csvPath}
          onClose={() => setCsvPath(null)}
          onImported={(s) => {
            setCsvPath(null);
            setImporting(false);
            setStatus(s.message);
            refreshMeta();
          }}
        />
      )}
      {overlay === "settings" && (
        <SettingsPanel
          initialTab={settingsTab}
          onClose={() => { setOverlay(null); setSettingsTab(undefined); setBindingsVersion((v) => v + 1); }}
          onImport={(a) => { setOverlay(null); doImport(a); }}
          onSaved={(shortcutChanged) => {
            setOverlay(null);
            setSettingsTab(undefined);
            setBindingsVersion((v) => v + 1);
            getConfig().then(setConfig).catch(() => {});
            getSettings().then((s) => setPageSize(s.result_limit || 80)).catch(() => {});
            showToast(shortcutChanged ? "Saved · restart to apply shortcut" : "Settings saved");
          }}
        />
      )}
      {workView && config && (
        <WorkView work={workView} archiveRoot={config.archive_path} onClose={() => setWorkView(null)} onToast={showToast} />
      )}
    </div>
  );
}
