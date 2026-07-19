import * as vscode from "vscode";

/**
 * A persistent search input at the top of the sidebar (tree views cannot embed
 * inputs, so this is a small WebviewView styled with VS Code theme variables).
 * Typing emits debounced queries that drive the shared session filter.
 */
export class SearchBoxViewProvider implements vscode.WebviewViewProvider {
  static readonly viewId = "sessfind.searchBox";

  private view: vscode.WebviewView | undefined;

  constructor(private readonly onQuery: (query: string) => void) {}

  resolveWebviewView(view: vscode.WebviewView): void {
    this.view = view;
    view.webview.options = { enableScripts: true };
    view.webview.html = this.html();
    view.webview.onDidReceiveMessage((msg: { type: string; value?: string }) => {
      if (msg.type === "query") {
        this.onQuery(msg.value ?? "");
      }
    });
  }

  /** Push a value into the input (keeps it in sync with external changes). */
  setValue(value: string): void {
    void this.view?.webview.postMessage({ type: "setValue", value });
  }

  /** Reveal the view and focus the input. */
  focus(): void {
    this.view?.show?.(true);
    void this.view?.webview.postMessage({ type: "focus" });
  }

  private html(): string {
    const nonce = getNonce();
    return /* html */ `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta http-equiv="Content-Security-Policy"
      content="default-src 'none'; style-src 'nonce-${nonce}'; script-src 'nonce-${nonce}';">
<style nonce="${nonce}">
  html, body {
    padding: 0;
    margin: 0;
    background: transparent;
  }
  .wrap {
    display: flex;
    align-items: center;
    margin: 6px 8px;
    background: var(--vscode-input-background);
    border: 1px solid var(--vscode-input-border, transparent);
    border-radius: 4px;
  }
  .wrap:focus-within {
    border-color: var(--vscode-focusBorder);
  }
  input {
    flex: 1;
    min-width: 0;
    background: transparent;
    color: var(--vscode-input-foreground);
    border: none;
    outline: none;
    padding: 5px 8px;
    font-family: var(--vscode-font-family);
    font-size: var(--vscode-font-size);
  }
  input::placeholder {
    color: var(--vscode-input-placeholderForeground);
  }
  .clear {
    display: none;
    border: none;
    background: transparent;
    color: var(--vscode-input-foreground);
    cursor: pointer;
    padding: 0 8px;
    font-size: 14px;
    line-height: 1;
  }
  .wrap.has-value .clear {
    display: block;
  }
</style>
</head>
<body>
  <div class="wrap" id="wrap">
    <input id="q" type="text" placeholder="Search sessions…" />
    <button class="clear" id="clear" title="Clear">✕</button>
  </div>
  <script nonce="${nonce}">
    const vscode = acquireVsCodeApi();
    const input = document.getElementById("q");
    const wrap = document.getElementById("wrap");
    const clear = document.getElementById("clear");
    let timer;

    function send(value) {
      vscode.postMessage({ type: "query", value });
    }
    function sync() {
      wrap.classList.toggle("has-value", input.value.length > 0);
    }

    input.addEventListener("input", () => {
      sync();
      clearTimeout(timer);
      timer = setTimeout(() => send(input.value), 250);
    });
    input.addEventListener("keydown", (e) => {
      if (e.key === "Escape") {
        input.value = "";
        sync();
        clearTimeout(timer);
        send("");
      } else if (e.key === "Enter") {
        clearTimeout(timer);
        send(input.value);
      }
    });
    clear.addEventListener("click", () => {
      input.value = "";
      sync();
      clearTimeout(timer);
      send("");
      input.focus();
    });

    window.addEventListener("message", (event) => {
      const msg = event.data;
      if (msg.type === "setValue") {
        input.value = msg.value ?? "";
        sync();
      } else if (msg.type === "focus") {
        input.focus();
      }
    });
  </script>
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
