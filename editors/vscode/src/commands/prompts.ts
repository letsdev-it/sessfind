import * as vscode from "vscode";
import { SessfindClient, SessfindError } from "../sessfind/client";
import type { SessionSummary } from "../sessfind/types";
import { splitTags } from "../util/parseTags";

/**
 * Input prompts for hub mutations (tagging, rename). Each returns true when
 * something changed, so the caller knows to push fresh state.
 */

export async function promptAddTag(
  client: SessfindClient,
  kind: "session" | "project",
  id: string,
  label: string,
): Promise<boolean> {
  const input = await vscode.window.showInputBox({
    prompt: `Tags to add to ${label} (comma or space separated)`,
    placeHolder: "work, rust",
  });
  const tags = splitTags(input);
  if (tags.length === 0) {
    return false;
  }
  return guard(async () => {
    if (kind === "session") {
      await client.tagAdd(id, tags);
    } else {
      await client.projectTagAdd(id, tags);
    }
  });
}

export async function promptRemoveTag(
  client: SessfindClient,
  kind: "session" | "project",
  id: string,
  label: string,
  tags: string[],
): Promise<boolean> {
  if (tags.length === 0) {
    vscode.window.showInformationMessage(`${label} has no tags to remove.`);
    return false;
  }
  const picked = await vscode.window.showQuickPick(tags, {
    canPickMany: true,
    placeHolder: `Select tags to remove from ${label}`,
  });
  if (!picked || picked.length === 0) {
    return false;
  }
  return guard(async () => {
    if (kind === "session") {
      await client.tagRemove(id, picked);
    } else {
      await client.projectTagRemove(id, picked);
    }
  });
}

export async function promptRename(
  client: SessfindClient,
  session: SessionSummary,
): Promise<boolean> {
  const name = await vscode.window.showInputBox({
    prompt: "New session name (leave empty to restore the original title)",
    value: session.custom_name ?? session.title ?? "",
  });
  if (name === undefined) {
    return false;
  }
  const trimmed = name.trim();
  return guard(() =>
    client.sessionRename(
      session.session_id,
      trimmed.length === 0 ? null : trimmed,
    ),
  );
}

async function guard(action: () => Promise<void>): Promise<boolean> {
  try {
    await action();
    return true;
  } catch (err) {
    if (err instanceof SessfindError) {
      vscode.window.showErrorMessage(`sessfind: ${err.stderr || err.message}`);
    } else {
      vscode.window.showErrorMessage(`sessfind: ${String(err)}`);
    }
    return false;
  }
}
