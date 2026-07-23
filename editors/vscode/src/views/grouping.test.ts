import { describe, expect, it } from "vitest";
import type { SessionSummary } from "../sessfind/types";
import { countTags, groupSessions, lastSegment } from "./grouping";

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

describe("groupSessions", () => {
  it("groups by project with counts, sources and latest activity", () => {
    const groups = groupSessions([
      session({ session_id: "a", project: "/p1", timestamp: "2026-01-01T00:00:00Z" }),
      session({
        session_id: "b",
        project: "/p1",
        source: "codex",
        timestamp: "2026-02-01T00:00:00Z",
      }),
      session({ session_id: "c", project: "/p2", timestamp: "2026-03-01T00:00:00Z" }),
    ]);
    expect(groups).toHaveLength(2);
    // Sorted by last activity descending
    expect(groups[0].path).toBe("/p2");
    const p1 = groups[1];
    expect(p1.session_count).toBe(2);
    expect(p1.sources).toEqual(["claude", "codex"]);
    expect(p1.last_activity).toBe("2026-02-01T00:00:00Z");
  });

  it("returns empty for no sessions", () => {
    expect(groupSessions([])).toEqual([]);
  });
});

describe("countTags", () => {
  it("counts tags across sessions, sorted by count then name", () => {
    const tags = countTags([
      session({ session_id: "a", tags: ["work", "rust"] }),
      session({ session_id: "b", tags: ["work"] }),
    ]);
    expect(tags).toEqual([
      { tag: "work", session_count: 2 },
      { tag: "rust", session_count: 1 },
    ]);
  });
});

describe("lastSegment", () => {
  it("returns last path component", () => {
    expect(lastSegment("/a/b/c")).toBe("c");
    expect(lastSegment("plain")).toBe("plain");
  });
});
