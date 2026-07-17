import { describe, expect, it } from "vitest";
import { quoteArg, quoteCommand } from "./shellQuote";

describe("quoteArg", () => {
  it("leaves safe tokens bare", () => {
    expect(quoteArg("claude")).toBe("claude");
    expect(quoteArg("--resume")).toBe("--resume");
    expect(quoteArg("/Users/m/projects/foo-bar")).toBe(
      "/Users/m/projects/foo-bar",
    );
    expect(quoteArg("--resume=abc123")).toBe("--resume=abc123");
  });

  it("quotes paths with spaces", () => {
    expect(quoteArg("/Users/m/my project")).toBe("'/Users/m/my project'");
  });

  it("escapes embedded single quotes", () => {
    expect(quoteArg("it's")).toBe(`'it'\\''s'`);
  });

  it("quotes the empty string", () => {
    expect(quoteArg("")).toBe("''");
  });

  it("quotes shell metacharacters", () => {
    expect(quoteArg("a;b")).toBe("'a;b'");
    expect(quoteArg("$(rm)")).toBe("'$(rm)'");
  });
});

describe("quoteCommand", () => {
  it("joins a resume command safely", () => {
    expect(
      quoteCommand(["claude", "--resume", "abc"]),
    ).toBe("claude --resume abc");
  });

  it("quotes a cursor command with a spaced path", () => {
    expect(quoteCommand(["cursor", "/a b/c"])).toBe("cursor '/a b/c'");
  });
});
