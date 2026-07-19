import * as vscode from "vscode";
import type { SessfindClient } from "../sessfind/client";
import type { TagCount } from "../sessfind/types";
import { applyFilter, type SessionFilter } from "../state/filter";
import { countTags } from "./grouping";
import { MessageItem, SessionItem, TagItem } from "./items";

/**
 * "Tags" view: one node per tag, expanding to the sessions carrying it.
 * With an active filter, tags and counts are recomputed from the matching
 * sessions only.
 */
export class TagsTreeProvider
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
    const filter = this.getFilter();
    if (!element) {
      let tags: TagCount[];
      if (filter) {
        tags = countTags(applyFilter(await this.client.sessions(), filter));
      } else {
        tags = await this.client.tags();
      }
      if (tags.length === 0) {
        return [
          new MessageItem(
            filter
              ? `No tagged sessions match “${filter.query}”.`
              : "No tags yet. Tag a session to see it here.",
          ),
        ];
      }
      return tags.map((t) => new TagItem(t.tag, t.session_count));
    }
    if (element instanceof TagItem) {
      const sessions = applyFilter(await this.client.sessions(), filter);
      return sessions
        .filter((s) => s.tags.includes(element.tag))
        .map((s) => new SessionItem(s));
    }
    return [];
  }
}
