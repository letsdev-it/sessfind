import * as vscode from "vscode";
import { SessfindClient } from "../sessfind/client";
import type { CommandSpec } from "../sessfind/types";
import { runCommandSpec } from "./terminal";

/**
 * Start a new session in `dir`, asking which installed tool to use when more
 * than one is available. Falls back to `fallback` (the session's own tool)
 * when the binary predates `tools list`.
 */
export async function startNewSession(
  client: SessfindClient,
  dir: string,
  fallback?: CommandSpec,
): Promise<void> {
  let spec: CommandSpec | undefined;

  const caps = await client.capabilities().catch(() => undefined);
  if (caps?.features.includes("tools-list")) {
    const tools = await client.toolsList(dir);
    if (tools.length === 0) {
      vscode.window.showErrorMessage(
        "sessfind: no AI CLI tools found on PATH (claude, opencode, copilot, cursor, codex).",
      );
      return;
    }
    if (tools.length === 1) {
      spec = tools[0].new_session;
    } else {
      const pick = await vscode.window.showQuickPick(
        tools.map((t) => ({
          label: t.name,
          description: t.new_session.args.join(" "),
          spec: t.new_session,
        })),
        { placeHolder: `Start a new session in ${dir} with…` },
      );
      if (!pick) {
        return; // cancelled
      }
      spec = pick.spec;
    }
  } else {
    spec = fallback;
  }

  if (!spec) {
    vscode.window.showErrorMessage(
      "sessfind: cannot determine how to start a new session — upgrade the sessfind binary.",
    );
    return;
  }
  await runCommandSpec(spec, `new: ${dir}`);
}
