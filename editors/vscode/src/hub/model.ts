// Pure view-model computation for the hub webview: takes the raw state pushed
// by the extension and produces the render tree (filtered sessions, project
// hierarchy, tag groups). No `vscode` or DOM imports — unit-testable.

import { applyFilter, type SessionFilter } from "../state/filter";
import {
  sessionKey,
  type ProjectGroup,
  type SessionSummary,
} from "../sessfind/types";
import { buildDirTree, isProjectLeaf, type DirNode } from "../views/dirTree";
import { countTags, groupSessions } from "../views/grouping";
import { tagChildren } from "../views/tagIndex";
import type { HubState } from "./protocol";

export interface ProjectEntry {
  group: ProjectGroup;
  sessions: SessionSummary[];
}

export type ProjectNode =
  | { kind: "dir"; label: string; path: string; children: ProjectNode[]; here: ProjectEntry[] }
  | { kind: "project"; label: string; entry: ProjectEntry };

export interface TagEntry {
  tag: string;
  count: number;
  projects: ProjectEntry[];
  sessions: SessionSummary[];
}

/** A ranked search hit: the session plus the matching snippet to highlight. */
export interface ResultEntry {
  session: SessionSummary;
  snippet: string;
}

export interface HubModel {
  /** Ranked engine results when a query is active; empty otherwise. */
  results: ResultEntry[];
  /** Most-recent sessions for quick resume; only when no filter is active. */
  recent: SessionSummary[];
  projects: ProjectNode[];
  tags: TagEntry[];
  visibleSessions: number;
  totalSessions: number;
  filterActive: boolean;
  /** The active query (for highlighting), empty when none. */
  query: string;
}

const RECENT_LIMIT = 8;

export function toFilter(state: HubState): SessionFilter | undefined {
  if (!state.filter) {
    return undefined;
  }
  return {
    query: state.filter.query,
    engineIds: new Set(state.filter.engineIds),
    engineOnly: state.filter.engineOnly,
  };
}

export function buildModel(state: HubState): HubModel {
  const filter = toFilter(state);
  const sessions = applyFilter(state.sessions, filter);

  const byId = new Map(state.sessions.map((s) => [sessionKey(s), s]));

  // Ranked results: engine matches in rank order, each mapped to the session
  // (which carries tags/name). Sessions matched only by substring append after.
  const results: ResultEntry[] = [];
  if (state.filter) {
    const placed = new Set<string>();
    for (const m of state.filter.matches) {
      const session = byId.get(m.session_key);
      if (session) {
        results.push({ session, snippet: m.snippet });
        placed.add(m.session_key);
      }
    }
    for (const s of sessions) {
      if (!placed.has(sessionKey(s))) {
        results.push({ session: s, snippet: s.snippet });
      }
    }
  }

  // Recent: newest sessions, only shown when nothing is being searched.
  const recent = filter
    ? []
    : [...state.sessions]
        .sort((a, b) => b.timestamp.localeCompare(a.timestamp))
        .slice(0, RECENT_LIMIT);

  // Groups: server-computed when unfiltered, recomputed from matches when
  // filtered (so counts reflect the filter). Directory tags always come from
  // the full project list.
  const tagsByPath = new Map(state.projects.map((p) => [p.path, p.tags ?? []]));
  const rawGroups = filter ? groupSessions(sessions) : state.projects;
  const groups = rawGroups.map((g) => ({
    ...g,
    tags: (g.tags ?? []).length > 0 ? g.tags : tagsByPath.get(g.path) ?? [],
  }));

  const sessionsFor = (path: string) =>
    sessions.filter((s) => s.project === path);
  const entryFor = (g: ProjectGroup): ProjectEntry => ({
    group: g,
    sessions: sessionsFor(g.path),
  });

  const projects: ProjectNode[] =
    state.viewMode === "tree"
      ? buildDirTree(groups).map((n) => toProjectNode(n, entryFor))
      : groups.map((g) => ({ kind: "project", label: g.name, entry: entryFor(g) }));

  const tags: TagEntry[] = countTags(sessions).map((t) => {
    const { projects: taggedProjects, sessions: loose } = tagChildren(
      t.tag,
      sessions,
      groups,
    );
    return {
      tag: t.tag,
      count: t.session_count,
      projects: taggedProjects
        .map(entryFor)
        .filter((e) => !filter || e.sessions.length > 0),
      sessions: loose,
    };
  });

  return {
    results,
    recent,
    projects,
    tags,
    visibleSessions: sessions.length,
    totalSessions: state.sessions.length,
    filterActive: filter !== undefined,
    query: state.filter?.query ?? "",
  };
}

function toProjectNode(
  node: DirNode,
  entryFor: (g: ProjectGroup) => ProjectEntry,
): ProjectNode {
  if (isProjectLeaf(node)) {
    return { kind: "project", label: node.label, entry: entryFor(node.projects[0]) };
  }
  return {
    kind: "dir",
    label: node.label,
    path: node.path,
    children: node.children.map((c) => toProjectNode(c, entryFor)),
    here: node.projects.map(entryFor),
  };
}
