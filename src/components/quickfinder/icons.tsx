// Stroke icons from the round-1 mockups (_build/lib.py ICONS).
const PATHS = {
  search: <><circle cx="11" cy="11" r="7" /><path d="m20 20-3.5-3.5" /></>,
  link: <><path d="M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1" /><path d="M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1" /></>,
  quote: <><path d="M7 7h4v4c0 3-1.5 5-4 6" /><path d="M14 7h4v4c0 3-1.5 5-4 6" /></>,
  check: <path d="m5 12 5 5 9-10" />,
  ext: <><path d="M14 4h6v6" /><path d="M20 4 10 14" /><path d="M19 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1h5" /></>,
  spark: <path d="M12 3l1.8 5.2L19 10l-5.2 1.8L12 17l-1.8-5.2L5 10l5.2-1.8z" />,
  cite: <path d="M5 6h14M5 10h14M5 14h9M5 18h6" />,
  clock: <><circle cx="12" cy="12" r="9" /><path d="M12 7v5l3 2" /></>,
  set: <><rect x="4" y="4" width="16" height="5" rx="1.5" /><rect x="4" y="11" width="16" height="4" rx="1.5" /><rect x="4" y="17" width="16" height="3" rx="1.5" /></>,
  gear: <><circle cx="12" cy="12" r="3" /><path d="M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9l2.1 2.1M17 17l2.1 2.1M4.9 19.1 7 17M17 7l2.1-2.1" /></>,
  alert: <><circle cx="12" cy="12" r="9" /><path d="M12 8v5M12 16h.01" /></>,
  refresh: <><path d="M20 11a8 8 0 1 0-2.3 5.7" /><path d="M20 4v7h-7" /></>,
  pane: <><rect x="3" y="5" width="18" height="14" rx="2" /><path d="M14 5v14" /></>,
  filter: <><path d="M4 5h16l-6 7.5V19l-4-2v-4.5z" /></>,
  x: <path d="M7 7l10 10M17 7 7 17" />,
  enter: <><path d="M20 5v7a2 2 0 0 1-2 2H6" /><path d="m9 10-4 4 4 4" /></>,
  doc: <><path d="M14 3H6a1 1 0 0 0-1 1v16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V8z" /><path d="M14 3v5h5" /></>,
};

export type IconName = keyof typeof PATHS;

export function Icon({ name, size = "" }: { name: IconName; size?: "" | "sm" | "lg" }) {
  return (
    <svg className={`icon ${size}`} viewBox="0 0 24 24" aria-hidden="true">
      {PATHS[name]}
    </svg>
  );
}
