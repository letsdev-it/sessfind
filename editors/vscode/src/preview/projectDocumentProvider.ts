import * as vscode from "vscode";
import type { SessfindClient } from "../sessfind/client";
import { execCapture } from "../util/execCapture";
import { lastSegment } from "../views/grouping";
import { renderProject } from "./renderProject";

export const PROJECT_SCHEME = "sessfind-project";

/**
 * Renders a project overview (metadata + metrics) as a read-only Markdown
 * document. URIs: `sessfind-project:/<name>.md?<base64url(path)>` — the path
 * carries the display name for the tab title, the query carries the encoded
 * directory.
 */
export class ProjectDocumentProvider
  implements vscode.TextDocumentContentProvider
{
  constructor(private readonly client: SessfindClient) {}

  static uriForAuto(path: string): vscode.Uri {
    const encoded = Buffer.from(path, "utf8").toString("base64url");
    return vscode.Uri.from({
      scheme: PROJECT_SCHEME,
      path: `/${lastSegment(path)}.md`,
      query: encoded,
    });
  }

  async provideTextDocumentContent(uri: vscode.Uri): Promise<string> {
    try {
      const path = Buffer.from(uri.query, "base64url").toString("utf8");
      return await this.renderAuto(path);
    } catch (err) {
      return `# Project unavailable\n\n\`\`\`\n${String(err)}\n\`\`\`\n`;
    }
  }

  private async renderAuto(path: string): Promise<string> {
    const sessions = await this.client.sessions();
    const projects = await this.client.projects();
    const group = projects.find((p) => p.path === path);
    const base = renderProject({
      title: lastSegment(path),
      kind: "auto",
      rootDir: path,
      dirs: [],
      pinnedSessions: [],
      description: group?.description ?? null,
      tags: group?.tags ?? [],
      sessions: sessions.filter((s) => s.project === path),
    });
    return base + (await renderGitSections(path));
  }
}

/**
 * Best-effort git enrichment: recent commits (git) and open PRs (gh). Either
 * section is silently omitted when the tool is missing, the directory is not
 * a repository, or the call times out.
 */
async function renderGitSections(dir: string): Promise<string> {
  const [log, prs] = await Promise.all([
    execCapture(
      "git",
      ["log", "--pretty=format:%h  %ad  %s", "--date=short", "-12"],
      dir,
    ),
    execCapture(
      "gh",
      ["pr", "list", "--limit", "10", "--state", "open"],
      dir,
      5000,
    ),
  ]);

  const lines: string[] = [];
  if (log) {
    lines.push("## Recent commits", "", "```", log, "```", "");
  }
  if (prs) {
    lines.push("## Open pull requests", "", "```", prs, "```", "");
  }
  return lines.length > 0 ? `\n${lines.join("\n")}` : "";
}
