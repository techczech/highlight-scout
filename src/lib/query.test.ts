import { describe, expect, test } from "vitest";
import { parseSearch, scopeToFilters } from "./query";

describe("has_image filter", () => {
  test("i: token sets has_image and is removed from free text", () => {
    const p = parseSearch("i: karpathy");
    expect(p.has_image).toBe(true);
    expect(p.positive_terms).toContain("karpathy");
  });

  test("plain query leaves has_image false", () => {
    expect(parseSearch("karpathy").has_image).toBe(false);
  });

  test("img scope maps to has_image", () => {
    expect(scopeToFilters("img")).toEqual({ has_image: true });
  });
});
