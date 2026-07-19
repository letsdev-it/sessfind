import * as vscode from "vscode";
import type { SessfindClient } from "../sessfind/client";
import { belongsTo } from "../views/membership";
import { lastSegment } from "../views/grouping";
import { renderProject } from "./renderProject";

export const PROJECT_SCHEME = "sessfind-project";

/**
 * Renders a project overview (metadata + metrics) as a read-only Markdown
 * document. URIs: `sessfind-project:/auto/<base64url(path)>.md` for
 * auto-projects and `sessfind-project:/user/<encoded name>.md` for user
 * projects.
 */
export class ProjectDocumentProvider
  implements vscode.TextDocumentContentProvider
{
  constructor(private readonly client: SessfindClient) {}

  static uriForAuto(path: string): vscode.Uri {
    const encoded = Buffer.from(path, "utf8").toString("base64url");
    return vscode.Uri.parse(`${PROJECT_SCHEME}:/auto/${encoded}.md`);
  }

  static uriForUser(name: string): vscode.Uri {
    return vscode.Uri.parse(
      `${PROJECT_SCHEME}:/user/${encodeURIComponent(name)}.md`,
    );
  }

  async provideTextDocumentContent(uri: vscode.Uri): Promise<string> {
    const match = uri.path.match(/^\/(auto|user)\/(.+)\.md$/);
    if (!match) {
      return "# Unknown project";
    }
    const [, kind, encoded] = match;
    try {
      return kind === "auto"
        ? await this.renderAuto(Buffer.from(encoded, "base64url").toString("utf8"))
        : await this.renderUser(decodeURIComponent(encoded));
    } catch (err) {
      return `# Project unavailable\n\n\`\`\`\n${String(err)}\n\`\`\`\n`;
    }
  }

  private async renderAuto(path: string): Promise<string> {
    const sessions = await this.client.sessions();
    return renderProject({
      title: lastSegment(path),
      kind: "auto",
      rootDir: path,
      dirs: [],
      pinnedSessions: [],
      description: null,
      sessions: sessions.filter((s) => s.project === path),
    });
  }

  private async renderUser(name: string): Promise<string> {
    const projects = await this.client.userProjects();
    const project = projects.find((p) => p.name === name);
    if (!project) {
      return `# ${name}\n\nProject not found.`;
    }
    const sessions = await this.client.sessions();
    return renderProject({
      title: project.name,
      kind: "user",
      rootDir: project.root_dir,
      dirs: project.dirs,
      pinnedSessions: project.pinned_sessions,
      description: project.description,
      sessions: sessions.filter((s) => belongsTo(s, project)),
    });
  }
}
