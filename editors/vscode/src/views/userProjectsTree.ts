import * as vscode from "vscode";
import type { SessfindClient } from "../sessfind/client";
import { MessageItem, SessionItem, UserProjectItem } from "./items";
import { belongsTo } from "./membership";

/**
 * "My Projects" view: user-defined projects, expanding to their sessions.
 * A session belongs to a user project if its directory is the root or an extra
 * dir, or it is explicitly pinned — the same rule the CLI applies for
 * `sessions list --user-project`.
 */
export class UserProjectsTreeProvider
  implements vscode.TreeDataProvider<vscode.TreeItem>
{
  private readonly emitter = new vscode.EventEmitter<void>();
  readonly onDidChangeTreeData = this.emitter.event;

  constructor(private readonly client: SessfindClient) {}

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
      const sessions = await this.client.sessions();
      return sessions
        .filter((s) => belongsTo(s, element.project))
        .map((s) => new SessionItem(s));
    }
    return [];
  }
}
