// The shared overlay frame (settings, palette, import log, CSV mapping, work
// view). The tag picker that gave this file its name is now the tag field in
// the Filters popover.
export function Overlay({
  title,
  onClose,
  children,
  wide,
}: {
  title: string;
  onClose: () => void;
  children: React.ReactNode;
  wide?: boolean;
}) {
  return (
    <div className="absolute inset-0 z-50 flex items-start justify-center bg-black/20 p-8" onClick={onClose}>
      <div
        className={`flex max-h-full w-full ${wide ? "max-w-3xl" : "max-w-md"} flex-col rounded-lg border border-zinc-200 bg-white p-4 text-zinc-900 shadow-xl [color-scheme:light]`}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="mb-2 flex items-center justify-between">
          <h2 className="text-sm font-semibold text-zinc-700">{title}</h2>
          <button onClick={onClose} className="text-lg leading-none text-zinc-400 hover:text-zinc-600">×</button>
        </div>
        {children}
      </div>
    </div>
  );
}
