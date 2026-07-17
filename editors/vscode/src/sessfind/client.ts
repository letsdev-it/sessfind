import { execFile } from "node:child_process";
import * as vscode from "vscode";
import type {
  Capabilities,
  ProjectGroup,
  SearchMethod,
  SearchResult,
  SessionDetail,
  SessionSummary,
  TagCount,
  UserProject,
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
    return this.runJson<Capabilities>(["capabilities"]);
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

  tags(): Promise<TagCount[]> {
    return this.runJson<TagCount[]>(["tag", "list", "--json"]);
  }

  userProjects(): Promise<UserProject[]> {
    return this.runJson<UserProject[]>(["project", "list", "--json"]);
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

  // ── Mutations ──

  async tagAdd(sessionId: string, tags: string[]): Promise<void> {
    await this.run(["tag", "add", sessionId, ...tags]);
    this.invalidate();
  }

  async tagRemove(sessionId: string, tags: string[]): Promise<void> {
    await this.run(["tag", "rm", sessionId, ...tags]);
    this.invalidate();
  }

  async projectCreate(name: string, root: string): Promise<void> {
    await this.run(["project", "create", name, "--root", root]);
    this.invalidate();
  }

  async projectDelete(name: string): Promise<void> {
    await this.run(["project", "delete", name]);
    this.invalidate();
  }

  async projectAddDir(name: string, dir: string): Promise<void> {
    await this.run(["project", "add-dir", name, dir]);
    this.invalidate();
  }

  async projectPin(name: string, sessionId: string): Promise<void> {
    await this.run(["project", "add-session", name, sessionId]);
    this.invalidate();
  }

  async projectUnpin(name: string, sessionId: string): Promise<void> {
    await this.run(["project", "rm-session", name, sessionId]);
    this.invalidate();
  }

  /** Drop cached data; call after a mutation or explicit refresh. */
  invalidate(): void {
    this.sessionCache = undefined;
  }
}
