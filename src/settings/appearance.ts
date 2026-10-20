// Text size: Tailwind's sizes are rem-based, so scaling the root font size scales all text. The value is the
// `appearance.textSize` setting (a percentage); every window applies it from the settings view.
export function textSizeOf(settings: Record<string, unknown> | undefined): number {
  const v = settings?.["appearance.textSize"];
  return typeof v === "number" && v >= 80 ? v : 100;
}

export function applyTextSize(pct: number): void {
  document.documentElement.style.fontSize = `${(16 * pct) / 100}px`;
}
