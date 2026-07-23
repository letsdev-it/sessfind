import { describe, expect, it } from "vitest";
import { sanitizeForPath } from "./sanitize";

describe("sanitizeForPath", () => {
  it("keeps normal names intact", () => {
    expect(sanitizeForPath("Fix auth flow")).toBe("Fix auth flow");
  });

  it("strips path-hostile characters", () => {
    expect(sanitizeForPath("a/b\\c:d?e#f")).toBe("a b c d e f");
  });

  it("collapses whitespace and trims", () => {
    expect(sanitizeForPath("  a   b  ")).toBe("a b");
  });

  it("caps the length", () => {
    expect(sanitizeForPath("x".repeat(100)).length).toBe(60);
  });

  it("returns empty for empty input", () => {
    expect(sanitizeForPath("")).toBe("");
  });
});
