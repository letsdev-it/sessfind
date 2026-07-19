import * as vscode from "vscode";
import { registerManageCommands } from "./commands/manage";
import { startNewSession } from "./commands/newSession";
import { availableMethods, preferredMethod } from "./commands/searchHelpers";
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
import {
  SUPPORTED_JSON_API_VERSION,
  type Capabilities,
  type SearchMethod,
} from "./sessfind/types";
import type { SessionFilter } from "./state/filter";
import {
  AutoProjectsTreeProvider,
  type ProjectsViewMode,
} from "./views/autoProjectsTree";
import { ProjectGroupItem, SessionItem } from "./views/items";
import { SearchBoxViewProvider } from "./views/searchBoxView";
import { TagsTreeProvider } from "./views/tagsTree";

const VIEW_MODE_KEY = "sessfind.projectsViewMode";

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  const client = new SessfindClient();

  let activeFilter: SessionFilter | undefined;
  const getFilter = () => activeFilter;

  let viewMode: ProjectsViewMode =
    context.globalState.get<ProjectsViewMode>(VIEW_MODE_KEY) ?? "list";

  const autoProjects = new AutoProjectsTreeProvider(
    client,
    getFilter,
    () => viewMode,
  );
  const tags = new TagsTreeProvider(client, getFilter);
  const previewProvider = new SessionDocumentProvider(client);
  const projectPreviewProvider = new ProjectDocumentProvider(client);

  const autoView = vscode.window.createTreeView("sessfind.autoProjects", {
    treeDataProvider: autoProjects,
  });
  const tagsView = vscode.window.createTreeView("sessfind.tags", {
    treeDataProvider: tags,
  });

  context.subscriptions.push(
    autoView,
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
    tags.refresh();
  };

  const setFilter = async (filter: SessionFilter | undefined) => {
    activeFilter = filter;
    const badge = filter ? `filter: “${filter.query}”` : undefined;
    autoView.description = badge;
    tagsView.description = badge;
    await vscode.commands.executeCommand(
      "setContext",
      "sessfind.filterActive",
      filter !== undefined,
    );
    autoProjects.refresh();
    tags.refresh();
  };

  const setViewMode = async (mode: ProjectsViewMode) => {
    viewMode = mode;
    await context.globalState.update(VIEW_MODE_KEY, mode);
    await vscode.commands.executeCommand(
      "setContext",
      "sessfind.projectsTreeMode",
      mode === "tree",
    );
    autoProjects.refresh();
  };
  await vscode.commands.executeCommand(
    "setContext",
    "sessfind.projectsTreeMode",
    viewMode === "tree",
  );

  // Queries from the search box. Instant methods (fts/fuzzy): filter by
  // substrings immediately, then refine with engine full-content matches.
  // Deferred methods (semantic/llm): engine matches only, once they arrive.
  // A sequence counter drops results of superseded queries.
  let filterSeq = 0;
  const applyQuery = async (raw: string, method: SearchMethod = "fts") => {
    const query = raw.trim();
    const seq = ++filterSeq;
    if (query.length === 0) {
      searchBox.setBusy(false);
      await setFilter(undefined);
      return;
    }
    const instant = method === "fts" || method === "fuzzy";
    if (instant) {
      await setFilter({ query, engineIds: new Set() });
    }
    searchBox.setBusy(true);
    try {
      const results = await client.search(query, method, 500);
      if (seq === filterSeq) {
        await setFilter({
          query,
          engineIds: new Set(results.map((r) => r.session_id)),
          engineOnly: !instant,
        });
      }
    } catch {
      // Bad query syntax or engine failure — for instant methods substring
      // filtering stays active; for deferred ones fall back to substrings.
      if (seq === filterSeq && !instant) {
        await setFilter({ query, engineIds: new Set() });
      }
    } finally {
      if (seq === filterSeq) {
        searchBox.setBusy(false);
      }
    }
  };

  const searchBox = new SearchBoxViewProvider(
    (q, method) => void applyQuery(q, method),
  );
  context.subscriptions.push(
    vscode.window.registerWebviewViewProvider(
      SearchBoxViewProvider.viewId,
      searchBox,
    ),
  );

  context.subscriptions.push(...registerManageCommands(client, refreshAll));

  context.subscriptions.push(
    vscode.commands.registerCommand("sessfind.refresh", refreshAll),

    vscode.commands.registerCommand("sessfind.setFilter", () => {
      searchBox.focus();
    }),

    vscode.commands.registerCommand("sessfind.clearFilter", async () => {
      searchBox.setValue("");
      await applyQuery("");
    }),

    vscode.commands.registerCommand("sessfind.projectsAsTree", () =>
      setViewMode("tree"),
    ),
    vscode.commands.registerCommand("sessfind.projectsAsList", () =>
      setViewMode("list"),
    ),

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
      async (sessionId?: string, title?: string | null) => {
        if (!sessionId) {
          const picked = await pickSession(client);
          if (!picked) {
            return;
          }
          sessionId = picked.id;
          title = picked.title;
        }
        await openMarkdownPreview(
          SessionDocumentProvider.uriFor(sessionId, title),
        );
      },
    ),

    vscode.commands.registerCommand(
      "sessfind.openProjectDetails",
      async (item?: ProjectGroupItem) => {
        if (item instanceof ProjectGroupItem) {
          await openMarkdownPreview(
            ProjectDocumentProvider.uriForAuto(item.group.path),
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
      async (item?: ProjectGroupItem) => {
        if (!(item instanceof ProjectGroupItem)) {
          return;
        }
        try {
          await startNewSession(client, item.group.path);
        } catch (err) {
          reportError(err);
        }
      },
    ),
  );

  // Compatibility gate: probe the binary once on activation.
  const caps = await checkCompatibility(client);
  if (caps) {
    const methods = availableMethods(caps);
    const config = vscode.workspace.getConfiguration("sessfind");
    searchBox.setMethods(
      methods,
      preferredMethod(config.get<string>("defaultSearchMethod"), methods),
    );
    refreshAll();
  }
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
): Promise<Capabilities | undefined> {
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
    return undefined;
  }

  if (caps.json_api_version > SUPPORTED_JSON_API_VERSION) {
    vscode.window.showWarningMessage(
      `sessfind reports JSON API v${caps.json_api_version}, newer than this extension (v${SUPPORTED_JSON_API_VERSION}). Please update the extension.`,
    );
  }

  // Expose feature flags for menu `when` clauses.
  await vscode.commands.executeCommand(
    "setContext",
    "sessfind.features",
    caps.features,
  );

  return caps;
}

async function pickSession(
  client: SessfindClient,
): Promise<{ id: string; title: string | null } | undefined> {
  const sessions = await client.sessions();
  const pick = await vscode.window.showQuickPick(
    sessions.map((s) => ({
      label: s.title ?? s.session_id,
      description: `${s.source} · ${s.project}`,
      id: s.session_id,
      title: s.title,
    })),
    { placeHolder: "Select a session to open" },
  );
  return pick ? { id: pick.id, title: pick.title } : undefined;
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
