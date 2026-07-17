import { describe, expect, it } from "vitest";
import type { SessionSummary, UserProject } from "../sessfind/types";
import { belongsTo } from "./membership";

function session(over: Partial<SessionSummary>): SessionSummary {
  return {
    session_id: "s",
    source: "claude",
    project: "/home/me/proj",
    title: null,
    timestamp: "2026-01-01T00:00:00Z",
    snippet: "",
    tags: [],
    resume: { args: ["claude"], cwd: null },
    new_session: { args: ["claude"], cwd: null },
    ...over,
  };
}

const project: UserProject = {
  name: "p",
  root_dir: "/home/me/proj",
  dirs: ["/home/me/extra"],
  pinned_sessions: ["pinned-id"],
  description: null,
  created_at: "2026-01-01T00:00:00Z",
};

describe("belongsTo", () => {
  it("matches the root directory", () => {
    expect(belongsTo(session({ project: "/home/me/proj" }), project)).toBe(true);
  });

  it("matches an extra directory", () => {
    expect(belongsTo(session({ project: "/home/me/extra" }), project)).toBe(
      true,
    );
  });

  it("matches a pinned session regardless of directory", () => {
    expect(
      belongsTo(
        session({ project: "/somewhere/else", session_id: "pinned-id" }),
        project,
      ),
    ).toBe(true);
  });

  it("rejects unrelated sessions", () => {
    expect(
      belongsTo(session({ project: "/other", session_id: "x" }), project),
    ).toBe(false);
  });
});
