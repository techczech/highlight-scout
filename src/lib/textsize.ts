// App-wide text size. Tailwind's sizes are rem-based, so scaling the root
// font-size scales all text. Stored as a percentage in localStorage.

const KEY = "textSize";

export const TEXT_SIZES: Array<{ value: number; label: string }> = [
  { value: 100, label: "Normal" },
  { value: 120, label: "Large" },
  { value: 145, label: "Larger" },
  { value: 175, label: "Largest" },
  { value: 210, label: "Huge" },
];

export function getTextSize(): number {
  const v = Number(localStorage.getItem(KEY));
  return Number.isFinite(v) && v >= 80 ? v : 100;
}

export function applyTextSize(pct: number): void {
  document.documentElement.style.fontSize = `${(16 * pct) / 100}px`;
}

export function setTextSize(pct: number): void {
  localStorage.setItem(KEY, String(pct));
  applyTextSize(pct);
}
