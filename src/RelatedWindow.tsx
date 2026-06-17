import { useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { getHighlight, findRelated } from "./lib/api";
import { copyText } from "./lib/clipboard";
import { renderMarkdown } from "./lib/markdown";
import { authorLabel, formatDate } from "./lib/format";
import { resolveColor } from "./types";
import type { SearchResult } from "./types";

/** Standalone window: a source highlight at the top, semantically related
 * highlights underneath with a strength bar (?related=<id>). */
export default function RelatedWindow({ id }: { id: string }) {
  const [source, setSource] = useState<SearchResult | null>(null);
  const [related, setRelated] = useState<SearchResult[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [toast, setToast] = useState("");

  useEffect(() => {
    (async () => {
      try {
        const src = await getHighlight(id);
        setSource(src);
        if (src) {
          const rel = await findRelated(src.text, src.highlight_id);
          setRelated(rel);
          if (!rel.length) setError("No related highlights yet — run Rebuild semantic index in the main window.");
        } else {
          setError("Highlight not found.");
        }
      } catch (e) {
        setError(String(e));
      } finally {
        setLoading(false);
      }
    })();
  }, [id]);

  const showToast = (m: string) => { setToast(m); setTimeout(() => setToast(""), 1500); };
  const copy = (t: string) => copyText(t).then(() => showToast("Copied"));

  return (
    <div className="relative flex h-screen flex-col overflow-hidden bg-white text-zinc-900">
      {/* Source */}
      <div className="border-b border-zinc-200 bg-amber-50/40 p-4">
        <p className="mb-1 text-xs font-semibold uppercase tracking-wide text-amber-700">Related to</p>
        {source ? (
          <>
            <div className="text-sm leading-relaxed text-zinc-800">{renderMarkdown(source.text)}</div>
            <p className="mt-1 text-xs text-zinc-500">
              {source.title}{source.author ? ` · ${authorLabel(source)}` : ""}
            </p>
          </>
        ) : (
          <p className="text-sm text-zinc-400">{loading ? "Loading…" : error}</p>
        )}
      </div>

      {/* Related list */}
      <div className="flex-1 overflow-y-auto">
        {loading && <p className="p-4 text-sm text-zinc-400">Finding related…</p>}
        {!loading && related.length === 0 && (
          <p className="p-4 text-sm text-zinc-400">{error || "No related highlights."}</p>
        )}
        {related.map((r) => {
          const strength = Math.round((r.relevance ?? 0) * 100);
          const colorDot = resolveColor(r.annotation_color);
          return (
            <div key={r.highlight_id} className="border-b border-zinc-100 p-4">
              <div className="mb-1.5 flex items-center gap-2">
                <div className="h-1.5 w-24 overflow-hidden rounded-full bg-zinc-100">
                  <div className="h-full bg-violet-400" style={{ width: `${strength}%` }} />
                </div>
                <span className="text-xs text-violet-600">{strength}% match</span>
                {colorDot && <span className="h-2 w-2 rounded-full" style={{ backgroundColor: colorDot }} />}
                <button onClick={() => copy(r.text)} className="ml-auto rounded px-1.5 py-0.5 text-xs text-zinc-400 hover:bg-zinc-100">Copy</button>
              </div>
              {r.format === "image" && r.asset_path ? (
                <img src={convertFileSrc(r.asset_path)} alt="annotation" className="max-h-60 rounded border border-zinc-200" />
              ) : (
                <div className="text-sm leading-relaxed text-zinc-700">{renderMarkdown(r.text)}</div>
              )}
              <p className="mt-1 flex flex-wrap items-center gap-2 text-xs text-zinc-400">
                <span className="font-medium text-zinc-500">{r.title}</span>
                {r.author && <span>· {authorLabel(r)}</span>}
                {r.highlighted_at && <span>· {formatDate(r.highlighted_at)}</span>}
                {r.url && (
                  <button onClick={() => openUrl(r.zotero_link || r.url!)} className="text-blue-500 hover:underline">source ↗</button>
                )}
              </p>
            </div>
          );
        })}
      </div>

      {toast && (
        <div className="absolute bottom-6 left-1/2 -translate-x-1/2 rounded bg-zinc-800 px-3 py-1.5 text-xs text-white shadow-lg">{toast}</div>
      )}
    </div>
  );
}
