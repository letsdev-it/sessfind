import { describe, expect, it } from "vitest";
import type { ProjectGroup } from "../sessfind/types";
import { buildDirTree, isProjectLeaf } from "./dirTree";

function project(path: string): ProjectGroup {
  const parts = path.split("/").filter(Boolean);
  return {
    path,
    name: parts[parts.length - 1] ?? path,
    session_count: 1,
    last_activity: "2026-01-01T00:00:00Z",
    sources: ["claude"],
  };
}

describe("buildDirTree", () => {
  it("compacts single-child chains into one node", () => {
    const tree = buildDirTree([
      project("/Users/m/projects/sdd/alpha"),
      project("/Users/m/projects/sdd/beta"),
    ]);
    expect(tree).toHaveLength(1);
    expect(tree[0].label).toBe("Users/m/projects/sdd");
    expect(tree[0].children.map((c) => c.label)).toEqual(["alpha", "beta"]);
    expect(tree[0].children.every(isProjectLeaf)).toBe(true);
  });

  it("keeps same-named projects apart under different branches", () => {
    const tree = buildDirTree([
      project("/home/work/app"),
      project("/home/personal/app"),
    ]);
    expect(tree).toHaveLength(1);
    expect(tree[0].label).toBe("home");
    const [personal, work] = tree[0].children;
    expect(personal.label).toBe("personal/app");
    expect(work.label).toBe("work/app");
    expect(isProjectLeaf(personal)).toBe(true);
  });

  it("attaches a project that is itself a parent directory of others", () => {
    const tree = buildDirTree([project("/a"), project("/a/b")]);
    expect(tree).toHaveLength(1);
    const a = tree[0];
    expect(a.label).toBe("a");
    expect(isProjectLeaf(a)).toBe(false);
    expect(a.projects.map((p) => p.path)).toEqual(["/a"]);
    expect(a.children).toHaveLength(1);
    expect(isProjectLeaf(a.children[0])).toBe(true);
    expect(a.children[0].projects[0].path).toBe("/a/b");
  });

  it("returns one leaf per isolated project", () => {
    const tree = buildDirTree([project("/x/y/z")]);
    expect(tree).toHaveLength(1);
    expect(tree[0].label).toBe("x/y/z");
    expect(isProjectLeaf(tree[0])).toBe(true);
  });

  it("handles empty input", () => {
    expect(buildDirTree([])).toEqual([]);
  });
});
