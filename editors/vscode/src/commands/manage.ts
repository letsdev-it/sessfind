import * as vscode from "vscode";
import { SessfindClient, SessfindError } from "../sessfind/client";
import { splitTags } from "../util/parseTags";
import type { SessionItem, UserProjectItem } from "../views/items";

/**
 * Mutation commands for tags and user projects. Each prompts for input, calls
 * the CLI, then refreshes the views (the client cache is invalidated by the
 * mutating client methods). Errors surface via notifications.
 */
export function registerManageCommands(
  client: SessfindClient,
  refresh: () => void,
): vscode.Disposable[] {
  return [
    vscode.commands.registerCommand(
      "sessfind.addTag",
      async (item?: SessionItem) => {
        const sessionId = item?.session.session_id;
        if (!sessionId) {
          return;
        }
        const input = await vscode.window.showInputBox({
          prompt: "Tags to add (comma or space separated)",
          placeHolder: "work, rust",
        });
        const tags = splitTags(input);
        if (tags.length === 0) {
          return;
        }
        await guard(() => client.tagAdd(sessionId, tags), refresh);
      },
    ),

    vscode.commands.registerCommand(
      "sessfind.removeTag",
      async (item?: SessionItem) => {
        const session = item?.session;
        if (!session) {
          return;
        }
        if (session.tags.length === 0) {
          vscode.window.showInformationMessage("This session has no tags.");
          return;
        }
        const picked = await vscode.window.showQuickPick(session.tags, {
          canPickMany: true,
          placeHolder: "Select tags to remove",
        });
        if (!picked || picked.length === 0) {
          return;
        }
        await guard(() => client.tagRemove(session.session_id, picked), refresh);
      },
    ),

    vscode.commands.registerCommand("sessfind.createProject", async () => {
      const name = await vscode.window.showInputBox({
        prompt: "New project name",
      });
      if (!name) {
        return;
      }
      const root = await pickDirectory("Select the project root directory");
      if (!root) {
        return;
      }
      await guard(() => client.projectCreate(name, root), refresh);
    }),

    vscode.commands.registerCommand(
      "sessfind.deleteProject",
      async (item?: UserProjectItem) => {
        const name = item?.project.name;
        if (!name) {
          return;
        }
        const confirm = await vscode.window.showWarningMessage(
          `Delete project “${name}”? Sessions and tags are not affected.`,
          { modal: true },
          "Delete",
        );
        if (confirm !== "Delete") {
          return;
        }
        await guard(() => client.projectDelete(name), refresh);
      },
    ),

    vscode.commands.registerCommand(
      "sessfind.addDirToProject",
      async (item?: UserProjectItem) => {
        const name = item?.project.name;
        if (!name) {
          return;
        }
        const dir = await pickDirectory("Select a directory to add");
        if (!dir) {
          return;
        }
        await guard(() => client.projectAddDir(name, dir), refresh);
      },
    ),

    vscode.commands.registerCommand(
      "sessfind.pinSession",
      async (item?: SessionItem) => {
        const sessionId = item?.session.session_id;
        if (!sessionId) {
          return;
        }
        const name = await pickProjectName(client);
        if (!name) {
          return;
        }
        await guard(() => client.projectPin(name, sessionId), refresh);
      },
    ),

    vscode.commands.registerCommand(
      "sessfind.unpinSession",
      async (item?: SessionItem) => {
        const sessionId = item?.session.session_id;
        if (!sessionId) {
          return;
        }
        const name = await pickProjectName(client);
        if (!name) {
          return;
        }
        await guard(() => client.projectUnpin(name, sessionId), refresh);
      },
    ),
  ];
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
      vscode.window.showErrorMessage(
        `sessfind: ${err.stderr || err.message}`,
      );
    } else {
      vscode.window.showErrorMessage(`sessfind: ${String(err)}`);
    }
  }
}

async function pickDirectory(title: string): Promise<string | undefined> {
  const uris = await vscode.window.showOpenDialog({
    canSelectFolders: true,
    canSelectFiles: false,
    canSelectMany: false,
    title,
    defaultUri: vscode.workspace.workspaceFolders?.[0]?.uri,
  });
  return uris?.[0]?.fsPath;
}

async function pickProjectName(
  client: SessfindClient,
): Promise<string | undefined> {
  const projects = await client.userProjects();
  if (projects.length === 0) {
    vscode.window.showInformationMessage(
      'No user projects yet. Create one first with “Sessfind: Create Project”.',
    );
    return undefined;
  }
  const pick = await vscode.window.showQuickPick(
    projects.map((p) => ({ label: p.name, description: p.root_dir })),
    { placeHolder: "Select a project" },
  );
  return pick?.label;
}
