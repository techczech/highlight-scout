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

function headingClass(level: 1 | 2 | 3): string {
  return level === 1
    ? "mt-2 text-lg font-semibold text-zinc-800"
    : level === 2
      ? "mt-2 text-base font-semibold text-zinc-800"
      : "mt-2 text-sm font-semibold text-zinc-800";
}

/** Render inline tokens to React. listMode collapses images to a 🖼 marker so
 * single-line rows stay single-line. */
function renderTokens(
  tokens: InlineToken[],
  terms: string[] | undefined,
  key: string,
  listMode = false,
): ReactNode[] {
  return tokens.map((tk, i) => {
    const k = `${key}-${i}`;
    switch (tk.t) {
      case "text":
        return <span key={k}>{markTerms(tk.v, terms, k)}</span>;
      case "bold":
        return <strong key={k}>{markTerms(tk.v, terms, k)}</strong>;
      case "italic":
        return <em key={k}>{markTerms(tk.v, terms, k)}</em>;
      case "code":
        return (
          <code key={k} className="rounded bg-zinc-100 px-1 text-[0.9em]">
            {tk.v}
          </code>
        );
      case "link":
        return (
          <button
            key={k}
            onClick={() => openUrl(tk.url)}
            className="text-blue-500 hover:underline"
          >
            {markTerms(tk.text, terms, k)}
          </button>
        );
      case "image":
        return listMode ? (
          <span key={k}>🖼 </span>
        ) : (
          <img
            key={k}
            src={tk.url}
            alt={tk.alt}
            className="my-2 max-h-80 max-w-full rounded border border-zinc-200"
          />
        );
    }
  });
}

/** Inline render (newlines collapsed to spaces) — for one-line list rows. */
export function renderInlineMarkdown(text: string, terms?: string[]): ReactNode {
  return <>{renderTokens(tokenize((text || "").replace(/\s+/g, " ").trim()), terms, "il", true)}</>;
}

/** Block render (headings, blockquotes, paragraphs) — for the reading pane. */
export function renderMarkdown(text: string, terms?: string[]): ReactNode {
  const blocks = splitBlocks((text || "").trim());
  return (
    <>
      {blocks.map((b, i) => {
        if (b.t === "heading") {
          const H = `h${b.level}` as "h1" | "h2" | "h3";
          return (
            <H key={i} className={headingClass(b.level)}>
              {renderTokens(tokenize(b.text), terms, `h${i}`)}
            </H>
          );
        }
        if (b.t === "quote") {
          return (
            <blockquote key={i} className="my-2 border-l-2 border-zinc-300 pl-3 text-zinc-600">
              {b.lines.map((l, j) => (
                <div key={j}>{renderTokens(tokenize(l), terms, `q${i}-${j}`)}</div>
              ))}
            </blockquote>
          );
        }
        return (
          <p key={i} className={i > 0 ? "mt-2" : undefined}>
            {renderTokens(tokenize(b.text.replace(/\n/g, " ")), terms, `p${i}`)}
          </p>
        );
      })}
    </>
  );
}
