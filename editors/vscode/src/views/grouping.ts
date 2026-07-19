import type { ProjectGroup, SessionSummary, TagCount } from "../sessfind/types";

/**
 * Group sessions into auto-projects by their directory — the client-side
 * mirror of `sessfind projects list`, used when a filter is active so project
 * nodes and counts reflect only the matching sessions.
 */
export function groupSessions(sessions: SessionSummary[]): ProjectGroup[] {
  const groups = new Map<string, ProjectGroup>();
  for (const s of sessions) {
    let g = groups.get(s.project);
    if (!g) {
      g = {
        path: s.project,
        name: lastSegment(s.project),
        session_count: 0,
        last_activity: s.timestamp,
        sources: [],
      };
      groups.set(s.project, g);
    }
    g.session_count += 1;
    if (s.timestamp > g.last_activity) {
      g.last_activity = s.timestamp;
    }
    if (!g.sources.includes(s.source)) {
      g.sources.push(s.source);
    }
  }
  return [...groups.values()].sort((a, b) =>
    b.last_activity.localeCompare(a.last_activity),
  );
}

/** Tag counts recomputed from a (possibly filtered) session list. */
export function countTags(sessions: SessionSummary[]): TagCount[] {
  const counts = new Map<string, number>();
  for (const s of sessions) {
    for (const t of s.tags) {
      counts.set(t, (counts.get(t) ?? 0) + 1);
    }
  }
  return [...counts.entries()]
    .map(([tag, session_count]) => ({ tag, session_count }))
    .sort(
      (a, b) => b.session_count - a.session_count || a.tag.localeCompare(b.tag),
    );
}

export function lastSegment(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}
