// The meaning index (semantic search over every corpus, ticket 09): its
// state per corpus as the backend reports it (`corpus_meaning_state`), the
// background build's progress (`corpus:meaning`), and what the search box
// says about it in Semantic mode. The app never builds it on its own: the
// notice offers "Build meaning index" with the time and size it takes.
// Pure apart from the two invoke wrappers.
import { invoke } from "@tauri-apps/api/core";
import { CORPUS_LABEL } from "./quickFinder";

export interface CorpusVectors {
  corpus: string;
  /** `no_index`: the corpus has no full-text index yet (the keeper builds it). */
  state: "current" | "stale" | "missing" | "no_index";
  why: string | null;
  passages: number;
}

export interface MeaningEstimate {
  corpora: string[];
  minutes: number;
  megabytes: number;
  /** The one-off model download, when the model is not on disk yet. */
  download: string | null;
}

export interface MeaningJob {
  phase: "idle" | "building" | "built" | "failed";
  corpora: string[];
  corpus: string | null;
  done: number;
  total: number;
  message: string;
}

export interface MeaningState {
  available: boolean;
  model: string | null;
  model_ready: boolean;
  corpora: CorpusVectors[];
  estimate: MeaningEstimate | null;
  job: MeaningJob;
  running: boolean;
}

export function meaningState(): Promise<MeaningState> {
  return invoke<MeaningState>("corpus_meaning_state");
}

/** Start the background build (`corpora` empty: every corpus not current). */
export function meaningBuild(corpora: string[] = []): Promise<MeaningJob> {
  return invoke<MeaningJob>("corpus_meaning_build", { corpora });
}

export interface MeaningNotice {
  /** The vector-state UI's states: `missing` covers stale too. */
  kind: "missing" | "building" | "failed" | "unavailable";
  text: string;
  /** The button, when one is offered. */
  action?: string;
}

const label = (c: string) => CORPUS_LABEL[c as keyof typeof CORPUS_LABEL] ?? c;

function list(words: string[]): string {
  return words.length <= 1 ? words.join("") : `${words.slice(0, -1).join(", ")} and ${words[words.length - 1]}`;
}

/** "about 9 min and 265 MB" (plus the model download, once). */
export function estimateLine(e: MeaningEstimate): string {
  const parts: string[] = [];
  if (e.minutes > 0) parts.push(`about ${e.minutes} min and ${e.megabytes.toLocaleString()} MB`);
  const dl = e.download?.match(/about ([\d,]+) MB/)?.[1];
  if (e.download) parts.push(dl ? `a one-off model download of about ${dl} MB` : "a one-off model download");
  return parts.join(", plus ");
}

/**
 * What Semantic says about the meaning index for the ticked corpora, or null
 * when it is current for all of them (nothing to say).
 */
export function meaningNotice(state: MeaningState | null, job: MeaningJob | null, ticked: string[]): MeaningNotice | null {
  if (!state) return null;
  const j = job ?? state.job;
  if (j.phase === "building" || state.running) {
    const pct = j.total > 0 ? ` (${Math.round((j.done / j.total) * 100)}%)` : "";
    const where = j.corpus ? `${label(j.corpus)}: ${j.done.toLocaleString()} of ${j.total.toLocaleString()} passages${pct}` : j.message || "starting";
    return { kind: "building", text: `Building the meaning index in the background · ${where}. Semantic search uses each corpus as soon as it is done.` };
  }
  if (!state.available) return { kind: "unavailable", text: "Semantic search is unavailable: this build of the app has no embedding model. Keyword search works." };
  const mine = state.corpora.filter((c) => ticked.includes(c.corpus));
  const todo = mine.filter((c) => c.state === "missing" || c.state === "stale");
  const current = mine.filter((c) => c.state === "current");
  if (state.model_ready && todo.length === 0) return null;
  if (j.phase === "failed") return { kind: "failed", text: j.message, action: "Try again" };
  const what = todo.length
    ? `the meaning index for ${list(todo.map((c) => label(c.corpus)))} ${todo.every((c) => c.state === "stale") ? "(out of date)" : todo.some((c) => c.state === "stale") ? "(missing or out of date)" : "(not built)"}`
    : "its model on this Mac";
  const cost = state.estimate ? estimateLine(state.estimate) : "";
  const until = state.model_ready && current.length
    ? `Until then Semantic searches ${list(current.map((c) => label(c.corpus)))} by meaning and the rest by their words.`
    : "Until then Semantic is unavailable; Keyword works.";
  return {
    kind: "missing",
    text: `Semantic search needs ${what}. Building it runs in the background${cost ? ` and takes ${cost}` : ""}. ${until}`,
    action: "Build meaning index",
  };
}
