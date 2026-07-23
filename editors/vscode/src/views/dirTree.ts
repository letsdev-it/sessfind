import type { ProjectGroup } from "../sessfind/types";

/**
 * A node in the directory-tree display of projects. Chains of directories
 * with a single child and no project of their own are compacted into one node
 * ("Users/m/projects"), like VS Code's compact folders.
 */
export interface DirNode {
  /** Compacted display label (one or more path segments joined by "/"). */
  label: string;
  /** Full path of the deepest segment this node represents. */
  path: string;
  children: DirNode[];
  /** Projects living exactly at this node's path or attached beneath it. */
  projects: ProjectGroup[];
}

interface TrieNode {
  segment: string;
  path: string;
  children: Map<string, TrieNode>;
  project?: ProjectGroup;
}

/** Build a compacted directory tree out of flat project groups. */
export function buildDirTree(projects: ProjectGroup[]): DirNode[] {
  const root: TrieNode = { segment: "", path: "", children: new Map() };

  for (const project of projects) {
    const segments = project.path.split(/[\\/]/).filter(Boolean);
    let node = root;
    let path = "";
    for (const segment of segments) {
      path += `/${segment}`;
      let child = node.children.get(segment);
      if (!child) {
        child = { segment, path, children: new Map() };
        node.children.set(segment, child);
      }
      node = child;
    }
    node.project = project;
  }

  return [...root.children.values()].map(compact).sort(byLabel);
}

function compact(node: TrieNode): DirNode {
  let label = node.segment;
  let current = node;
  // Collapse single-child chains that carry no project themselves.
  while (!current.project && current.children.size === 1) {
    const only = [...current.children.values()][0];
    label += `/${only.segment}`;
    current = only;
  }

  const children = [...current.children.values()].map(compact).sort(byLabel);
  const projects = current.project ? [current.project] : [];

  // A node that is purely a project leaf (no subdirectories) collapses into
  // its project; the caller renders projects directly.
  return { label, path: current.path, children, projects };
}

function byLabel(a: DirNode, b: DirNode): number {
  return a.label.localeCompare(b.label);
}

/** True when the node is nothing but a single project leaf. */
export function isProjectLeaf(node: DirNode): boolean {
  return node.children.length === 0 && node.projects.length === 1;
}
