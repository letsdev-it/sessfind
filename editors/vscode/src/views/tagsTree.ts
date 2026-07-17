import * as vscode from "vscode";
import type { SessfindClient } from "../sessfind/client";
import type { SessionSummary } from "../sessfind/types";
import { MessageItem, SessionItem, TagItem } from "./items";

/**
 * "Tags" view: one node per tag, expanding to the sessions carrying it.
 * Sessions are filtered client-side from the cached session list.
 */
export class TagsTreeProvider
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
      const tags = await this.client.tags();
      if (tags.length === 0) {
        return [new MessageItem("No tags yet. Tag a session to see it here.")];
      }
      return tags.map((t) => new TagItem(t.tag, t.session_count));
    }
    if (element instanceof TagItem) {
      const sessions = await this.client.sessions();
      return sessions
        .filter((s: SessionSummary) => s.tags.includes(element.tag))
        .map((s) => new SessionItem(s));
    }
    return [];
  }
}
