import { useEffect, useRef, useState } from "react";
import type { SearchResult } from "../types";
import { copyHtml, copyImage, copyText } from "../lib/clipboard";
import { imageText, imageSources, toHtml, toMarkdown, toPlainText } from "../lib/copyFormats";

interface Props {
  row: SearchResult;
  onToast: (msg: string) => void;
}

export function CopyMenu({ row, onToast }: Props) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const imgs = imageSources(row);
  const ocr = imageText(row);

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
    <div ref={ref} className="qf-copymenu">
      <button className="qf-act" onClick={() => setOpen((o) => !o)} aria-haspopup="menu" aria-expanded={open}>
        Copy ▾
      </button>
      {open && (
        <div className="menu" role="menu">
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
          <Item
            disabled={!ocr}
            onClick={() => run(() => copyText(ocr ?? ""), "Copied image text", "Copy failed")}
          >
            Text from image
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
    <button role="menuitem" disabled={disabled} onClick={onClick}>
      {children}
    </button>
  );
}
