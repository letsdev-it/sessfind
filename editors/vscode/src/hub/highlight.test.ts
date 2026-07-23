import { describe, expect, it } from "vitest";
import { condenseSnippet, highlightSegments } from "./highlight";

describe("highlightSegments", () => {
  it("marks case-insensitive matches", () => {
    const segs = highlightSegments("Fix the Auth flow", "auth");
    expect(segs.map((s) => (s.mark ? `[${s.text}]` : s.text)).join("")).toBe(
      "Fix the [Auth] flow",
    );
  });

  it("handles multiple terms and strips fts operators", () => {
    const segs = highlightSegments("deploy the pipeline", "+deploy +pipeline");
    const marked = segs.filter((s) => s.mark).map((s) => s.text);
    expect(marked).toEqual(["deploy", "pipeline"]);
  });

  it("ignores terms shorter than 2 chars", () => {
    const segs = highlightSegments("a big cat", "a");
    expect(segs).toEqual([{ text: "a big cat", mark: false }]);
  });

  it("returns the whole text unmarked when no term matches", () => {
    const segs = highlightSegments("nothing here", "zzz");
    expect(segs).toEqual([{ text: "nothing here", mark: false }]);
  });

  it("coalesces overlapping term matches", () => {
    const segs = highlightSegments("abcdef", "abc cde");
    // abc and cde overlap → one contiguous marked run "abcde".
    expect(segs).toEqual([
      { text: "abcde", mark: true },
      { text: "f", mark: false },
    ]);
  });
});

describe("condenseSnippet", () => {
  it("returns short snippets as one line", () => {
    expect(condenseSnippet("USER: hi\nASSISTANT: yo", "hi")).toBe(
      "USER: hi ASSISTANT: yo",
    );
  });

  it("centers a long snippet on the first match", () => {
    const long = "x".repeat(200) + " needle " + "y".repeat(200);
    const out = condenseSnippet(long, "needle", 60);
    expect(out).toContain("needle");
    expect(out.startsWith("…")).toBe(true);
    expect(out.endsWith("…")).toBe(true);
    expect(out.length).toBeLessThanOrEqual(62);
  });

  it("truncates from the start when no term matches", () => {
    const out = condenseSnippet("z".repeat(300), "needle", 50);
    expect(out.endsWith("…")).toBe(true);
    expect(out.length).toBe(50);
  });
});
