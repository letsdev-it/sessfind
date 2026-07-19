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
 * Shell integration fires only after the prompt appears, so prefer it, with a
 * sendText fallback for shells where integration never activates.
 */
function sendWhenShellReady(
  terminal: vscode.Terminal,
  commandLine: string,
): void {
  const FALLBACK_MS = 4000;
  let done = false;

  const run = () => {
    if (done) {
      return;
    }
    done = true;
    listener.dispose();
    clearTimeout(timer);
    if (terminal.shellIntegration) {
      terminal.shellIntegration.executeCommand(commandLine);
    } else {
      terminal.sendText(commandLine);
    }
  };

  const listener = vscode.window.onDidChangeTerminalShellIntegration((e) => {
    if (e.terminal === terminal) {
      run();
    }
  });
  const timer = setTimeout(run, FALLBACK_MS);

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
