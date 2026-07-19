import * as vscode from "vscode";
import { startNewSession } from "./commands/newSession";
import {
  promptAddTag,
  promptRemoveTag,
  promptRename,
} from "./commands/prompts";
import { availableMethods } from "./commands/searchHelpers";
import { runCommandSpec } from "./commands/terminal";
import { HubViewProvider } from "./hub/hubViewProvider";
import type { FilterPayload, ViewMode, WebToExt } from "./hub/protocol";
import {
  PROJECT_SCHEME,
  ProjectDocumentProvider,
} from "./preview/projectDocumentProvider";
import {
  SESSION_SCHEME,
  SessionDocumentProvider,
} from "./preview/sessionDocumentProvider";
import {
  BinaryNotFoundError,
  SessfindClient,
  SessfindError,
} from "./sessfind/client";
import {
  SUPPORTED_JSON_API_VERSION,
  type SearchMethod,
  type SessionSummary,
} from "./sessfind/types";

const VIEW_MODE_KEY = "sessfind.projectsViewMode";

export function activate(context: vscode.ExtensionContext): void {
  const client = new SessfindClient();

  let viewMode: ViewMode =
    context.globalState.get<ViewMode>(VIEW_MODE_KEY) ?? "list";
  let filter: FilterPayload | null = null;
  let busy = false;
  let methods: SearchMethod[] = ["fts", "fuzzy"];
  let filterSeq = 0;

  // ── State push ──

  const pushState = async () => {
    let sessions: SessionSummary[] = [];
    let projects = [] as Awaited<ReturnType<SessfindClient["projects"]>>;
    let error: string | null = null;
    try {
      const caps = await client.capabilities();
      methods = availableMethods(caps);
      if (caps.json_api_version > SUPPORTED_JSON_API_VERSION) {
        vscode.window.showWarningMessage(
          `sessfind reports JSON API v${caps.json_api_version}, newer than this extension (v${SUPPORTED_JSON_API_VERSION}). Please update the extension.`,
        );
      }
      [sessions, projects] = await Promise.all([
        client.sessions(),
        client.projects(),
      ]);
    } catch (err) {
      if (err instanceof BinaryNotFoundError) {
        error =
          "sessfind binary not found.\nInstall it (cargo install sessfind) or set “sessfind.binaryPath” in settings.";
      } else if (err instanceof SessfindError) {
        error = `sessfind failed:\n${err.stderr || err.message}\n\nIf your sessfind predates the JSON API, upgrade it (>= 0.9).`;
      } else {
        error = String(err);
      }
    }
    hub.post({
      type: "state",
      state: { sessions, projects, methods, viewMode, filter, busy, error },
    });
  };

  const refresh = async () => {
    client.invalidate();
    await pushState();
  };

  // ── Filtering (engine search happens extension-side) ──

  const applyQuery = async (raw: string, method: SearchMethod) => {
    const query = raw.trim();
    const seq = ++filterSeq;
    if (query.length === 0) {
      filter = null;
      busy = false;
      await pushState();
      return;
    }
    const instant = method === "fts" || method === "fuzzy";
    filter = instant ? { query, engineIds: [], engineOnly: false } : filter;
    busy = true;
    await pushState();
    try {
      const results = await client.search(query, method, 500);
      if (seq === filterSeq) {
        filter = {
          query,
          engineIds: [...new Set(results.map((r) => r.session_id))],
          engineOnly: !instant,
        };
      }
    } catch {
      if (seq === filterSeq && !instant) {
        filter = { query, engineIds: [], engineOnly: false };
      }
    } finally {
      if (seq === filterSeq) {
        busy = false;
        await pushState();
      }
    }
  };

  // ── Hub message handling ──

  const findSession = async (
    sessionId: string,
  ): Promise<SessionSummary | undefined> => {
    const sessions = await client.sessions();
    return sessions.find((s) => s.session_id === sessionId);
  };

  const handleMessage = async (msg: WebToExt): Promise<void> => {
    switch (msg.type) {
      case "ready":
        await pushState();
        break;
      case "query":
        await applyQuery(msg.value, msg.method);
        break;
      case "open":
        await openMarkdownPreview(
          SessionDocumentProvider.uriFor(msg.sessionId, msg.title),
        );
        break;
      case "openProject":
        await openMarkdownPreview(ProjectDocumentProvider.uriForAuto(msg.path));
        break;
      case "resume": {
        const session = await findSession(msg.sessionId);
        if (session) {
          await runCommandSpec(
            session.resume,
            `resume: ${session.title ?? session.session_id}`,
          );
        }
        break;
      }
      case "newSession": {
        const fallback = msg.sessionId
          ? (await findSession(msg.sessionId))?.new_session
          : undefined;
        await startNewSession(client, msg.dir, fallback);
        break;
      }
      case "rename": {
        const session = await findSession(msg.sessionId);
        if (session && (await promptRename(client, session))) {
          await pushState();
        }
        break;
      }
      case "addTag":
        if (await promptAddTag(client, msg.kind, msg.id, msg.label)) {
          await pushState();
        }
        break;
      case "removeTag":
        if (
          await promptRemoveTag(client, msg.kind, msg.id, msg.label, msg.tags)
        ) {
          await pushState();
        }
        break;
      case "setViewMode":
        viewMode = msg.mode;
        await context.globalState.update(VIEW_MODE_KEY, msg.mode);
        await pushState();
        break;
      case "refresh":
        await refresh();
        break;
      case "index":
        await vscode.window.withProgress(
          {
            location: vscode.ProgressLocation.Notification,
            title: "sessfind: indexing…",
          },
          async () => {
            try {
              await client.index();
              await refresh();
            } catch (err) {
              reportError(err);
            }
          },
        );
        break;
    }
  };

  const hub = new HubViewProvider(
    context.extensionUri,
    (msg) => void handleMessage(msg).catch(reportError),
    () => void pushState(),
  );

  context.subscriptions.push(
    vscode.window.registerWebviewViewProvider(HubViewProvider.viewId, hub),
    vscode.workspace.registerTextDocumentContentProvider(
      SESSION_SCHEME,
      new SessionDocumentProvider(client),
    ),
    vscode.workspace.registerTextDocumentContentProvider(
      PROJECT_SCHEME,
      new ProjectDocumentProvider(client),
    ),

    vscode.commands.registerCommand("sessfind.refresh", () => void refresh()),

    vscode.commands.registerCommand("sessfind.setFilter", () => hub.focus()),

    vscode.commands.registerCommand("sessfind.indexNow", () =>
      handleMessage({ type: "index" }),
    ),

    vscode.commands.registerCommand("sessfind.openSession", async () => {
      const sessions = await client.sessions().catch(() => []);
      const pick = await vscode.window.showQuickPick(
        sessions.map((s) => ({
          label: s.title ?? s.session_id,
          description: `${s.source} · ${s.project}`,
          id: s.session_id,
          title: s.title,
        })),
        { placeHolder: "Select a session to open" },
      );
      if (pick) {
        await openMarkdownPreview(
          SessionDocumentProvider.uriFor(pick.id, pick.title),
        );
      }
    }),
  );
}

export function deactivate(): void {
  // Nothing to clean up; terminals and disposables are owned by VS Code.
}

async function openMarkdownPreview(uri: vscode.Uri): Promise<void> {
  const doc = await vscode.workspace.openTextDocument(uri);
  await vscode.languages.setTextDocumentLanguage(doc, "markdown");
  await vscode.window.showTextDocument(doc, { preview: true });
  await vscode.commands.executeCommand("markdown.showPreview", uri);
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
