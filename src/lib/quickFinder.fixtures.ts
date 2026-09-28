// Archive search results for the quick finder tests.
import type { ArchiveDoc, ArchiveSearchResults } from "./archive";

export function doc(corpus: string, rel: string, o: Partial<ArchiveDoc> = {}): ArchiveDoc {
  return {
    corpus,
    rel_path: rel,
    path: `/abs/${rel}`,
    title: rel,
    author: "Dominik Lukeš",
    date: "2016-06-23",
    date_display: "23 June 2016",
    public_url: null,
    score: 1,
    rank: 1,
    title_match: false,
    passage_count: 1,
    hits: [{ passage_id: `${corpus}:${rel}:3`, line_start: 3, line_end: 3, line: 3, quote: `quote of ${rel}`, score: 1, link: null }],
    citation: { markdown: "", plain: "" },
    ...o,
  };
}

export function results(docs: ArchiveDoc[], total = docs.length): ArchiveSearchResults {
  return { schema_version: 1, query: "paths metaphor", corpora: ["writing", "tweets", "highlights"], total_documents: total, total_passages: total, results: docs };
}
