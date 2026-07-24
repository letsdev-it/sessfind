import { execFile } from "node:child_process";
import { existsSync } from "node:fs";
import { join } from "node:path";
import * as vscode from "vscode";
import type {
  Capabilities,
  CommandSpec,
  ProjectGroup,
  SearchMethod,
  SearchResult,
  SessionDetail,
  SessionSummary,
  Source,
  StatsPayload,
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

  private binaryPaths(): string[] {
    const configured = vscode.workspace
      .getConfiguration("sessfind")
      .get<string>("binaryPath");
    if (configured && configured !== "sessfind") {
      return [configured];
    }

    // VS Code launched from Finder/Dock does not inherit the user's shell
    // startup files, so PATH often omits Cargo's bin directory on macOS. Try
    // PATH first, then an existing standard Cargo installation. An explicit
    // binaryPath above always takes precedence.
    const cargoHome = process.env.CARGO_HOME ||
      (process.env.HOME ? join(process.env.HOME, ".cargo") : undefined);
    const cargoBinary = cargoHome ? join(cargoHome, "bin", "sessfind") : undefined;
    return cargoBinary && existsSync(cargoBinary)
      ? ["sessfind", cargoBinary]
      : ["sessfind"];
  }

  private run(
    args: string[],
    token?: vscode.CancellationToken,
    timeoutMs?: number,
  ): Promise<string> {
    const [bin, ...fallbacks] = this.binaryPaths();
    return this.runBinary(bin, fallbacks, args, token, timeoutMs);
  }

  private runBinary(
    bin: string,
    fallbacks: string[],
    args: string[],
    token?: vscode.CancellationToken,
    timeoutMs?: number,
  ): Promise<string> {
    return new Promise((resolve, reject) => {
      let cancellation: vscode.Disposable | undefined;
      // Node forwards spawn options to execFile at runtime, although the
      // ExecFileOptions type omits `detached`.
      const options = {
        maxBuffer: MAX_BUFFER,
        timeout: timeoutMs,
        killSignal: "SIGTERM" as const,
        detached: token !== undefined && process.platform !== "win32",
      };
      const child = execFile(
        bin,
        args,
        // A cancellable CLI gets its own Unix process group so terminating a
        // superseded search also stops an LLM subprocess it spawned.
        options,
        (err, stdout, stderr) => {
          cancellation?.dispose();
          if (err) {
            const code = (err as NodeJS.ErrnoException).code;
            if (code === "ENOENT" && fallbacks.length > 0) {
              const [fallback, ...remainingFallbacks] = fallbacks;
              resolve(
                this.runBinary(
                  fallback,
                  remainingFallbacks,
                  args,
                  token,
                  timeoutMs,
                ),
              );
              return;
            }
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
      if (token?.isCancellationRequested) {
        terminateProcessTree(child);
      } else {
        cancellation = token?.onCancellationRequested(() =>
          terminateProcessTree(child),
        );
      }
    });
  }

  private async runJson<T>(
    args: string[],
    token?: vscode.CancellationToken,
    timeoutMs?: number,
  ): Promise<T> {
    const stdout = await this.run(args, token, timeoutMs);
    return JSON.parse(stdout) as T;
  }

  capabilities(): Promise<Capabilities> {
    if (!this.capsCache) {
      const request = this.runJson<Capabilities>(["capabilities"]);
      this.capsCache = request;
      void request.catch(() => {
        if (this.capsCache === request) {
          this.capsCache = undefined;
        }
      });
    }
    return this.capsCache;
  }

  toolsList(dir: string): Promise<ToolInfo[]> {
    return this.runJson<ToolInfo[]>(["tools", "list", "--dir", dir, "--json"]);
  }

  async sessions(force = false): Promise<SessionSummary[]> {
    if (force || !this.sessionCache) {
      const request = this.runJson<SessionSummary[]>([
        "sessions",
        "list",
        "--json",
      ]);
      this.sessionCache = request;
      void request.catch(() => {
        if (this.sessionCache === request) {
          this.sessionCache = undefined;
        }
      });
    }
    return this.sessionCache;
  }

  projects(): Promise<ProjectGroup[]> {
    return this.runJson<ProjectGroup[]>(["projects", "list", "--json"]);
  }

  show(sessionId: string, source: Source): Promise<SessionDetail> {
    return this.runJson<SessionDetail>([
      "show",
      sessionId,
      "--source",
      source,
      "--json",
    ]);
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
      190_000,
    );
  }

  index(): Promise<string> {
    return this.run(["index"]);
  }

  stats(): Promise<StatsPayload> {
    return this.runJson<StatsPayload>(["stats", "--json"]);
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

  async tagAdd(
    sessionId: string,
    source: Source,
    tags: string[],
  ): Promise<void> {
    await this.run(["tag", "add", sessionId, "--source", source, ...tags]);
    this.invalidate();
  }

  async tagRemove(
    sessionId: string,
    source: Source,
    tags: string[],
  ): Promise<void> {
    await this.run(["tag", "rm", sessionId, "--source", source, ...tags]);
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
  async sessionRename(
    sessionId: string,
    source: Source,
    name: string | null,
  ): Promise<void> {
    if (name === null) {
      await this.run([
        "sessions",
        "rename",
        sessionId,
        "--source",
        source,
        "--clear",
      ]);
    } else {
      await this.run([
        "sessions",
        "rename",
        sessionId,
        "--source",
        source,
        name,
      ]);
    }
    this.invalidate();
  }

  /** Drop cached data; call after a mutation or explicit refresh. */
  invalidate(): void {
    this.sessionCache = undefined;
    this.capsCache = undefined;
  }
}

function terminateProcessTree(child: ReturnType<typeof execFile>): void {
  if (process.platform === "win32" && child.pid) {
    execFile(
      "taskkill",
      ["/pid", String(child.pid), "/t", "/f"],
      (err) => {
        if (err) {
          child.kill("SIGTERM");
        }
      },
    ).unref();
    return;
  }
  if (process.platform !== "win32" && child.pid) {
    try {
      process.kill(-child.pid, "SIGTERM");
      return;
    } catch {
      // The process may already have exited; child.kill is a safe fallback.
    }
  }
  child.kill("SIGTERM");
}
