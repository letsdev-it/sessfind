import * as vscode from "vscode";
import type { SessfindClient } from "../sessfind/client";
import { applyFilter, type SessionFilter } from "../state/filter";
import {
  MessageItem,
  ProjectDirItem,
  SessionItem,
  UserProjectItem,
} from "./items";
import { belongsTo } from "./membership";

/**
 * "My Projects" view: user-defined projects. Expanding a project shows its
 * directories (root first, then extras — removable) followed by its member
 * sessions. Membership mirrors the CLI rule for `sessions list
 * --user-project`; an active filter narrows the sessions shown.
 */
export class UserProjectsTreeProvider
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
      const projects = await this.client.userProjects();
      if (projects.length === 0) {
        return [
          new MessageItem('No user projects. Use “Sessfind: Create Project”.'),
        ];
      }
      return projects.map((p) => new UserProjectItem(p));
    }
    if (element instanceof UserProjectItem) {
      const project = element.project;
      const dirs: vscode.TreeItem[] = [
        new ProjectDirItem(project.name, project.root_dir, true),
        ...project.dirs.map((d) => new ProjectDirItem(project.name, d, false)),
      ];
      const sessions = applyFilter(
        await this.client.sessions(),
        this.getFilter(),
      );
      const members = sessions
        .filter((s) => belongsTo(s, project))
        .map((s) => new SessionItem(s));
      return [...dirs, ...members];
    }
    return [];
  }
}
