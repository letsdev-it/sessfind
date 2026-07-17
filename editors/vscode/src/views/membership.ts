import type { SessionSummary, UserProject } from "../sessfind/types";

/**
 * Whether a session belongs to a user project: its directory is the root or an
 * extra dir, or it is explicitly pinned. Mirrors the CLI's membership rule for
 * `sessions list --user-project`.
 */
export function belongsTo(
  session: SessionSummary,
  project: UserProject,
): boolean {
  const dirs = new Set([project.root_dir, ...project.dirs]);
  return (
    dirs.has(session.project) ||
    project.pinned_sessions.includes(session.session_id)
  );
}
