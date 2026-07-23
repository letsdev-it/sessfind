import { describe, expect, it } from "vitest";
import type { SessionSummary } from "../sessfind/types";
import { renderProject } from "./renderProject";

function session(over: Partial<SessionSummary>): SessionSummary {
  return {
    session_id: "s",
    source: "claude",
    project: "/home/me/proj",
    title: "Session title",
    timestamp: "2026-01-15T10:00:00Z",
    snippet: "USER: hello",
    tags: [],
    resume: { args: ["claude"], cwd: null },
    new_session: { args: ["claude"], cwd: null },
    ...over,
  };
}

describe("renderProject", () => {
  it("renders project metadata", () => {
    const md = renderProject({
      title: "backend",
      rootDir: "/home/me/backend",
      description: "The API service.",
      sessions: [],
    });
    expect(md).toContain("# backend");
    expect(md).toContain("The API service.");
    expect(md).toContain("`/home/me/backend`");
    expect(md).toContain("**Sessions:** 0");
  });

  it("computes metrics from sessions", () => {
    const md = renderProject({
      title: "proj",
      rootDir: "/p",
      description: null,
      sessions: [
        session({
          session_id: "a",
          timestamp: "2026-01-10T09:00:00Z",
          tags: ["work"],
        }),
        session({
          session_id: "b",
          source: "codex",
          timestamp: "2026-01-12T09:00:00Z",
          tags: ["work", "ci"],
        }),
      ],
    });
    expect(md).toContain("**Sessions:** 2");
    expect(md).toContain("claude: 1");
    expect(md).toContain("codex: 1");
    expect(md).toContain("**First activity:** 2026-01-10");
    expect(md).toContain("**Last activity:** 2026-01-12");
    expect(md).toContain("**Active days:** 2");
    expect(md).toContain("`work` (2)");
    expect(md).toContain("| 2026-01-12 | codex | Session title | work, ci |");
  });

  it("escapes pipes in titles inside the table", () => {
    const md = renderProject({
      title: "p",
      rootDir: "/p",
      description: null,
      sessions: [session({ title: "a | b" })],
    });
    expect(md).toContain("a \\| b");
  });
});
