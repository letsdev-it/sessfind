import * as vscode from "vscode";
import { SessfindClient, SessfindError } from "../sessfind/client";
import { splitTags } from "../util/parseTags";
import { ProjectGroupItem, SessionItem } from "../views/items";

/**
 * Mutation commands: tagging (sessions and whole project directories) and
 * session rename. Each prompts for input, calls the CLI, then refreshes the
 * views. Errors surface via notifications.
 */
export function registerManageCommands(
  client: SessfindClient,
  refresh: () => void,
): vscode.Disposable[] {
  return [
    vscode.commands.registerCommand(
      "sessfind.addTag",
      async (item?: SessionItem | ProjectGroupItem) => {
        const target = describeTarget(item);
        if (!target) {
          return;
        }
        const input = await vscode.window.showInputBox({
          prompt: `Tags to add to ${target.label} (comma or space separated)`,
          placeHolder: "work, rust",
        });
        const tags = splitTags(input);
        if (tags.length === 0) {
          return;
        }
        await guard(async () => {
          if (target.kind === "session") {
            await client.tagAdd(target.id, tags);
          } else {
            await client.projectTagAdd(target.id, tags);
          }
        }, refresh);
      },
    ),

    vscode.commands.registerCommand(
      "sessfind.removeTag",
      async (item?: SessionItem | ProjectGroupItem) => {
        const target = describeTarget(item);
        if (!target) {
          return;
        }
        if (target.tags.length === 0) {
          vscode.window.showInformationMessage(
            `${target.label} has no tags to remove.`,
          );
          return;
        }
        const picked = await vscode.window.showQuickPick(target.tags, {
          canPickMany: true,
          placeHolder: `Select tags to remove from ${target.label}`,
        });
        if (!picked || picked.length === 0) {
          return;
        }
        await guard(async () => {
          if (target.kind === "session") {
            await client.tagRemove(target.id, picked);
          } else {
            await client.projectTagRemove(target.id, picked);
          }
        }, refresh);
      },
    ),

    vscode.commands.registerCommand(
      "sessfind.renameSession",
      async (item?: SessionItem) => {
        const session = item?.session;
        if (!session) {
          return;
        }
        const name = await vscode.window.showInputBox({
          prompt: "New session name (leave empty to restore the original title)",
          value: session.custom_name ?? session.title ?? "",
        });
        if (name === undefined) {
          return; // cancelled
        }
        const trimmed = name.trim();
        await guard(
          () =>
            client.sessionRename(
              session.session_id,
              trimmed.length === 0 ? null : trimmed,
            ),
          refresh,
        );
      },
    ),
  ];
}

interface MutationTarget {
  kind: "session" | "project";
  /** Session id or project directory path. */
  id: string;
  label: string;
  tags: string[];
}

function describeTarget(
  item: SessionItem | ProjectGroupItem | undefined,
): MutationTarget | undefined {
  if (item instanceof SessionItem) {
    return {
      kind: "session",
      id: item.session.session_id,
      label: item.session.title ?? item.session.session_id,
      tags: item.session.tags,
    };
  }
  if (item instanceof ProjectGroupItem) {
    return {
      kind: "project",
      id: item.group.path,
      label: item.group.name,
      tags: item.group.tags ?? [],
    };
  }
  return undefined;
}

async function guard(
  action: () => Promise<void>,
  refresh: () => void,
): Promise<void> {
  try {
    await action();
    refresh();
  } catch (err) {
    if (err instanceof SessfindError) {
      vscode.window.showErrorMessage(`sessfind: ${err.stderr || err.message}`);
    } else {
      vscode.window.showErrorMessage(`sessfind: ${String(err)}`);
    }
  }
}
