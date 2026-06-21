import type { SearchResult } from "../types";
import { markdownQuote } from "./format";
import { type InlineToken, splitBlocks, tokenize } from "./markdown";

function escapeHtml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

function escapeAttr(s: string): string {
  return escapeHtml(s).replace(/"/g, "&quot;");
}

function inlinePlain(tokens: InlineToken[]): string {
  return tokens
    .map((tk) => {
      switch (tk.t) {
        case "text":
        case "bold":
        case "italic":
        case "code":
          return tk.v;
        case "link":
          return tk.text === tk.url ? tk.url : `${tk.text} (${tk.url})`;
        case "image":
          return "";
      }
    })
    .join("");
}

function inlineHtml(tokens: InlineToken[]): string {
  return tokens
    .map((tk) => {
      switch (tk.t) {
        case "text":
          return escapeHtml(tk.v);
        case "bold":
          return `<strong>${escapeHtml(tk.v)}</strong>`;
        case "italic":
          return `<em>${escapeHtml(tk.v)}</em>`;
        case "code":
          return `<code>${escapeHtml(tk.v)}</code>`;
        case "link":
          return `<a href="${escapeAttr(tk.url)}">${escapeHtml(tk.text)}</a>`;
        case "image":
          return `<img src="${escapeAttr(tk.url)}" alt="${escapeAttr(tk.alt)}">`;
      }
    })
    .join("");
}

function attribution(row: SearchResult): string {
  return `— ${row.author || "Unknown"}, ${row.title}`;
}

/** Readable text, all markdown stripped, images dropped, attribution appended. */
export function toPlainText(row: SearchResult): string {
  const body = splitBlocks((row.text || "").trim())
    .map((b) => {
      if (b.t === "heading") return inlinePlain(tokenize(b.text));
      if (b.t === "quote") return b.lines.map((l) => inlinePlain(tokenize(l))).join("\n");
      return inlinePlain(tokenize(b.text.replace(/\n/g, " ")));
    })
    .filter((s) => s.trim() !== "")
    .join("\n\n");
  return `${body}\n\n${attribution(row)}`;
}

/** Portable markdown: body verbatim (images + quotes kept) as a `>` quote + attribution. */
export function toMarkdown(row: SearchResult): string {
  return markdownQuote(row);
}

/** HTML for the clipboard `writeHtml` path — bold/italic/links, blockquotes,
 * headings, real <img>, and a trailing attribution line. */
export function toHtml(row: SearchResult): string {
  const blocks = splitBlocks((row.text || "").trim())
    .map((b) => {
      if (b.t === "heading") return `<h${b.level}>${inlineHtml(tokenize(b.text))}</h${b.level}>`;
      if (b.t === "quote")
        return `<blockquote>${b.lines.map((l) => inlineHtml(tokenize(l))).join("<br>")}</blockquote>`;
      return `<p>${inlineHtml(tokenize(b.text.replace(/\n/g, " ")))}</p>`;
    })
    .join("\n");
  return `${blocks}\n<p>${escapeHtml(attribution(row))}</p>`;
}

/** Image sources for copy-as-image, in document order. Zotero image annotations
 * expose their local file; tweets expose embedded remote URLs. Index 0 is primary. */
export function imageSources(row: SearchResult): Array<{ url?: string; path?: string }> {
  if (row.format === "image" && row.asset_path) return [{ path: row.asset_path }];
  return tokenize(row.text || "")
    .filter((tk): tk is Extract<InlineToken, { t: "image" }> => tk.t === "image")
    .map((tk) => ({ url: tk.url }));
}
