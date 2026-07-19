import { describe, expect, it } from "vitest";
import type { SessionSummary } from "../sessfind/types";
import { applyFilter, sessionMatchesFilter } from "./filter";

function session(over: Partial<SessionSummary>): SessionSummary {
  return {
    session_id: "s1",
    source: "claude",
    project: "/home/me/backend",
    title: "Fix auth flow",
    timestamp: "2026-01-01T00:00:00Z",
    snippet: "USER: login is broken",
    tags: ["work"],
    resume: { args: ["claude"], cwd: null },
    new_session: { args: ["claude"], cwd: null },
    ...over,
  };
}

describe("sessionMatchesFilter", () => {
  it("passes everything when no filter", () => {
    expect(sessionMatchesFilter(session({}), undefined)).toBe(true);
  });

  it("matches engine-provided ids regardless of text", () => {
    const f = { query: "zzz", engineIds: new Set(["s1"]) };
    expect(sessionMatchesFilter(session({}), f)).toBe(true);
  });

  it("matches substring in title, project, snippet and tags", () => {
    const none = new Set<string>();
    expect(
      sessionMatchesFilter(session({}), { query: "auth", engineIds: none }),
    ).toBe(true);
    expect(
      sessionMatchesFilter(session({}), { query: "backend", engineIds: none }),
    ).toBe(true);
    expect(
      sessionMatchesFilter(session({}), { query: "LOGIN", engineIds: none }),
    ).toBe(true);
    expect(
      sessionMatchesFilter(session({}), { query: "work", engineIds: none }),
    ).toBe(true);
  });

  it("rejects non-matching sessions", () => {
    expect(
      sessionMatchesFilter(session({}), {
        query: "frontend",
        engineIds: new Set(),
      }),
    ).toBe(false);
  });
});

describe("applyFilter", () => {
  it("filters a list", () => {
    const list = [
      session({ session_id: "a", title: "deploy fix" }),
      session({ session_id: "b", title: "unrelated", snippet: "", project: "/x", tags: [] }),
    ];
    const out = applyFilter(list, { query: "deploy", engineIds: new Set() });
    expect(out.map((s) => s.session_id)).toEqual(["a"]);
  });
});
