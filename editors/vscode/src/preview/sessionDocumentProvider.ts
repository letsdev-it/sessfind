import * as vscode from "vscode";
import type { SessfindClient } from "../sessfind/client";
import { renderSession } from "./renderSession";

export const SESSION_SCHEME = "sessfind";

/**
 * Renders a session as a read-only Markdown document under the `sessfind:`
 * scheme, so VS Code's built-in Markdown preview and search work on it for
 * free. URIs look like `sessfind:/<session_id>.md`.
 */
export class SessionDocumentProvider
  implements vscode.TextDocumentContentProvider
{
  constructor(private readonly client: SessfindClient) {}

  static uriFor(sessionId: string): vscode.Uri {
    return vscode.Uri.parse(`${SESSION_SCHEME}:/${sessionId}.md`);
  }

  async provideTextDocumentContent(uri: vscode.Uri): Promise<string> {
    const sessionId = uri.path.replace(/^\//, "").replace(/\.md$/, "");
    try {
      const detail = await this.client.show(sessionId);
      return renderSession(detail);
    } catch (err) {
      return `# Session unavailable\n\nCould not load \`${sessionId}\`.\n\n\`\`\`\n${String(err)}\n\`\`\`\n`;
    }
  }
}
