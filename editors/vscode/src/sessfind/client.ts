import { execFile } from "node:child_process";
import * as vscode from "vscode";
import type {
  Capabilities,
  CommandSpec,
  ProjectGroup,
  SearchMethod,
  SearchResult,
  SessionDetail,
  SessionSummary,
  ToolInfo,
} from "./types";

const MAX_BUFFER = 64 * 1024 * 1024; // large JSON dumps

/** Raised when the binary cannot be found on disk (ENOENT). */
export class BinaryNotFoundError extends Error {}

/** Raised when the binary exits non-zero; carries its stderr. */
export class SessfindError extends Error {
  constructor(
    message: string,
    readonly stderr: string,
    readonly code: number | null,
  ) {
    super(message);
  }
}

/**
 * Thin wrapper around the `sessfind` binary. Every method spawns the binary
 * with `--json` and parses stdout. A shared cache of the session list is kept
 * so the tree views render from one spawn per refresh rather than one per node.
 */
export class SessfindClient {
  private sessionCache: Promise<SessionSummary[]> | undefined;
  private capsCache: Promise<Capabilities> | undefined;

  private binaryPath(): string {
    return (
      vscode.workspace.getConfiguration("sessfind").get<string>("binaryPath") ||
      "sessfind"
    );
  }

  private run(args: string[], token?: vscode.CancellationToken): Promise<string> {
    const bin = this.binaryPath();
    return new Promise((resolve, reject) => {
      const child = execFile(
        bin,
        args,
        { maxBuffer: MAX_BUFFER },
        (err, stdout, stderr) => {
          if (err) {
            const code = (err as NodeJS.ErrnoException).code;
            if (code === "ENOENT") {
              reject(
                new BinaryNotFoundError(
                  `sessfind binary not found at '${bin}'.`,
                ),
              );
              return;
            }
            reject(
              new SessfindError(
                `sessfind ${args[0]} failed`,
                stderr?.toString() ?? "",
                typeof (err as { code?: unknown }).code === "number"
                  ? (err as { code: number }).code
                  : null,
              ),
            );
            return;
          }
          resolve(stdout.toString());
        },
      );
      token?.onCancellationRequested(() => child.kill());
    });
  }

  private async runJson<T>(
    args: string[],
    token?: vscode.CancellationToken,
  ): Promise<T> {
    const stdout = await this.run(args, token);
    return JSON.parse(stdout) as T;
  }

  capabilities(): Promise<Capabilities> {
    if (!this.capsCache) {
      this.capsCache = this.runJson<Capabilities>(["capabilities"]);
    }
    return this.capsCache;
  }

  toolsList(dir: string): Promise<ToolInfo[]> {
    return this.runJson<ToolInfo[]>(["tools", "list", "--dir", dir, "--json"]);
  }

  async sessions(force = false): Promise<SessionSummary[]> {
    if (force || !this.sessionCache) {
      this.sessionCache = this.runJson<SessionSummary[]>([
        "sessions",
        "list",
        "--json",
      ]);
    }
    return this.sessionCache;
  }

  projects(): Promise<ProjectGroup[]> {
    return this.runJson<ProjectGroup[]>(["projects", "list", "--json"]);
  }

  show(sessionId: string): Promise<SessionDetail> {
    return this.runJson<SessionDetail>(["show", sessionId, "--json"]);
  }

  search(
    query: string,
    method: SearchMethod,
    limit: number,
    token?: vscode.CancellationToken,
  ): Promise<SearchResult[]> {
    return this.runJson<SearchResult[]>(
      ["search", query, "--method", method, "-n", String(limit), "--json"],
      token,
    );
  }

  index(): Promise<string> {
    return this.run(["index"]);
  }

  stats(): Promise<unknown> {
    return this.runJson<unknown>(["stats", "--json"]);
  }

  /** Generate and store an LLM summary for a project directory (slow). */
  async projectsSummarize(dir: string, tool?: string): Promise<string> {
    const args = ["projects", "summarize", dir, "--json"];
    if (tool) {
      args.push("--tool", tool);
    }
    const out = JSON.parse(await this.run(args)) as { description: string };
    this.invalidate();
    return out.description;
  }

  /** Command that opens a chat about the project, context pre-loaded. */
  projectsChat(dir: string, tool?: string): Promise<CommandSpec> {
    const args = ["projects", "chat", dir, "--json"];
    if (tool) {
      args.push("--tool", tool);
    }
    return this.runJson<CommandSpec>(args);
  }

  // ── Mutations ──

  async tagAdd(sessionId: string, tags: string[]): Promise<void> {
    await this.run(["tag", "add", sessionId, ...tags]);
    this.invalidate();
  }

  async tagRemove(sessionId: string, tags: string[]): Promise<void> {
    await this.run(["tag", "rm", sessionId, ...tags]);
    this.invalidate();
  }

  async projectTagAdd(dir: string, tags: string[]): Promise<void> {
    await this.run(["tag", "add-project", dir, ...tags]);
    this.invalidate();
  }

  async projectTagRemove(dir: string, tags: string[]): Promise<void> {
    await this.run(["tag", "rm-project", dir, ...tags]);
    this.invalidate();
  }

  /** Set a custom display name, or clear it when `name` is null. */
  async sessionRename(sessionId: string, name: string | null): Promise<void> {
    if (name === null) {
      await this.run(["sessions", "rename", sessionId, "--clear"]);
    } else {
      await this.run(["sessions", "rename", sessionId, name]);
    }
    this.invalidate();
  }

  /** Drop cached data; call after a mutation or explicit refresh. */
  invalidate(): void {
    this.sessionCache = undefined;
    this.capsCache = undefined;
  }
}
