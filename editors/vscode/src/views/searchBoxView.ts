import * as vscode from "vscode";
import type { SearchMethod } from "../sessfind/types";

/**
 * A persistent search input at the top of the sidebar (tree views cannot embed
 * inputs, so this is a small WebviewView styled with VS Code theme variables),
 * with chips to switch the search method. Instant methods (fts/fuzzy) emit
 * debounced queries as you type; deferred methods (semantic/llm) emit only on
 * Enter, since they can take seconds.
 */
export class SearchBoxViewProvider implements vscode.WebviewViewProvider {
  static readonly viewId = "sessfind.searchBox";

  private view: vscode.WebviewView | undefined;
  private pendingMethods:
    | { methods: SearchMethod[]; active: SearchMethod }
    | undefined;

  constructor(
    private readonly onQuery: (query: string, method: SearchMethod) => void,
  ) {}

  resolveWebviewView(view: vscode.WebviewView): void {
    this.view = view;
    view.webview.options = { enableScripts: true };
    view.webview.html = this.html();
    view.webview.onDidReceiveMessage(
      (msg: { type: string; value?: string; method?: SearchMethod }) => {
        if (msg.type === "query") {
          this.onQuery(msg.value ?? "", msg.method ?? "fts");
        }
      },
    );
    if (this.pendingMethods) {
      void view.webview.postMessage({
        type: "methods",
        ...this.pendingMethods,
      });
    }
  }

  /** Advertise the methods the binary supports (from capabilities). */
  setMethods(methods: SearchMethod[], active: SearchMethod): void {
    this.pendingMethods = { methods, active };
    void this.view?.webview.postMessage({ type: "methods", methods, active });
  }

  /** Push a value into the input (keeps it in sync with external changes). */
  setValue(value: string): void {
    void this.view?.webview.postMessage({ type: "setValue", value });
  }

  setBusy(busy: boolean): void {
    void this.view?.webview.postMessage({ type: "busy", value: busy });
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
    overflow: hidden;
  }
  .wrap {
    display: flex;
    align-items: center;
    margin: 4px 8px;
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
  .modes {
    display: flex;
    align-items: center;
    gap: 4px;
    margin: 0 8px 4px;
  }
  .chip {
    border: none;
    border-radius: 3px;
    padding: 2px 8px;
    font-size: 11px;
    cursor: pointer;
    background: var(--vscode-button-secondaryBackground, rgba(128,128,128,0.2));
    color: var(--vscode-button-secondaryForeground, var(--vscode-foreground));
  }
  .chip.active {
    background: var(--vscode-button-background);
    color: var(--vscode-button-foreground);
  }
  .status {
    margin-left: auto;
    font-size: 11px;
    color: var(--vscode-descriptionForeground);
    white-space: nowrap;
  }
</style>
</head>
<body>
  <div class="wrap" id="wrap">
    <input id="q" type="text" placeholder="Search sessions…" />
    <button class="clear" id="clear" title="Clear">✕</button>
  </div>
  <div class="modes" id="modes"></div>
  <script nonce="${nonce}">
    const vscode = acquireVsCodeApi();
    const input = document.getElementById("q");
    const wrap = document.getElementById("wrap");
    const clear = document.getElementById("clear");
    const modesEl = document.getElementById("modes");

    const LABELS = { fts: "FTS", fuzzy: "Fuzzy", semantic: "Semantic", llm: "LLM" };
    const INSTANT = new Set(["fts", "fuzzy"]);

    let methods = ["fts", "fuzzy"];
    let method = (vscode.getState() && vscode.getState().method) || "fts";
    let busy = false;
    let timer;

    function isInstant() { return INSTANT.has(method); }

    function send(value) {
      vscode.postMessage({ type: "query", value, method });
    }

    function status() {
      if (busy) return "searching…";
      if (!isInstant() && input.value.trim().length > 0) return "Enter ↵ to search";
      return "";
    }

    function render() {
      wrap.classList.toggle("has-value", input.value.length > 0);
      modesEl.innerHTML = "";
      for (const m of methods) {
        const b = document.createElement("button");
        b.className = "chip" + (m === method ? " active" : "");
        b.textContent = LABELS[m] || m;
        b.title = "Search method: " + (LABELS[m] || m);
        b.addEventListener("click", () => {
          method = m;
          vscode.setState({ method });
          clearTimeout(timer);
          if (input.value.trim().length > 0 && isInstant()) {
            send(input.value);
          }
          render();
          input.focus();
        });
        modesEl.appendChild(b);
      }
      const s = document.createElement("span");
      s.className = "status";
      s.textContent = status();
      modesEl.appendChild(s);
    }

    input.addEventListener("input", () => {
      clearTimeout(timer);
      if (isInstant()) {
        timer = setTimeout(() => send(input.value), 250);
      } else if (input.value.trim().length === 0) {
        send("");
      }
      render();
    });
    input.addEventListener("keydown", (e) => {
      if (e.key === "Escape") {
        input.value = "";
        clearTimeout(timer);
        send("");
        render();
      } else if (e.key === "Enter") {
        clearTimeout(timer);
        send(input.value);
      }
    });
    clear.addEventListener("click", () => {
      input.value = "";
      clearTimeout(timer);
      send("");
      render();
      input.focus();
    });

    window.addEventListener("message", (event) => {
      const msg = event.data;
      if (msg.type === "setValue") {
        input.value = msg.value ?? "";
        render();
      } else if (msg.type === "focus") {
        input.focus();
      } else if (msg.type === "methods") {
        methods = msg.methods;
        if (!methods.includes(method)) {
          method = msg.active;
        }
        vscode.setState({ method });
        render();
      } else if (msg.type === "busy") {
        busy = !!msg.value;
        render();
      }
    });

    render();
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
