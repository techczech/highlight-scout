import { useEffect, useMemo, useState } from "react";
import { inspectCsv, importCsv, type CsvInspect, type CsvMapping } from "../lib/api";
import type { ImportStatus } from "../types";
import { Overlay } from "./TagPicker";

const FIELDS: Array<{ key: keyof CsvMapping; label: string; required?: boolean; detect: RegExp }> = [
  { key: "text", label: "Highlight text", required: true, detect: /high.?light|quote|^text$|content|clipping/i },
  { key: "title", label: "Title / book", detect: /title|book|work|^source$/i },
  { key: "author", label: "Author", detect: /author|creator|by/i },
  { key: "note", label: "Note", detect: /note|comment|remark|annotation$/i },
  { key: "date", label: "Date", detect: /date|added|created|highlighted/i },
  { key: "location", label: "Location / page", detect: /location|page|position|^loc/i },
  { key: "tags", label: "Tags", detect: /tags?|labels?|categor/i },
  { key: "url", label: "URL", detect: /url|link|source.?url|href/i },
  { key: "color", label: "Colour", detect: /colou?r/i },
];

const PROFILES_KEY = "csv-mappings";

function loadProfiles(): Record<string, CsvMapping> {
  try {
    return JSON.parse(localStorage.getItem(PROFILES_KEY) || "{}");
  } catch {
    return {};
  }
}
function saveProfile(sig: string, m: CsvMapping) {
  const all = loadProfiles();
  all[sig] = m;
  localStorage.setItem(PROFILES_KEY, JSON.stringify(all));
}

function autoDetect(headers: string[], delimiter: string): CsvMapping {
  const m: CsvMapping = {
    text: null, title: null, author: null, note: null, date: null,
    location: null, tags: null, url: null, color: null, delimiter,
  };
  const bag = m as unknown as Record<string, string | null>;
  for (const f of FIELDS) {
    const hit = headers.find((h) => f.detect.test(h));
    if (hit) bag[f.key] = hit;
  }
  return m;
}

interface Props {
  path: string;
  onClose: () => void;
  onImported: (s: ImportStatus) => void;
}

export function CsvMappingPanel({ path, onClose, onImported }: Props) {
  const [inspect, setInspect] = useState<CsvInspect | null>(null);
  const [mapping, setMapping] = useState<CsvMapping | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  const signature = useMemo(() => (inspect ? inspect.headers.join("|") : ""), [inspect]);

  useEffect(() => {
    inspectCsv(path)
      .then((ins) => {
        setInspect(ins);
        const saved = loadProfiles()[ins.headers.join("|")];
        setMapping(saved ?? autoDetect(ins.headers, ins.delimiter));
      })
      .catch((e) => setError(String(e)));
  }, [path]);

  const set = (key: keyof CsvMapping, value: string) =>
    setMapping((m) => (m ? { ...m, [key]: value || null } : m));

  const colIndex = (h: string | null) => (h && inspect ? inspect.headers.indexOf(h) : -1);
  const previewVal = (h: string | null) => {
    const i = colIndex(h);
    return i >= 0 && inspect?.sample_rows[0] ? inspect.sample_rows[0][i] ?? "" : "";
  };

  const doImport = async () => {
    if (!mapping || !mapping.text) return;
    setBusy(true);
    try {
      saveProfile(signature, mapping);
      const status = await importCsv(path, mapping);
      onImported(status);
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  };

  return (
    <Overlay title="Import CSV — map columns" onClose={onClose} wide>
      {error && <p className="mb-2 text-xs text-red-500">{error}</p>}
      {!inspect || !mapping ? (
        <p className="p-3 text-sm text-zinc-400">{error || "Reading file…"}</p>
      ) : (
        <>
          <p className="mb-2 text-xs text-zinc-400">
            {inspect.headers.length} columns · delimiter "{mapping.delimiter === "\t" ? "tab" : mapping.delimiter}".
            Map your columns to the fields below — only <strong>Highlight text</strong> is required.
          </p>
          <div className="max-h-[55vh] overflow-y-auto">
            <table className="w-full text-sm">
              <tbody>
                {FIELDS.map((f) => (
                  <tr key={f.key} className="border-b border-zinc-50">
                    <td className="py-1.5 pr-3 text-zinc-600">
                      {f.label}
                      {f.required && <span className="text-red-500"> *</span>}
                    </td>
                    <td className="py-1.5 pr-3">
                      <select
                        value={(mapping[f.key] as string) || ""}
                        onChange={(e) => set(f.key, e.target.value)}
                        className="w-full rounded border border-zinc-200 px-2 py-1 text-xs"
                      >
                        <option value="">— not mapped —</option>
                        {inspect.headers.map((h) => (
                          <option key={h} value={h}>{h}</option>
                        ))}
                      </select>
                    </td>
                    <td className="max-w-[40%] truncate py-1.5 text-xs text-zinc-400">
                      {previewVal(mapping[f.key] as string | null)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <div className="mt-3 flex items-center justify-end gap-2 border-t border-zinc-100 pt-3">
            <span className="mr-auto text-xs text-zinc-400">Unmapped columns are preserved.</span>
            <button onClick={onClose} className="rounded px-3 py-1.5 text-sm text-zinc-500 hover:bg-zinc-100">Cancel</button>
            <button
              onClick={doImport}
              disabled={!mapping.text || busy}
              className="rounded bg-amber-400 px-3 py-1.5 text-sm font-medium text-white hover:bg-amber-500 disabled:opacity-50"
            >
              {busy ? "Importing…" : "Import"}
            </button>
          </div>
        </>
      )}
    </Overlay>
  );
}
