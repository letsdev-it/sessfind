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
  STATS_SCHEME,
  StatsDocumentProvider,
} from "./preview/statsDocumentProvider";
import {
  BinaryNotFoundError,
  SessfindClient,
  SessfindError,
} from "./sessfind/client";
import {
  SUPPORTED_JSON_API_VERSION,
  type SearchMethod,
  type SessionSummary,
  type Source,
} from "./sessfind/types";

const VIEW_MODE_KEY = "sessfind.projectsViewMode";
const REQUIRED_FEATURES = [
  "source-qualified-sessions",
  "source-freshness",
  "catalog-reconciliation",
  "session-grouped-search",
];

export function activate(context: vscode.ExtensionContext): void {
  const client = new SessfindClient();

  let viewMode: ViewMode =
    context.globalState.get<ViewMode>(VIEW_MODE_KEY) ?? "list";
  let filter: FilterPayload | null = null;
  let busy = false;
  let methods: SearchMethod[] = ["fts", "fuzzy"];
  let filterSeq = 0;
  let activeSearch: vscode.CancellationTokenSource | undefined;
  let searchError: string | null = null;
  let features: string[] = [];
  let defaultMethod: SearchMethod = "fts";

  const statsProvider = new StatsDocumentProvider(client);
  const projectProvider = new ProjectDocumentProvider(client);
  const sessionProvider = new SessionDocumentProvider(client);

  // ── State push ──

  const pushState = async () => {
    let sessions: SessionSummary[] = [];
    let projects = [] as Awaited<ReturnType<SessfindClient["projects"]>>;
    let error: string | null = null;
    const warnings: string[] = [];
    try {
      const caps = await client.capabilities();
      methods = availableMethods(caps);
      features = caps.features;
      const missingFeatures = REQUIRED_FEATURES.filter(
        (feature) => !features.includes(feature),
      );
      if (missingFeatures.length > 0) {
        throw new Error(
          `Incompatible sessfind binary; missing required capabilities: ${missingFeatures.join(", ")}.`,
        );
      }
      const configuredMethod = vscode.workspace
        .getConfiguration("sessfind")
        .get<string>("defaultSearchMethod");
      defaultMethod = methods.includes(configuredMethod as SearchMethod)
        ? (configuredMethod as SearchMethod)
        : "fts";
      if (
        configuredMethod &&
        !methods.includes(configuredMethod as SearchMethod)
      ) {
        warnings.push(
          `Configured search method '${configuredMethod}' is unavailable; using full-text for this activation.`,
        );
      }
      if (caps.json_api_version !== SUPPORTED_JSON_API_VERSION) {
        throw new Error(
          `Incompatible sessfind JSON API v${caps.json_api_version}; this extension requires v${SUPPORTED_JSON_API_VERSION}.`,
        );
      }
      const [loadedSessions, loadedProjects, stats] = await Promise.all([
        client.sessions(),
        client.projects(),
        client.stats(),
      ]);
      sessions = loadedSessions;
      projects = loadedProjects;
      for (const [source, freshness] of Object.entries(stats.sources ?? {})) {
        if (freshness?.status === "stale" || freshness?.status === "failed") {
          warnings.push(
            `${source} is ${freshness.status}; last successful sync: ${freshness.last_success ?? "never"}${freshness.error ? ` (${freshness.error})` : ""}.`,
          );
        }
      }
    } catch (err) {
      if (err instanceof BinaryNotFoundError) {
        error =
          "sessfind binary not found.\nInstall it (cargo install sessfind) or set “sessfind.binaryPath” in settings.";
      } else if (err instanceof SessfindError) {
        error = `sessfind failed:\n${err.stderr || err.message}\n\nInstall a sessfind version compatible with JSON API v${SUPPORTED_JSON_API_VERSION}.`;
      } else {
        error = String(err);
      }
    }
    hub.post({
      type: "state",
      state: {
        sessions,
        projects,
        methods,
        defaultMethod,
        features,
        viewMode,
        filter,
        busy,
        searchError,
        warnings,
        error,
      },
    });
  };

  const refresh = async () => {
    client.invalidate();
    await pushState();
  };

  const invalidateMutationPreviews = async (
    kind: "session" | "project",
    id: string,
    source: Source | undefined,
    label: string,
  ) => {
    if (kind === "session" && source) {
      sessionProvider.invalidate(id, source, label);
      const session = await findSession(id, source);
      if (session) {
        projectProvider.invalidate(session.project);
      }
    } else {
      projectProvider.invalidate(id);
    }
    statsProvider.invalidate();
  };

  // ── Filtering (engine search happens extension-side) ──

  const applyQuery = async (raw: string, method: SearchMethod) => {
    const query = raw.trim();
    const seq = ++filterSeq;
    activeSearch?.cancel();
    activeSearch?.dispose();
    activeSearch = undefined;
    if (query.length === 0) {
      filter = null;
      busy = false;
      searchError = null;
      await pushState();
      return;
    }
    const instant = method === "fts" || method === "fuzzy";
    filter = {
      query,
      engineIds: [],
      engineOnly: !instant,
      matches: [],
    };
    busy = true;
    searchError = null;
    const search = new vscode.CancellationTokenSource();
    activeSearch = search;
    await pushState();
    try {
      const configuredLimit = vscode.workspace
        .getConfiguration("sessfind")
        .get<number>("searchLimit", 50);
      const limit = Number.isFinite(configuredLimit)
        ? Math.max(1, Math.min(5000, Math.trunc(configuredLimit)))
        : 50;
      const results = await client.search(
        query,
        method,
        limit,
        search.token,
      );
      if (seq === filterSeq) {
        // Best (first-ranked) snippet per session, order preserved.
        const seen = new Set<string>();
        const matches = [];
        for (const r of results) {
          const key = `${r.source}:${r.session_id}`;
          if (!seen.has(key)) {
            seen.add(key);
            matches.push({ session_key: key, snippet: r.snippet });
          }
        }
        filter = {
          query,
          engineIds: [...seen],
          engineOnly: !instant,
          matches,
        };
      }
    } catch (err) {
      if (seq === filterSeq && !search.token.isCancellationRequested) {
        filter = { query, engineIds: [], engineOnly: !instant, matches: [] };
        searchError =
          err instanceof SessfindError
            ? err.stderr || err.message
            : String(err);
      }
    } finally {
      if (seq === filterSeq) {
        busy = false;
        if (activeSearch === search) {
          activeSearch = undefined;
        }
        await pushState();
      }
      search.dispose();
    }
  };

  // ── Hub message handling ──

  const findSession = async (
    sessionId: string,
    source: Source,
  ): Promise<SessionSummary | undefined> => {
    const sessions = await client.sessions();
    return sessions.find(
      (s) => s.session_id === sessionId && s.source === source,
    );
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
          SessionDocumentProvider.uriFor(
            msg.sessionId,
            msg.source,
            msg.title,
          ),
        );
        break;
      case "openProject":
        await openMarkdownPreview(ProjectDocumentProvider.uriForAuto(msg.path));
        break;
      case "resume": {
        const session = await findSession(msg.sessionId, msg.source);
        if (session) {
          await runCommandSpec(
            session.resume,
            `resume: ${session.title ?? session.session_id}`,
          );
        }
        break;
      }
      case "newSession": {
        const fallback =
          msg.sessionId && msg.source
            ? (await findSession(msg.sessionId, msg.source))?.new_session
            : undefined;
        await startNewSession(client, msg.dir, fallback);
        break;
      }
      case "rename": {
        const session = await findSession(msg.sessionId, msg.source);
        if (session && (await promptRename(client, session))) {
          sessionProvider.invalidate(
            session.session_id,
            session.source,
            session.title,
          );
          projectProvider.invalidate(session.project);
          statsProvider.invalidate();
          await pushState();
        }
        break;
      }
      case "addTag":
        if (
          await promptAddTag(
            client,
            msg.kind,
            msg.id,
            msg.label,
            msg.source,
          )
        ) {
          await invalidateMutationPreviews(
            msg.kind,
            msg.id,
            msg.source,
            msg.label,
          );
          await pushState();
        }
        break;
      case "removeTag":
        if (
          await promptRemoveTag(
            client,
            msg.kind,
            msg.id,
            msg.label,
            msg.tags,
            msg.source,
          )
        ) {
          await invalidateMutationPreviews(
            msg.kind,
            msg.id,
            msg.source,
            msg.label,
          );
          await pushState();
        }
        break;
      case "setViewMode":
        viewMode = msg.mode;
        await context.globalState.update(VIEW_MODE_KEY, msg.mode);
        await pushState();
        break;
      case "chat": {
        try {
          const tools = (await client.toolsList(msg.dir)).filter(
            (t) => t.chat_capable,
          );
          if (tools.length === 0) {
            vscode.window.showErrorMessage(
              "sessfind: no chat-capable AI CLI tools found (claude, opencode, codex).",
            );
            break;
          }
          let tool = tools[0].name;
          if (tools.length > 1) {
            const pick = await vscode.window.showQuickPick(
              tools.map((t) => t.name),
              { placeHolder: "Chat about this project with…" },
            );
            if (!pick) {
              break;
            }
            tool = pick;
          }
          const spec = await client.projectsChat(msg.dir, tool);
          await runCommandSpec(spec, `chat: ${msg.dir}`);
        } catch (err) {
          reportError(err);
        }
        break;
      }
      case "stats":
        statsProvider.invalidate();
        await openMarkdownPreview(StatsDocumentProvider.uri);
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
    new vscode.Disposable(() => {
      activeSearch?.cancel();
      activeSearch?.dispose();
    }),
    vscode.window.registerWebviewViewProvider(HubViewProvider.viewId, hub),
    vscode.workspace.registerTextDocumentContentProvider(
      SESSION_SCHEME,
      sessionProvider,
    ),
    vscode.workspace.registerTextDocumentContentProvider(
      PROJECT_SCHEME,
      projectProvider,
    ),
    vscode.workspace.registerTextDocumentContentProvider(
      STATS_SCHEME,
      statsProvider,
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
          source: s.source,
        })),
        { placeHolder: "Select a session to open" },
      );
      if (pick) {
        await openMarkdownPreview(
          SessionDocumentProvider.uriFor(pick.id, pick.source, pick.title),
        );
      }
    }),
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (event.affectsConfiguration("sessfind")) {
        client.invalidate();
        void pushState();
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
