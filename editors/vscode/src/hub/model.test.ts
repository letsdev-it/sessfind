import { describe, expect, it } from "vitest";
import type { ProjectGroup, SessionSummary } from "../sessfind/types";
import { buildModel } from "./model";
import type { HubState } from "./protocol";

function session(
  id: string,
  project: string,
  over: Partial<SessionSummary> = {},
): SessionSummary {
  return {
    session_id: id,
    source: "claude",
    project,
    title: `Session ${id}`,
    timestamp: "2026-01-01T00:00:00Z",
    snippet: "USER: hi",
    tags: [],
    resume: { args: ["claude"], cwd: null },
    new_session: { args: ["claude"], cwd: null },
    ...over,
  };
}

function group(path: string, count: number, tags: string[] = []): ProjectGroup {
  return {
    path,
    name: path.split("/").pop() ?? path,
    session_count: count,
    last_activity: "2026-01-01T00:00:00Z",
    sources: ["claude"],
    tags,
  };
}

function state(over: Partial<HubState>): HubState {
  return {
    sessions: [],
    projects: [],
    methods: ["fts", "fuzzy"],
    viewMode: "list",
    filter: null,
    busy: false,
    error: null,
    ...over,
  };
}

describe("buildModel", () => {
  // Sessions arrive from the CLI with effective tags (direct + inherited
  // from a tagged directory) — /p/alpha is dir-tagged "hub".
  const sessions = [
    session("a", "/p/alpha", { tags: ["hub", "work"] }),
    session("b", "/p/alpha", { tags: ["hub"] }),
    session("c", "/q/beta", { tags: ["work"] }),
  ];
  const projects = [group("/p/alpha", 2, ["hub"]), group("/q/beta", 1)];

  it("list mode: one project node per group with its sessions", () => {
    const model = buildModel(state({ sessions, projects }));
    expect(model.projects).toHaveLength(2);
    const alpha = model.projects.find(
      (n) => n.kind === "project" && n.entry.group.path === "/p/alpha",
    );
    expect(alpha?.kind).toBe("project");
    if (alpha?.kind === "project") {
      expect(alpha.entry.sessions.map((s) => s.session_id)).toEqual(["a", "b"]);
    }
    expect(model.totalSessions).toBe(3);
    expect(model.filterActive).toBe(false);
  });

  it("tree mode: builds a directory hierarchy", () => {
    const model = buildModel(state({ sessions, projects, viewMode: "tree" }));
    // /p/alpha and /q/beta share no root → two compacted leaves.
    expect(model.projects).toHaveLength(2);
    expect(model.projects.every((n) => n.kind === "project")).toBe(true);
    const labels = model.projects.map((n) => n.label).sort();
    expect(labels).toEqual(["p/alpha", "q/beta"]);
  });

  it("filter narrows sessions and recomputes groups", () => {
    const model = buildModel(
      state({
        sessions,
        projects,
        filter: { query: "zzz", engineIds: ["c"], engineOnly: true },
      }),
    );
    expect(model.visibleSessions).toBe(1);
    expect(model.projects).toHaveLength(1);
    const only = model.projects[0];
    if (only.kind === "project") {
      expect(only.entry.group.path).toBe("/q/beta");
    }
    expect(model.filterActive).toBe(true);
  });

  it("filtered groups keep directory tags from the full project list", () => {
    const model = buildModel(
      state({
        sessions,
        projects,
        filter: { query: "session", engineIds: [], engineOnly: false },
      }),
    );
    const alpha = model.projects.find(
      (n) => n.kind === "project" && n.entry.group.path === "/p/alpha",
    );
    if (alpha?.kind === "project") {
      expect(alpha.entry.group.tags).toEqual(["hub"]);
    } else {
      throw new Error("alpha missing");
    }
  });

  it("tags: project-tagged dirs come as projects, direct tags as sessions", () => {
    const model = buildModel(state({ sessions, projects }));
    const hub = model.tags.find((t) => t.tag === "hub");
    expect(hub?.count).toBe(2);
    expect(hub?.projects.map((e) => e.group.path)).toEqual(["/p/alpha"]);
    // Sessions inside the tagged project are reached via the project node.
    expect(hub?.sessions).toEqual([]);
    const work = model.tags.find((t) => t.tag === "work");
    // 'work' is direct on sessions a and c; neither project is tagged 'work'.
    expect(work?.count).toBe(2);
    expect(work?.projects).toEqual([]);
    expect(work?.sessions.map((s) => s.session_id).sort()).toEqual(["a", "c"]);
  });
});
