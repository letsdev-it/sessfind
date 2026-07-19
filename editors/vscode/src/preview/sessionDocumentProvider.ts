import * as vscode from "vscode";
import type { SessfindClient } from "../sessfind/client";
import { sanitizeForPath } from "../util/sanitize";
import { renderSession } from "./renderSession";

export const SESSION_SCHEME = "sessfind";

/**
 * Renders a session as a read-only Markdown document under the `sessfind:`
 * scheme, so VS Code's built-in Markdown preview and search work on it for
 * free. URIs look like `sessfind:/<display name>.md?<session_id>` — the path
 * carries the display name (that's what tab titles show), the query carries
 * the id used for lookup.
 */
export class SessionDocumentProvider
  implements vscode.TextDocumentContentProvider
{
  constructor(private readonly client: SessfindClient) {}

  static uriFor(sessionId: string, title?: string | null): vscode.Uri {
    const base = sanitizeForPath(title ?? "") || sessionId;
    return vscode.Uri.from({
      scheme: SESSION_SCHEME,
      path: `/${base}.md`,
      query: sessionId,
    });
  }

  async provideTextDocumentContent(uri: vscode.Uri): Promise<string> {
    // The id lives in the query; fall back to the path for legacy URIs.
    const sessionId =
      uri.query || uri.path.replace(/^\//, "").replace(/\.md$/, "");
    try {
      const detail = await this.client.show(sessionId);
      return renderSession(detail);
    } catch (err) {
      return `# Session unavailable\n\nCould not load \`${sessionId}\`.\n\n\`\`\`\n${String(err)}\n\`\`\`\n`;
    }
  }
}
