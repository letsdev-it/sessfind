import { describe, expect, it } from "vitest";
import { splitTags } from "../util/parseTags";

describe("splitTags", () => {
  it("splits on commas and whitespace", () => {
    expect(splitTags("work, rust")).toEqual(["work", "rust"]);
    expect(splitTags("a b  c")).toEqual(["a", "b", "c"]);
    expect(splitTags("one,two,,three")).toEqual(["one", "two", "three"]);
  });

  it("returns empty for empty or undefined input", () => {
    expect(splitTags("")).toEqual([]);
    expect(splitTags(undefined)).toEqual([]);
    expect(splitTags("   ")).toEqual([]);
  });
});
