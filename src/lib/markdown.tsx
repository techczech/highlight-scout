import type { ReactNode } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";

// Minimal, safe Markdown → React. Supports **bold**, *italic*/_italic_,
// `code`, and [text](url). No dangerouslySetInnerHTML — every node is a real
// React element. Optionally highlights matched search terms with <mark>.

const INLINE_RE =
  /(\*\*([^*]+)\*\*)|(__([^_]+)__)|(\*([^*\n]+)\*)|(?<![A-Za-z0-9])_([^_\n]+)_(?![A-Za-z0-9])|(`([^`]+)`)|(\[([^\]]+)\]\((https?:\/\/[^)\s]+)\))/g;

function markTerms(text: string, terms: string[] | undefined, key: string): ReactNode[] {
  if (!terms || terms.length === 0) return [text];
  const clean = terms.map((t) => t.trim()).filter((t) => t.length >= 2);
  if (!clean.length) return [text];
  const escaped = clean
    .sort((a, b) => b.length - a.length)
    .map((t) => t.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
  const re = new RegExp(`(${escaped.join("|")})`, "gi");
  const lower = new Set(clean.map((t) => t.toLowerCase()));
  return text
    .split(re)
    .filter((p) => p !== "")
    .map((p, i) =>
      lower.has(p.toLowerCase()) ? (
        <mark key={`${key}-m${i}`} className="rounded bg-amber-200 px-0.5">{p}</mark>
      ) : (
        <span key={`${key}-t${i}`}>{p}</span>
      )
    );
}

function parseInline(text: string, terms: string[] | undefined, key: string): ReactNode[] {
  const nodes: ReactNode[] = [];
  let last = 0;
  let m: RegExpExecArray | null;
  let i = 0;
  INLINE_RE.lastIndex = 0;
  while ((m = INLINE_RE.exec(text))) {
    if (m.index > last) nodes.push(...markTerms(text.slice(last, m.index), terms, `${key}-${i}-pre`));
    const k = `${key}-${i}`;
    if (m[2]) nodes.push(<strong key={k}>{markTerms(m[2], terms, k)}</strong>);
    else if (m[4]) nodes.push(<strong key={k}>{markTerms(m[4], terms, k)}</strong>);
    else if (m[6]) nodes.push(<em key={k}>{markTerms(m[6], terms, k)}</em>);
    else if (m[7]) nodes.push(<em key={k}>{markTerms(m[7], terms, k)}</em>);
    else if (m[9]) nodes.push(<code key={k} className="rounded bg-zinc-100 px-1 text-[0.9em]">{m[9]}</code>);
    else if (m[11]) {
      const url = m[12];
      nodes.push(
        <button key={k} onClick={() => openUrl(url)} className="text-blue-500 hover:underline">
          {m[11]}
        </button>
      );
    }
    last = INLINE_RE.lastIndex;
    i++;
  }
  if (last < text.length) nodes.push(...markTerms(text.slice(last), terms, `${key}-end`));
  return nodes;
}

/** Inline render (newlines collapsed to spaces) — for one-line list rows. */
export function renderInlineMarkdown(text: string, terms?: string[]): ReactNode {
  return <>{parseInline((text || "").replace(/\s+/g, " ").trim(), terms, "il")}</>;
}

/** Block render (paragraphs split on blank lines) — for the reading pane. */
export function renderMarkdown(text: string, terms?: string[]): ReactNode {
  const paragraphs = (text || "").split(/\n{2,}/);
  return (
    <>
      {paragraphs.map((p, i) => (
        <p key={i} className={i > 0 ? "mt-2" : undefined}>
          {parseInline(p.replace(/\n/g, " "), terms, `p${i}`)}
        </p>
      ))}
    </>
  );
}
