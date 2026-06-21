import { describe, expect, test } from "vitest";
import { tokenize } from "./markdown";
import { splitBlocks } from "./markdown";

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

describe("splitBlocks", () => {
  test("headings clamp at level 3", () => {
    expect(splitBlocks("# A\n## B\n#### C")).toEqual([
      { t: "heading", level: 1, text: "A" },
      { t: "heading", level: 2, text: "B" },
      { t: "heading", level: 3, text: "C" },
    ]);
  });

  test("consecutive > lines group into one quote, marker stripped", () => {
    expect(splitBlocks("> one\n> two")).toEqual([
      { t: "quote", lines: ["one", "two"] },
    ]);
  });

  test("blank lines separate paragraphs", () => {
    expect(splitBlocks("a\n\nb")).toEqual([
      { t: "para", text: "a" },
      { t: "para", text: "b" },
    ]);
  });

  test("tweet shape: body, attribution line, quote, image", () => {
    const text = "my take\n\n— Quoting @c:\n> the original\n\n![image](https://p/x.jpg)";
    expect(splitBlocks(text)).toEqual([
      { t: "para", text: "my take" },
      { t: "para", text: "— Quoting @c:" },
      { t: "quote", lines: ["the original"] },
      { t: "para", text: "![image](https://p/x.jpg)" },
    ]);
  });
});
