import * as vscode from "vscode";
import type { SessfindClient } from "../sessfind/client";
import { renderStats, type StatsPayload } from "./renderStats";

export const STATS_SCHEME = "sessfind-stats";

/** Renders the statistics dashboard as a read-only Markdown document. */
export class StatsDocumentProvider implements vscode.TextDocumentContentProvider {
  private readonly emitter = new vscode.EventEmitter<vscode.Uri>();
  readonly onDidChange = this.emitter.event;

  constructor(private readonly client: SessfindClient) {}

  static readonly uri = vscode.Uri.from({
    scheme: STATS_SCHEME,
    path: "/sessfind statistics.md",
  });

  /** Re-render on next open (e.g. after refresh). */
  invalidate(): void {
    this.emitter.fire(StatsDocumentProvider.uri);
  }

  async provideTextDocumentContent(): Promise<string> {
    try {
      const [stats, sessions, projects] = await Promise.all([
        this.client.stats() as Promise<StatsPayload>,
        this.client.sessions(),
        this.client.projects(),
      ]);
      return renderStats(stats, sessions, projects);
    } catch (err) {
      return `# Statistics unavailable\n\n\`\`\`\n${String(err)}\n\`\`\`\n`;
    }
  }
}
