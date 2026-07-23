import { promises as fs } from "node:fs";
import * as vscode from "vscode";
import type { CommandSpec } from "../sessfind/types";
import { quoteCommand } from "../util/shellQuote";

/**
 * Launch a CommandSpec (resume or new-session) in an integrated terminal.
 * Ensures the working directory exists first, prompting to create it when the
 * project directory is missing (the decoded project path is heuristic and may
 * not exist on disk — mirrors the TUI's resume confirmation).
 */
export async function runCommandSpec(
  spec: CommandSpec,
  terminalName: string,
): Promise<void> {
  let cwd = spec.cwd ?? undefined;

  if (cwd) {
    const exists = await directoryExists(cwd);
    if (!exists) {
      const choice = await vscode.window.showWarningMessage(
        `Directory does not exist:\n${cwd}`,
        { modal: true },
        "Create it",
        "Use workspace folder",
      );
      if (choice === "Create it") {
        await fs.mkdir(cwd, { recursive: true });
      } else if (choice === "Use workspace folder") {
        cwd = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
      } else {
        return; // cancelled
      }
    }
  }

  const terminal = vscode.window.createTerminal({ name: terminalName, cwd });
  terminal.show();
  sendWhenShellReady(terminal, quoteCommand(spec.args));
}

/**
 * Run a command line once the shell is actually ready. Plain `sendText` types
 * into whatever currently owns the terminal — e.g. an oh-my-zsh "update?
 * [Y/n]" prompt during rc-file startup would swallow the first characters.
 * Shell integration fires only after the prompt appears. If it never
 * activates, ask before typing into a terminal whose current prompt is
 * unknown.
 */
function sendWhenShellReady(
  terminal: vscode.Terminal,
  commandLine: string,
): void {
  const FALLBACK_MS = 10_000;
  let done = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let integrationListener: vscode.Disposable | undefined;
  let closeListener: vscode.Disposable | undefined;

  const cleanup = () => {
    integrationListener?.dispose();
    closeListener?.dispose();
    if (timer) {
      clearTimeout(timer);
    }
  };

  const run = () => {
    if (done) {
      return;
    }
    done = true;
    cleanup();
    if (terminal.shellIntegration) {
      terminal.shellIntegration.executeCommand(commandLine);
    }
  };

  integrationListener = vscode.window.onDidChangeTerminalShellIntegration(
    (e) => {
      if (e.terminal === terminal) {
        run();
      }
    },
  );
  closeListener = vscode.window.onDidCloseTerminal((closed) => {
    if (closed === terminal && !done) {
      done = true;
      cleanup();
    }
  });
  const offerFallback = async () => {
    if (done) {
      return;
    }
    const choice = await vscode.window.showWarningMessage(
      "The terminal shell is not ready yet. Running now could type into a startup prompt.",
      "Run anyway",
      "Copy command",
    );
    if (done) {
      return;
    }
    if (choice === "Run anyway") {
      done = true;
      cleanup();
      terminal.sendText(commandLine);
    } else if (choice === "Copy command") {
      done = true;
      cleanup();
      await vscode.env.clipboard.writeText(commandLine);
    }
  };
  timer = setTimeout(() => {
    void offerFallback();
  }, FALLBACK_MS);

  if (terminal.shellIntegration) {
    run();
  }
}

async function directoryExists(path: string): Promise<boolean> {
  try {
    const stat = await fs.stat(path);
    return stat.isDirectory();
  } catch {
    return false;
  }
}
