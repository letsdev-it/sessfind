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
  terminal.sendText(quoteCommand(spec.args));
}

async function directoryExists(path: string): Promise<boolean> {
  try {
    const stat = await fs.stat(path);
    return stat.isDirectory();
  } catch {
    return false;
  }
}
