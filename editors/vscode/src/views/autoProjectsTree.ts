import * as vscode from "vscode";
import type { SessfindClient } from "../sessfind/client";
import type { ProjectGroup, SessionSummary } from "../sessfind/types";
import { applyFilter, type SessionFilter } from "../state/filter";
import { buildDirTree, isProjectLeaf, type DirNode } from "./dirTree";
import { groupSessions } from "./grouping";
import {
  DirectoryItem,
  MessageItem,
  ProjectGroupItem,
  SessionItem,
} from "./items";

export type ProjectsViewMode = "list" | "tree";

/**
 * "Projects" view: sessions auto-grouped by their directory. Two display
 * modes: a flat list of projects, or the directory hierarchy (compacted like
 * VS Code's folders) — useful when same-named projects live in different
 * locations. When a filter is active, groups and counts are recomputed from
 * the matching sessions only.
 */
export class AutoProjectsTreeProvider
  implements vscode.TreeDataProvider<vscode.TreeItem>
{
  private readonly emitter = new vscode.EventEmitter<void>();
  readonly onDidChangeTreeData = this.emitter.event;

  /** Lookup for DirectoryItem children, rebuilt on every root render. */
  private nodesByPath = new Map<string, DirNode>();

  constructor(
    private readonly client: SessfindClient,
    private readonly getFilter: () => SessionFilter | undefined,
    private readonly getMode: () => ProjectsViewMode,
  ) {}

  refresh(): void {
    this.emitter.fire();
  }

  getTreeItem(element: vscode.TreeItem): vscode.TreeItem {
    return element;
  }

  async getChildren(element?: vscode.TreeItem): Promise<vscode.TreeItem[]> {
    if (!element) {
      return this.roots();
    }
    if (element instanceof DirectoryItem) {
      const node = this.nodesByPath.get(element.path);
      return node ? this.renderNodes(node.children, node.projects) : [];
    }
    if (element instanceof ProjectGroupItem) {
      return this.sessionsForProject(element.group);
    }
    return [];
  }

  private async currentProjects(): Promise<ProjectGroup[]> {
    const filter = this.getFilter();
    if (filter) {
      return groupSessions(applyFilter(await this.client.sessions(), filter));
    }
    return this.client.projects();
  }

  private async roots(): Promise<vscode.TreeItem[]> {
    const projects = await this.currentProjects();
    if (projects.length === 0) {
      const filter = this.getFilter();
      return [
        new MessageItem(
          filter
            ? `No sessions match “${filter.query}”.`
            : "No indexed sessions. Run “Sessfind: Refresh Index”.",
        ),
      ];
    }

    if (this.getMode() === "list") {
      return projects.map((p) => new ProjectGroupItem(p));
    }

    const nodes = buildDirTree(projects);
    this.nodesByPath = new Map();
    this.indexNodes(nodes);
    return this.renderNodes(nodes, []);
  }

  private indexNodes(nodes: DirNode[]): void {
    for (const node of nodes) {
      this.nodesByPath.set(node.path, node);
      this.indexNodes(node.children);
    }
  }

  private renderNodes(
    nodes: DirNode[],
    hereProjects: ProjectGroup[],
  ): vscode.TreeItem[] {
    const items: vscode.TreeItem[] = [];
    // A project living exactly at an expanded directory renders first.
    for (const project of hereProjects) {
      items.push(new ProjectGroupItem(project));
    }
    for (const node of nodes) {
      if (isProjectLeaf(node)) {
        items.push(new ProjectGroupItem(node.projects[0], node.label));
      } else {
        items.push(new DirectoryItem(node.path, node.label));
      }
    }
    return items;
  }

  private async sessionsForProject(
    group: ProjectGroup,
  ): Promise<vscode.TreeItem[]> {
    const sessions = applyFilter(await this.client.sessions(), this.getFilter());
    return sessions
      .filter((s: SessionSummary) => s.project === group.path)
      .map((s) => new SessionItem(s));
  }
}
