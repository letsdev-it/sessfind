import type { SessionSummary } from "../sessfind/types";

/**
 * An active session filter: the raw query plus the session ids the search
 * engine matched for it. A session passes if the engine matched it OR the
 * query appears as a substring in its title/snippet/project/tags — so both
 * full-content search hits and quick "path fragment" filtering work.
 */
export interface SessionFilter {
  query: string;
  engineIds: ReadonlySet<string>;
}

export function sessionMatchesFilter(
  session: SessionSummary,
  filter: SessionFilter | undefined,
): boolean {
  if (!filter) {
    return true;
  }
  if (filter.engineIds.has(session.session_id)) {
    return true;
  }
  const needle = filter.query.toLowerCase();
  if (needle.length === 0) {
    return true;
  }
  return (
    (session.title ?? "").toLowerCase().includes(needle) ||
    session.snippet.toLowerCase().includes(needle) ||
    session.project.toLowerCase().includes(needle) ||
    session.tags.some((t) => t.toLowerCase().includes(needle))
  );
}

export function applyFilter(
  sessions: SessionSummary[],
  filter: SessionFilter | undefined,
): SessionSummary[] {
  return filter ? sessions.filter((s) => sessionMatchesFilter(s, filter)) : sessions;
}
