import type { ProjectGroup, SessionSummary } from "../sessfind/types";

export interface TagChildren {
  /** Projects whose whole directory carries the tag. */
  projects: ProjectGroup[];
  /** Directly tagged sessions whose project is not already tag-covered. */
  sessions: SessionSummary[];
}

/**
 * What to show under a tag node: whole tagged projects first, then loose
 * sessions tagged individually (sessions inside a tagged project are reached
 * through the project node, not duplicated at top level).
 */
export function tagChildren(
  tag: string,
  sessions: SessionSummary[],
  projects: ProjectGroup[],
): TagChildren {
  const taggedProjects = projects.filter((p) => (p.tags ?? []).includes(tag));
  const covered = new Set(taggedProjects.map((p) => p.path));
  const loose = sessions.filter(
    (s) => s.tags.includes(tag) && !covered.has(s.project),
  );
  return { projects: taggedProjects, sessions: loose };
}
