import * as vscode from "vscode";
import { registerManageCommands } from "./commands/manage";
import { startNewSession } from "./commands/newSession";
import { runSearch } from "./commands/search";
import { runCommandSpec } from "./commands/terminal";
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
import { SUPPORTED_JSON_API_VERSION, type Capabilities } from "./sessfind/types";
import type { SessionFilter } from "./state/filter";
import { AutoProjectsTreeProvider } from "./views/autoProjectsTree";
import {
  ProjectDirItem,
  ProjectGroupItem,
  SessionItem,
  UserProjectItem,
} from "./views/items";
import { SearchBoxViewProvider } from "./views/searchBoxView";
import { TagsTreeProvider } from "./views/tagsTree";
import { UserProjectsTreeProvider } from "./views/userProjectsTree";

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  const client = new SessfindClient();

  let activeFilter: SessionFilter | undefined;
  const getFilter = () => activeFilter;

  const autoProjects = new AutoProjectsTreeProvider(client, getFilter);
  const userProjects = new UserProjectsTreeProvider(client, getFilter);
  const tags = new TagsTreeProvider(client, getFilter);
  const previewProvider = new SessionDocumentProvider(client);
  const projectPreviewProvider = new ProjectDocumentProvider(client);

  const autoView = vscode.window.createTreeView("sessfind.autoProjects", {
    treeDataProvider: autoProjects,
  });
  const userView = vscode.window.createTreeView("sessfind.userProjects", {
    treeDataProvider: userProjects,
  });
  const tagsView = vscode.window.createTreeView("sessfind.tags", {
    treeDataProvider: tags,
  });

  context.subscriptions.push(
    autoView,
    userView,
    tagsView,
    vscode.workspace.registerTextDocumentContentProvider(
      SESSION_SCHEME,
      previewProvider,
    ),
    vscode.workspace.registerTextDocumentContentProvider(
      PROJECT_SCHEME,
      projectPreviewProvider,
    ),
  );

  const refreshAll = () => {
    client.invalidate();
    autoProjects.refresh();
    userProjects.refresh();
    tags.refresh();
  };

  const setFilter = async (filter: SessionFilter | undefined) => {
    activeFilter = filter;
    const badge = filter ? `filter: “${filter.query}”` : undefined;
    autoView.description = badge;
    userView.description = badge;
    tagsView.description = badge;
    await vscode.commands.executeCommand(
      "setContext",
      "sessfind.filterActive",
      filter !== undefined,
    );
    autoProjects.refresh();
    userProjects.refresh();
    tags.refresh();
  };

  // Queries from the search box: filter by substrings immediately, then
  // refine with engine full-content matches. A sequence counter drops results
  // of superseded queries (the user kept typing).
  let filterSeq = 0;
  const applyQuery = async (raw: string) => {
    const query = raw.trim();
    const seq = ++filterSeq;
    if (query.length === 0) {
      await setFilter(undefined);
      return;
    }
    await setFilter({ query, engineIds: new Set() });
    try {
      const results = await client.search(query, "fts", 500);
      if (seq === filterSeq) {
        await setFilter({
          query,
          engineIds: new Set(results.map((r) => r.session_id)),
        });
      }
    } catch {
      // Bad query syntax or engine failure — substring filtering stays active.
    }
  };

  const searchBox = new SearchBoxViewProvider((q) => void applyQuery(q));
  context.subscriptions.push(
    vscode.window.registerWebviewViewProvider(
      SearchBoxViewProvider.viewId,
      searchBox,
    ),
  );

  context.subscriptions.push(...registerManageCommands(client, refreshAll));

  context.subscriptions.push(
    vscode.commands.registerCommand("sessfind.search", () => runSearch(client)),

    vscode.commands.registerCommand("sessfind.refresh", refreshAll),

    vscode.commands.registerCommand("sessfind.setFilter", () => {
      searchBox.focus();
    }),

    vscode.commands.registerCommand("sessfind.clearFilter", async () => {
      searchBox.setValue("");
      await applyQuery("");
    }),

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
        await openMarkdownPreview(SessionDocumentProvider.uriFor(id));
      },
    ),

    vscode.commands.registerCommand(
      "sessfind.openProjectDetails",
      async (item?: ProjectGroupItem | UserProjectItem) => {
        if (item instanceof ProjectGroupItem) {
          await openMarkdownPreview(
            ProjectDocumentProvider.uriForAuto(item.group.path),
          );
        } else if (item instanceof UserProjectItem) {
          await openMarkdownPreview(
            ProjectDocumentProvider.uriForUser(item.project.name),
          );
        }
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
          await startNewSession(client, session.project, session.new_session);
        } catch (err) {
          reportError(err);
        }
      },
    ),

    vscode.commands.registerCommand(
      "sessfind.newSessionInProject",
      async (item?: ProjectGroupItem | UserProjectItem) => {
        const dir =
          item instanceof ProjectGroupItem
            ? item.group.path
            : item instanceof UserProjectItem
              ? item.project.root_dir
              : undefined;
        if (!dir) {
          return;
        }
        try {
          await startNewSession(client, dir);
        } catch (err) {
          reportError(err);
        }
      },
    ),

    vscode.commands.registerCommand(
      "sessfind.removeDirFromProject",
      async (item?: ProjectDirItem) => {
        if (!item || item.isRoot) {
          return;
        }
        try {
          await client.projectRemoveDir(item.projectName, item.dir);
          refreshAll();
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

async function openMarkdownPreview(uri: vscode.Uri): Promise<void> {
  const doc = await vscode.workspace.openTextDocument(uri);
  await vscode.languages.setTextDocumentLanguage(doc, "markdown");
  await vscode.window.showTextDocument(doc, { preview: true });
  await vscode.commands.executeCommand("markdown.showPreview", uri);
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
