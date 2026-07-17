import * as vscode from "vscode";
import { runSearch } from "./commands/search";
import { runCommandSpec } from "./commands/terminal";
import {
  SESSION_SCHEME,
  SessionDocumentProvider,
} from "./preview/sessionDocumentProvider";
import {
  BinaryNotFoundError,
  SessfindClient,
  SessfindError,
} from "./sessfind/client";
import { SUPPORTED_JSON_API_VERSION, type Capabilities } from "./sessfind/types";
import { AutoProjectsTreeProvider } from "./views/autoProjectsTree";
import { SessionItem } from "./views/items";

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  const client = new SessfindClient();

  const autoProjects = new AutoProjectsTreeProvider(client);
  const previewProvider = new SessionDocumentProvider(client);

  context.subscriptions.push(
    vscode.window.registerTreeDataProvider("sessfind.autoProjects", autoProjects),
    vscode.workspace.registerTextDocumentContentProvider(
      SESSION_SCHEME,
      previewProvider,
    ),
  );

  const refreshAll = () => {
    client.invalidate();
    autoProjects.refresh();
  };

  context.subscriptions.push(
    vscode.commands.registerCommand("sessfind.search", () => runSearch(client)),

    vscode.commands.registerCommand("sessfind.refresh", refreshAll),

    vscode.commands.registerCommand("sessfind.indexNow", async () => {
      await vscode.window.withProgress(
        { location: vscode.ProgressLocation.Notification, title: "sessfind: indexing…" },
        async () => {
          try {
            await client.index();
            refreshAll();
            vscode.window.showInformationMessage("sessfind: index updated.");
          } catch (err) {
            reportError(err);
          }
        },
      );
    }),

    vscode.commands.registerCommand(
      "sessfind.openSession",
      async (sessionId?: string) => {
        const id = sessionId ?? (await pickSessionId(client));
        if (!id) {
          return;
        }
        const uri = SessionDocumentProvider.uriFor(id);
        const doc = await vscode.workspace.openTextDocument(uri);
        await vscode.languages.setTextDocumentLanguage(doc, "markdown");
        await vscode.window.showTextDocument(doc, { preview: true });
        await vscode.commands.executeCommand("markdown.showPreview", uri);
      },
    ),

    vscode.commands.registerCommand(
      "sessfind.resumeSession",
      async (item?: SessionItem) => {
        const session = item?.session;
        if (!session) {
          return;
        }
        try {
          await runCommandSpec(
            session.resume,
            `resume: ${session.title ?? session.session_id}`,
          );
        } catch (err) {
          reportError(err);
        }
      },
    ),

    vscode.commands.registerCommand(
      "sessfind.newSessionHere",
      async (item?: SessionItem) => {
        const session = item?.session;
        if (!session) {
          return;
        }
        try {
          await runCommandSpec(session.new_session, `new: ${session.project}`);
        } catch (err) {
          reportError(err);
        }
      },
    ),
  );

  // Compatibility gate: probe the binary once on activation.
  await checkCompatibility(client, refreshAll);
}

export function deactivate(): void {
  // Nothing to clean up; terminals and disposables are owned by VS Code.
}

async function checkCompatibility(
  client: SessfindClient,
  onReady: () => void,
): Promise<void> {
  let caps: Capabilities;
  try {
    caps = await client.capabilities();
  } catch (err) {
    if (err instanceof BinaryNotFoundError) {
      const choice = await vscode.window.showErrorMessage(
        "sessfind binary not found. Install it (cargo install sessfind) or set its path in settings.",
        "Open Settings",
      );
      if (choice === "Open Settings") {
        await vscode.commands.executeCommand(
          "workbench.action.openSettings",
          "sessfind.binaryPath",
        );
      }
    } else {
      // Non-zero exit or unparseable output → binary predates the JSON API.
      vscode.window.showErrorMessage(
        "sessfind does not support the JSON API. Please upgrade sessfind (>= 0.9).",
      );
    }
    return;
  }

  if (caps.json_api_version > SUPPORTED_JSON_API_VERSION) {
    vscode.window.showWarningMessage(
      `sessfind reports JSON API v${caps.json_api_version}, newer than this extension (v${SUPPORTED_JSON_API_VERSION}). Please update the extension.`,
    );
  }

  // Expose feature flags for menu `when` clauses (used by later PRs).
  await vscode.commands.executeCommand(
    "setContext",
    "sessfind.features",
    caps.features,
  );

  onReady();
}

async function pickSessionId(
  client: SessfindClient,
): Promise<string | undefined> {
  const sessions = await client.sessions();
  const pick = await vscode.window.showQuickPick(
    sessions.map((s) => ({
      label: s.title ?? s.session_id,
      description: `${s.source} · ${s.project}`,
      sessionId: s.session_id,
    })),
    { placeHolder: "Select a session to open" },
  );
  return pick?.sessionId;
}

function reportError(err: unknown): void {
  if (err instanceof SessfindError) {
    vscode.window.showErrorMessage(
      `sessfind: ${err.message}${err.stderr ? `\n${err.stderr}` : ""}`,
    );
  } else if (err instanceof BinaryNotFoundError) {
    vscode.window.showErrorMessage(err.message);
  } else {
    vscode.window.showErrorMessage(`sessfind: ${String(err)}`);
  }
}
