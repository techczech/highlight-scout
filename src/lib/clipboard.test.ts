import { beforeEach, describe, expect, test, vi } from "vitest";

const writes: Array<{ html: string; alt?: string }> = [];
vi.mock("@tauri-apps/plugin-clipboard-manager", () => ({
  writeHtml: vi.fn(async (html: string, alt?: string) => { writes.push({ html, alt }); }),
  writeText: vi.fn(async () => {}),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { copyCitation } from "./clipboard";

describe("citation clipboard writer", () => {
  beforeEach(() => { writes.length = 0; });

  test("puts text/html and text/plain on the clipboard in one write", async () => {
    await copyCitation({ html: '<p><a href="https://e.x">public</a></p>', text: "“Q.” · <https://e.x>" });
    expect(writes).toEqual([{ html: '<p><a href="https://e.x">public</a></p>', alt: "“Q.” · <https://e.x>" }]);
  });
});
