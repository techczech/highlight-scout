import { useEffect, useRef, useState } from "react";
import type { SearchResult } from "../types";
import { copyHtml, copyImage, copyText } from "../lib/clipboard";
import { imageSources, toHtml, toMarkdown, toPlainText } from "../lib/copyFormats";

interface Props {
  row: SearchResult;
  onToast: (msg: string) => void;
}

export function CopyMenu({ row, onToast }: Props) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const imgs = imageSources(row);

  useEffect(() => {
    if (!open) return;
    const onDoc = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onDoc);
    return () => document.removeEventListener("mousedown", onDoc);
  }, [open]);

  const run = (fn: () => Promise<void>, ok: string, fail: string) => {
    setOpen(false);
    fn()
      .then(() => onToast(ok))
      .catch(() => onToast(fail));
  };

  const copyRich = () =>
    copyHtml(toHtml(row)).catch(() => copyText(toPlainText(row)));

  const copyImg = () => {
    const src = imgs[0]?.path ?? imgs[0]?.url;
    if (!src) return Promise.reject(new Error("no image"));
    return copyImage(src);
  };

  const imageLabel = imgs.length > 1 ? `Image (1 of ${imgs.length})` : "Image";

  return (
    <div ref={ref} className="relative inline-block">
      <button
        onClick={() => setOpen((o) => !o)}
        className="rounded bg-zinc-100 px-1.5 py-0.5 text-zinc-500 hover:bg-zinc-200"
      >
        Copy ▾
      </button>
      {open && (
        <div className="absolute z-30 mt-1 w-44 rounded border border-zinc-200 bg-white py-1 text-sm shadow-lg">
          <Item onClick={() => run(() => copyText(toPlainText(row)), "Copied as plain text", "Copy failed")}>
            Plain text
          </Item>
          <Item onClick={() => run(() => copyText(toMarkdown(row)), "Copied as Markdown", "Copy failed")}>
            Markdown
          </Item>
          <Item onClick={() => run(copyRich, "Copied as rich text", "Copy failed")}>Rich text</Item>
          <Item
            disabled={imgs.length === 0}
            onClick={() =>
              run(
                copyImg,
                imgs.length > 1 ? `Copied image 1 of ${imgs.length}` : "Copied image",
                "Couldn't copy image",
              )
            }
          >
            {imageLabel}
          </Item>
          {row.citation && (
            <Item onClick={() => run(() => copyText(row.citation!), "Citation copied", "Copy failed")}>
              Citation
            </Item>
          )}
        </div>
      )}
    </div>
  );
}

function Item({
  children,
  onClick,
  disabled,
}: {
  children: React.ReactNode;
  onClick: () => void;
  disabled?: boolean;
}) {
  return (
    <button
      disabled={disabled}
      onClick={onClick}
      className="block w-full px-3 py-1 text-left text-zinc-700 hover:bg-zinc-50 disabled:cursor-default disabled:text-zinc-300 disabled:hover:bg-white"
    >
      {children}
    </button>
  );
}
