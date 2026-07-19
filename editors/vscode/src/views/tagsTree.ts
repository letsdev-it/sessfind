import * as vscode from "vscode";
import type { SessfindClient } from "../sessfind/client";
import { applyFilter, type SessionFilter } from "../state/filter";
import { countTags } from "./grouping";
import { MessageItem, ProjectGroupItem, SessionItem, TagItem } from "./items";
import { tagChildren } from "./tagIndex";

/**
 * "Tags" view. A tag can cover whole project directories and individual
 * sessions: under a tag node, tagged projects come first (expanding to their
 * sessions), then the individually tagged sessions. Counts are effective
 * (direct + inherited) and respect the active filter.
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
    const sessions = applyFilter(await this.client.sessions(), filter);

    if (!element) {
      const tags = countTags(sessions);
      if (tags.length === 0) {
        return [
          new MessageItem(
            filter
              ? `No tagged sessions match “${filter.query}”.`
              : "No tags yet. Tag a session or a project to see it here.",
          ),
        ];
      }
      return tags.map((t) => new TagItem(t.tag, t.session_count));
    }

    if (element instanceof TagItem) {
      const projects = await this.client.projects();
      const children = tagChildren(element.tag, sessions, projects);
      const visibleProjects = filter
        ? children.projects.filter((p) =>
            sessions.some((s) => s.project === p.path),
          )
        : children.projects;
      return [
        ...visibleProjects.map((p) => new ProjectGroupItem(p)),
        ...children.sessions.map((s) => new SessionItem(s)),
      ];
    }

    if (element instanceof ProjectGroupItem) {
      return sessions
        .filter((s) => s.project === element.group.path)
        .map((s) => new SessionItem(s));
    }

    return [];
  }
}
