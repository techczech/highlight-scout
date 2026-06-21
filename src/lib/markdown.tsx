import type { ReactNode } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";

// Minimal, safe Markdown → React. Supports **bold**, *italic*/_italic_,
// `code`, and [text](url). No dangerouslySetInnerHTML — every node is a real
// React element. Optionally highlights matched search terms with <mark>.

export type InlineToken =
  | { t: "text"; v: string }
  | { t: "bold"; v: string }
  | { t: "italic"; v: string }
  | { t: "code"; v: string }
  | { t: "link"; text: string; url: string }
  | { t: "image"; alt: string; url: string };

// Ordered alternation: image before link so `![](url)` wins; bare URL last so it
// only matches plain-text runs (markdown link/image URLs are consumed by their
// own branch first).
const INLINE_RE = new RegExp(
  [
    /!\[(?<imgAlt>[^\]]*)\]\((?<imgUrl>https?:\/\/[^)\s]+)\)/,
    /\*\*(?<b1>[^*]+)\*\*/,
    /__(?<b2>[^_]+)__/,
    /\*(?<i1>[^*\n]+)\*/,
    /(?<![A-Za-z0-9])_(?<i2>[^_\n]+)_(?![A-Za-z0-9])/,
    /`(?<code>[^`]+)`/,
    /\[(?<linkText>[^\]]+)\]\((?<linkUrl>https?:\/\/[^)\s]+)\)/,
    /(?<bare>https?:\/\/[^\s)]+)/,
  ]
    .map((r) => r.source)
    .join("|"),
  "g",
);

/** Split inline markdown into typed tokens. Pure — shared by the React renderer
 * and the copy serializers. */
export function tokenize(text: string): InlineToken[] {
  const out: InlineToken[] = [];
  let last = 0;
  let m: RegExpExecArray | null;
  INLINE_RE.lastIndex = 0;
  while ((m = INLINE_RE.exec(text))) {
    if (m.index > last) out.push({ t: "text", v: text.slice(last, m.index) });
    const g = m.groups!;
    if (g.imgUrl != null) out.push({ t: "image", alt: g.imgAlt || "", url: g.imgUrl });
    else if (g.b1 != null) out.push({ t: "bold", v: g.b1 });
    else if (g.b2 != null) out.push({ t: "bold", v: g.b2 });
    else if (g.i1 != null) out.push({ t: "italic", v: g.i1 });
    else if (g.i2 != null) out.push({ t: "italic", v: g.i2 });
    else if (g.code != null) out.push({ t: "code", v: g.code });
    else if (g.linkUrl != null) out.push({ t: "link", text: g.linkText!, url: g.linkUrl });
    else if (g.bare != null) out.push({ t: "link", text: g.bare, url: g.bare });
    last = INLINE_RE.lastIndex;
  }
  if (last < text.length) out.push({ t: "text", v: text.slice(last) });
  return out;
}

export type Block =
  | { t: "heading"; level: 1 | 2 | 3; text: string }
  | { t: "quote"; lines: string[] }
  | { t: "para"; text: string };

const HEADING_RE = /^(#{1,6})\s+(.*)$/;
const QUOTE_RE = /^>\s?/;

/** Split block-level markdown into headings, quote groups and paragraphs.
 * Paragraphs are runs of non-blank, non-heading, non-quote lines. */
export function splitBlocks(text: string): Block[] {
  const lines = (text || "").split("\n");
  const blocks: Block[] = [];
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    const h = HEADING_RE.exec(line);
    if (h) {
      blocks.push({ t: "heading", level: Math.min(3, h[1].length) as 1 | 2 | 3, text: h[2] });
      i++;
      continue;
    }
    if (QUOTE_RE.test(line)) {
      const q: string[] = [];
      while (i < lines.length && QUOTE_RE.test(lines[i])) {
        q.push(lines[i].replace(QUOTE_RE, ""));
        i++;
      }
      blocks.push({ t: "quote", lines: q });
      continue;
    }
    if (line.trim() === "") {
      i++;
      continue;
    }
    const para: string[] = [];
    while (
      i < lines.length &&
      lines[i].trim() !== "" &&
      !QUOTE_RE.test(lines[i]) &&
      !HEADING_RE.test(lines[i])
    ) {
      para.push(lines[i]);
      i++;
    }
    blocks.push({ t: "para", text: para.join("\n") });
  }
  return blocks;
}

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
