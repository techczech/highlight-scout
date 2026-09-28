// Archive search results (writing, tweets, highlights) and their reading
// pane. Data path only: the HS-1B layout (rail, grouping, copy actions)
// comes later. Quotes are shown as the original source text, never rendered.
import { useEffect, useRef } from "react";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";
import { BADGE_CLASS, corpusBadge, jobVisible, sourceLine, type ArchiveDoc } from "../lib/archive";
import { docKey, type ArchiveSearchState } from "../lib/useArchiveSearch";

function Badge({ corpus }: { corpus: string }) {
  return (
    <span
      className={`shrink-0 rounded px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide ${BADGE_CLASS[corpus] ?? "bg-zinc-100 text-zinc-700"}`}
    >
      {corpusBadge(corpus)}
    </span>
  );
}

function byline(d: { author: string | null; date_display: string | null; date_source?: string }): string {
  const date = d.date_display ? (d.date_source === "saved" ? `saved ${d.date_display}` : d.date_display) : null;
  return [d.author, date].filter(Boolean).join(" · ");
}

function Row({ doc, active, onClick }: { doc: ArchiveDoc; active: boolean; onClick: () => void }) {
  const ref = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (active) ref.current?.scrollIntoView({ block: "nearest" });
  }, [active]);
  const hit = doc.hits[0];
  return (
    <button
      ref={ref}
      onClick={onClick}
      data-testid="archive-row"
      className={`block w-full border-b border-zinc-100 px-4 py-2.5 text-left ${active ? "bg-blue-50" : "hover:bg-zinc-50"}`}
    >
      <div className="flex items-center gap-2">
        <Badge corpus={doc.corpus} />
        <span className="truncate text-sm font-medium text-zinc-800">{doc.title || doc.rel_path}</span>
      </div>
      <div className="mt-0.5 truncate text-xs text-zinc-400">
        {byline(doc)}
        {doc.passage_count > 1 && ` · ${doc.passage_count} passages`}
      </div>
      {hit && <p className="mt-1 line-clamp-2 whitespace-pre-wrap text-sm text-zinc-600">{hit.quote}</p>}
    </button>
  );
}

export function ArchiveList({ s }: { s: ArchiveSearchState }) {
  if (s.docs.length === 0) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center gap-2 px-6 text-center text-sm text-zinc-400">
        {s.error ? (
          <p className="text-amber-700">{s.error}</p>
        ) : s.loading ? (
          <p>Searching writing, tweets and highlights…</p>
        ) : s.results ? (
          <p>No results for “{s.results.query}”</p>
        ) : (
          <>
            <p>Search your writing, tweets and highlights.</p>
            <p className="text-xs text-zinc-300">"exact phrase" · -exclude · prefix* · in:writing · after:2020 · /regex/</p>
          </>
        )}
      </div>
    );
  }
  return (
    <div className="min-h-0 flex-1 overflow-y-auto">
      {s.docs.map((d) => (
        <Row key={docKey(d)} doc={d} active={docKey(d) === s.activeKey} onClick={() => s.setActiveKey(docKey(d))} />
      ))}
    </div>
  );
}

export function ArchivePane({ s, onToast }: { s: ArchiveSearchState; onToast: (m: string) => void }) {
  const d = s.active;
  if (!d) return <div className="flex h-full items-center justify-center text-sm text-zinc-300">No result selected</div>;
  const c = s.cited;
  return (
    <div className="h-full overflow-y-auto px-6 py-5" data-testid="archive-pane">
      <div className="flex items-center gap-2">
        <Badge corpus={d.corpus} />
        <h2 className="text-base font-semibold text-zinc-900">{d.title || d.rel_path}</h2>
      </div>
      <p className="mt-1 text-xs text-zinc-500">{byline(d)}</p>

      <div className="mt-4 rounded border border-zinc-200 bg-zinc-50 p-4">
        {c ? (
          <p className="whitespace-pre-wrap font-serif text-[15px] leading-relaxed text-zinc-800">{c.quote}</p>
        ) : s.citeError ? (
          <p className="text-sm text-amber-700">{s.citeError}</p>
        ) : (
          <p className="text-sm text-zinc-400">Loading passage…</p>
        )}
      </div>

      {c && (
        <div className="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-zinc-500">
          <button
            onClick={() => openPath(c.path).catch(() => onToast("Could not open the file"))}
            className="break-all text-left font-mono text-zinc-600 underline decoration-zinc-300 hover:text-zinc-900"
            title="Open the source file"
          >
            {sourceLine(c)}
          </button>
          {c.public_url && (
            <button onClick={() => openUrl(c.public_url!)} className="text-blue-600 hover:underline">
              public ↗
            </button>
          )}
        </div>
      )}

      {c && (
        <div className="mt-4">
          <div className="mb-1 text-[11px] uppercase tracking-wide text-zinc-400">Citation</div>
          <pre className="whitespace-pre-wrap rounded border border-zinc-100 p-3 font-mono text-xs text-zinc-600">{c.citation.markdown}</pre>
        </div>
      )}

      {d.hits.length > 1 && (
        <div className="mt-5">
          <div className="mb-1 text-[11px] uppercase tracking-wide text-zinc-400">
            Passages in this {d.corpus === "tweets" ? "tweet" : "document"} ({d.hits.length})
          </div>
          {d.hits.map((h) => (
            <button
              key={h.passage_id}
              onClick={() => s.setPassageId(h.passage_id)}
              className={`block w-full rounded px-2 py-1.5 text-left text-sm ${h.passage_id === s.passageId ? "bg-blue-50 text-zinc-800" : "text-zinc-600 hover:bg-zinc-50"}`}
            >
              <span className="mr-2 font-mono text-[11px] text-zinc-400">line {h.line}</span>
              <span className="whitespace-pre-wrap">{h.quote}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

/** Status-bar line: counts, the engine's notes, and the index keeper's progress. */
export function ArchiveStatus({ s }: { s: ArchiveSearchState }) {
  const job = s.job;
  return (
    <span className="truncate" data-testid="archive-status">
      {s.summary}
      {s.notes.map((n) => <span key={n} className="text-zinc-500">{" · "}{n}</span>)}
      {job && jobVisible(job) && (
        <span className={s.indexBusy ? "text-blue-500" : job.phase === "built" ? "text-zinc-500" : "text-amber-700"}>
          {s.summary || s.notes.length ? " · " : ""}{job.message}
        </span>
      )}
    </span>
  );
}
