import * as vscode from "vscode";
import type { SessfindClient } from "../sessfind/client";
import type { ProjectGroup, SessionSummary } from "../sessfind/types";
import { applyFilter, type SessionFilter } from "../state/filter";
import { groupSessions } from "./grouping";
import { MessageItem, ProjectGroupItem, SessionItem } from "./items";

/**
 * "Projects" view: sessions auto-grouped by their directory. Top level is one
 * node per project; expanding a project lists its sessions, filtered from the
 * client's cached session list (one spawn per refresh, not one per node).
 * When a filter is active, groups and counts are recomputed from the matching
 * sessions only.
 */
export class AutoProjectsTreeProvider
  implements vscode.TreeDataProvider<vscode.TreeItem>
{
  private readonly emitter = new vscode.EventEmitter<void>();
  readonly onDidChangeTreeData = this.emitter.event;

  constructor(
    private readonly client: SessfindClient,
    private readonly getFilter: () => SessionFilter | undefined,
  ) {}

  refresh(): void {
    this.emitter.fire();
  }

  getTreeItem(element: vscode.TreeItem): vscode.TreeItem {
    return element;
  }

  async getChildren(element?: vscode.TreeItem): Promise<vscode.TreeItem[]> {
    if (!element) {
      return this.rootProjects();
    }
    if (element instanceof ProjectGroupItem) {
      return this.sessionsForProject(element.group);
    }
    return [];
  }

  private async rootProjects(): Promise<vscode.TreeItem[]> {
    const filter = this.getFilter();
    let projects: ProjectGroup[];
    if (filter) {
      const sessions = applyFilter(await this.client.sessions(), filter);
      projects = groupSessions(sessions);
      if (projects.length === 0) {
        return [new MessageItem(`No sessions match “${filter.query}”.`)];
      }
    } else {
      projects = await this.client.projects();
      if (projects.length === 0) {
        return [
          new MessageItem("No indexed sessions. Run “Sessfind: Refresh Index”."),
        ];
      }
    }
    return projects.map((p) => new ProjectGroupItem(p));
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
