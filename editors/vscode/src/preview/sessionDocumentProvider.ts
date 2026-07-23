import * as vscode from "vscode";
import type { SessfindClient } from "../sessfind/client";
import type { Source } from "../sessfind/types";
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
  private readonly emitter = new vscode.EventEmitter<vscode.Uri>();
  readonly onDidChange = this.emitter.event;

  constructor(private readonly client: SessfindClient) {}

  invalidate(sessionId: string, source: Source, title?: string | null): void {
    this.emitter.fire(SessionDocumentProvider.uriFor(sessionId, source, title));
  }

  static uriFor(
    sessionId: string,
    source: Source,
    title?: string | null,
  ): vscode.Uri {
    const base = sanitizeForPath(title ?? "") || sessionId;
    const query = new URLSearchParams({ id: sessionId, source }).toString();
    return vscode.Uri.from({
      scheme: SESSION_SCHEME,
      path: `/${base}.md`,
      query,
    });
  }

  async provideTextDocumentContent(uri: vscode.Uri): Promise<string> {
    // The id lives in the query; fall back to the path for legacy URIs.
    const params = new URLSearchParams(uri.query);
    const sessionId =
      params.get("id") ||
      uri.query ||
      uri.path.replace(/^\//, "").replace(/\.md$/, "");
    const source = params.get("source") as Source | null;
    if (!source) {
      return `# Session unavailable\n\nThe preview URI does not identify the session source. Reopen it from the Sessfind hub.\n`;
    }
    try {
      const detail = await this.client.show(sessionId, source);
      return renderSession(detail);
    } catch (err) {
      return `# Session unavailable\n\nCould not load \`${sessionId}\`.\n\n\`\`\`\n${String(err)}\n\`\`\`\n`;
    }
  }
}
