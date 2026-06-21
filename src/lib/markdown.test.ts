import { describe, expect, test } from "vitest";
import { tokenize } from "./markdown";

describe("tokenize", () => {
  test("plain text is one text token", () => {
    expect(tokenize("hello world")).toEqual([{ t: "text", v: "hello world" }]);
  });

  test("bold, italic, code", () => {
    expect(tokenize("**b** *i* `c`")).toEqual([
      { t: "bold", v: "b" },
      { t: "text", v: " " },
      { t: "italic", v: "i" },
      { t: "text", v: " " },
      { t: "code", v: "c" },
    ]);
  });

  test("markdown link", () => {
    expect(tokenize("see [docs](https://x.io/a)")).toEqual([
      { t: "text", v: "see " },
      { t: "link", text: "docs", url: "https://x.io/a" },
    ]);
  });

  test("image is its own token, matched before a link", () => {
    expect(tokenize("![pic](https://pbs.twimg.com/media/A.jpg)")).toEqual([
      { t: "image", alt: "pic", url: "https://pbs.twimg.com/media/A.jpg" },
    ]);
  });

  test("bare URL autolinks", () => {
    expect(tokenize("🔗 https://example.com/post")).toEqual([
      { t: "text", v: "🔗 " },
      { t: "link", text: "https://example.com/post", url: "https://example.com/post" },
    ]);
  });

  test("URL inside a markdown link is not double-matched", () => {
    expect(tokenize("[a](https://x.io)")).toEqual([
      { t: "link", text: "a", url: "https://x.io" },
    ]);
  });
});
