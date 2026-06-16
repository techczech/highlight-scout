import type { ReactNode } from "react";

/**
 * Render an FTS5 snippet safely. The only HTML FTS5 emits is <mark>…</mark>.
 * We split on those tags and render React elements — no dangerouslySetInnerHTML.
 */
export function renderSnippet(snippet: string): ReactNode {
  const clean = snippet.replace(/<(?!\/?mark\b)[^>]*>/gi, "");
  const parts = clean.split(/(<mark>|<\/mark>)/);

  const nodes: ReactNode[] = [];
  let inMark = false;
  let key = 0;

  for (const part of parts) {
    if (part === "<mark>") {
      inMark = true;
    } else if (part === "</mark>") {
      inMark = false;
    } else if (part) {
      nodes.push(
        inMark ? (
          <mark key={key++}>{part}</mark>
        ) : (
          <span key={key++}>{part}</span>
        )
      );
    }
  }

  return <>{nodes}</>;
}
