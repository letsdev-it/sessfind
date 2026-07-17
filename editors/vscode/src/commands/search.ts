import * as vscode from "vscode";
import { SessfindClient, SessfindError } from "../sessfind/client";
import { SessionDocumentProvider } from "../preview/sessionDocumentProvider";
import type { Capabilities, SearchResult } from "../sessfind/types";
import {
  availableMethods,
  firstLine,
  isInstant,
  methodLabel,
  nextMethod,
  preferredMethod,
  projectName,
} from "./searchHelpers";

const DEBOUNCE_MS = 300;

interface ResultPick extends vscode.QuickPickItem {
  sessionId?: string;
}

/**
 * Interactive search. A QuickPick whose title shows the current method; a
 * button cycles through the methods the binary actually supports. Instant
 * methods (fts/fuzzy) search as you type, debounced and cancellable; deferred
 * methods (semantic/llm) only run on Enter, since they can take seconds.
 */
export async function runSearch(client: SessfindClient): Promise<void> {
  let caps: Capabilities;
  try {
    caps = await client.capabilities();
  } catch (err) {
    reportError(err);
    return;
  }

  const methods = availableMethods(caps);
  const config = vscode.workspace.getConfiguration("sessfind");
  const limit = config.get<number>("searchLimit") ?? 50;
  let method = preferredMethod(config.get<string>("defaultSearchMethod"), methods);

  const qp = vscode.window.createQuickPick<ResultPick>();
  qp.matchOnDescription = true;

  const cycleButton: vscode.QuickInputButton = {
    iconPath: new vscode.ThemeIcon("arrow-swap"),
    tooltip: "Change search method",
  };
  if (methods.length > 1) {
    qp.buttons = [cycleButton];
  }

  let debounceTimer: ReturnType<typeof setTimeout> | undefined;
  let inFlight: vscode.CancellationTokenSource | undefined;

  const applyTitle = () => {
    qp.title = `Search sessions — ${methodLabel(method)}`;
    qp.placeholder = isInstant(method)
      ? "Type to search…"
      : `Type a query and press Enter to run ${methodLabel(method)} search`;
  };

  const doSearch = async () => {
    const query = qp.value.trim();
    if (!query) {
      qp.items = [];
      return;
    }
    inFlight?.cancel();
    inFlight = new vscode.CancellationTokenSource();
    const token = inFlight.token;
    qp.busy = true;
    try {
      const results = await client.search(query, method, limit, token);
      if (!token.isCancellationRequested) {
        qp.items = toItems(results);
      }
    } catch (err) {
      if (!token.isCancellationRequested) {
        qp.items = [];
        reportError(err);
      }
    } finally {
      if (!token.isCancellationRequested) {
        qp.busy = false;
      }
    }
  };

  qp.onDidChangeValue(() => {
    if (!isInstant(method)) {
      return; // deferred methods run on accept only
    }
    if (debounceTimer) {
      clearTimeout(debounceTimer);
    }
    debounceTimer = setTimeout(doSearch, DEBOUNCE_MS);
  });

  qp.onDidTriggerButton((button) => {
    if (button === cycleButton) {
      method = nextMethod(method, methods);
      applyTitle();
      if (isInstant(method)) {
        void doSearch();
      } else {
        qp.items = [];
      }
    }
  });

  qp.onDidAccept(async () => {
    const active = qp.activeItems[0];
    if (active?.sessionId) {
      qp.hide();
      await openSession(active.sessionId);
      return;
    }
    // No selection yet + deferred method → run the search now.
    if (!isInstant(method) && qp.value.trim()) {
      await vscode.window.withProgress(
        {
          location: vscode.ProgressLocation.Notification,
          title: `sessfind: ${methodLabel(method)} search…`,
        },
        doSearch,
      );
    }
  });

  qp.onDidHide(() => {
    inFlight?.cancel();
    if (debounceTimer) {
      clearTimeout(debounceTimer);
    }
    qp.dispose();
  });

  applyTitle();
  qp.show();
}

function toItems(results: SearchResult[]): ResultPick[] {
  return results.map((r) => ({
    label: r.title ?? firstLine(r.snippet),
    description: `${r.source} · ${projectName(r.project)}`,
    detail: firstLine(r.snippet),
    sessionId: r.session_id,
  }));
}

async function openSession(sessionId: string): Promise<void> {
  const uri = SessionDocumentProvider.uriFor(sessionId);
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
  } else {
    vscode.window.showErrorMessage(`sessfind: ${String(err)}`);
  }
}
