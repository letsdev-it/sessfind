import * as vscode from "vscode";
import type { ExtToWeb, WebToExt } from "./protocol";

/**
 * Hosts the hub webview — the entire sidebar UI lives in one WebviewView
 * (single view in the container, so VS Code renders it full-height with no
 * section splits). All logic stays in the extension; the webview renders
 * state and reports intents.
 */
export class HubViewProvider implements vscode.WebviewViewProvider {
  static readonly viewId = "sessfind.hub";

  private view: vscode.WebviewView | undefined;

  constructor(
    private readonly extensionUri: vscode.Uri,
    private readonly onMessage: (msg: WebToExt) => void,
    private readonly onVisible: () => void,
  ) {}

  resolveWebviewView(view: vscode.WebviewView): void {
    this.view = view;
    view.webview.options = {
      enableScripts: true,
      localResourceRoots: [vscode.Uri.joinPath(this.extensionUri, "dist")],
    };
    view.webview.html = this.html(view.webview);
    view.webview.onDidReceiveMessage((msg: WebToExt) => this.onMessage(msg));
    view.onDidChangeVisibility(() => {
      if (view.visible) {
        this.onVisible();
      }
    });
  }

  post(msg: ExtToWeb): void {
    void this.view?.webview.postMessage(msg);
  }

  focus(): void {
    this.view?.show?.(true);
    this.post({ type: "focus" });
  }

  private html(webview: vscode.Webview): string {
    const script = webview.asWebviewUri(
      vscode.Uri.joinPath(this.extensionUri, "dist", "hub.js"),
    );
    const style = webview.asWebviewUri(
      vscode.Uri.joinPath(this.extensionUri, "dist", "hub.css"),
    );
    const nonce = getNonce();
    return `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta http-equiv="Content-Security-Policy"
      content="default-src 'none'; style-src ${webview.cspSource}; script-src 'nonce-${nonce}';">
<link rel="stylesheet" href="${style}">
</head>
<body>
  <div id="root"></div>
  <script nonce="${nonce}" src="${script}"></script>
</body>
</html>`;
  }
}

function getNonce(): string {
  let text = "";
  const possible =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  for (let i = 0; i < 32; i++) {
    text += possible.charAt(Math.floor(Math.random() * possible.length));
  }
  return text;
}
