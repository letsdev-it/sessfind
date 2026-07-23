import { describe, expect, it } from "vitest";
import type { ProjectGroup, SessionSummary } from "../sessfind/types";
import { tagChildren } from "./tagIndex";

function session(id: string, projectPath: string, tags: string[]): SessionSummary {
  return {
    session_id: id,
    source: "claude",
    project: projectPath,
    title: null,
    timestamp: "2026-01-01T00:00:00Z",
    snippet: "",
    tags,
    resume: { args: ["claude"], cwd: null },
    new_session: { args: ["claude"], cwd: null },
  };
}

function projectGroup(path: string, tags: string[]): ProjectGroup {
  return {
    path,
    name: path.split("/").pop() ?? path,
    session_count: 1,
    last_activity: "2026-01-01T00:00:00Z",
    sources: ["claude"],
    tags,
  };
}

describe("tagChildren", () => {
  const projects = [
    projectGroup("/p1", ["work"]),
    projectGroup("/p2", []),
  ];
  const sessions = [
    session("in-p1", "/p1", ["work"]), // inherited via project
    session("loose", "/p2", ["work"]), // tagged directly
    session("other", "/p2", []),
  ];

  it("splits into tagged projects and loose sessions", () => {
    const { projects: tp, sessions: loose } = tagChildren(
      "work",
      sessions,
      projects,
    );
    expect(tp.map((p) => p.path)).toEqual(["/p1"]);
    // The session inside the tagged project is not duplicated at top level.
    expect(loose.map((s) => s.session_id)).toEqual(["loose"]);
  });

  it("returns empty for an unknown tag", () => {
    const { projects: tp, sessions: loose } = tagChildren(
      "nope",
      sessions,
      projects,
    );
    expect(tp).toEqual([]);
    expect(loose).toEqual([]);
  });
});
