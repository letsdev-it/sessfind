// Webview script for the sessfind hub. Renders the full sidebar (search,
// projects, tags) from state pushed by the extension, and sends user intents
// back. Rendering is plain DOM — the lists are moderate (hundreds of rows).

import "./hub.css";
import {
  buildModel,
  type HubModel,
  type ProjectEntry,
  type ProjectNode,
} from "../model";
import type { ExtToWeb, HubState, WebToExt } from "../protocol";
import type { SearchMethod, SessionSummary } from "../../sessfind/types";

declare function acquireVsCodeApi(): {
  postMessage(msg: WebToExt): void;
  getState(): { method?: SearchMethod; expanded?: string[] } | undefined;
  setState(state: { method?: SearchMethod; expanded?: string[] }): void;
};

const vscode = acquireVsCodeApi();

const INSTANT: SearchMethod[] = ["fts", "fuzzy"];
const LABELS: Record<SearchMethod, string> = {
  fts: "FTS",
  fuzzy: "Fuzzy",
  semantic: "Semantic",
  llm: "LLM",
};

// ── Local UI state ──

let state: HubState | null = null;
let method: SearchMethod = vscode.getState()?.method ?? "fts";
let expanded = new Set<string>(vscode.getState()?.expanded ?? []);
let debounce: ReturnType<typeof setTimeout> | undefined;

function persist(): void {
  vscode.setState({ method, expanded: [...expanded] });
}

function send(msg: WebToExt): void {
  vscode.postMessage(msg);
}

// ── Icons (inline SVG, stroke follows currentColor) ──

const svg = (paths: string, viewBox = "0 0 16 16"): string =>
  `<svg viewBox="${viewBox}" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round">${paths}</svg>`;

const ICONS = {
  search: svg('<circle cx="7" cy="7" r="4.5"/><line x1="10.5" y1="10.5" x2="14" y2="14"/>'),
  play: svg('<path d="M5 3.5v9l7-4.5z" fill="currentColor" stroke="none"/>'),
  plus: svg('<line x1="8" y1="3.5" x2="8" y2="12.5"/><line x1="3.5" y1="8" x2="12.5" y2="8"/>'),
  edit: svg('<path d="M11.5 2.5l2 2L6 12l-2.7.7L4 10z"/>'),
  tag: svg('<path d="M8.6 2.2H13.8V7.4L7.4 13.8 2.2 8.6z"/><circle cx="10.8" cy="5.2" r="0.9" fill="currentColor" stroke="none"/>'),
  untag: svg('<path d="M8.6 2.2H13.8V7.4L7.4 13.8 2.2 8.6z"/><line x1="5" y1="9" x2="9" y2="13"/>'),
  info: svg('<circle cx="8" cy="8" r="5.7"/><line x1="8" y1="7.2" x2="8" y2="11"/><circle cx="8" cy="5" r="0.7" fill="currentColor" stroke="none"/>'),
  chevron: svg('<path d="M6 3.5 10.5 8 6 12.5"/>'),
  folder: svg('<path d="M1.8 4.2c0-.6.4-1 1-1h3.4l1.4 1.6h5.6c.6 0 1 .4 1 1v6c0 .6-.4 1-1 1H2.8c-.6 0-1-.4-1-1z"/>'),
  listTree: svg('<line x1="3" y1="4" x2="7" y2="4"/><line x1="6" y1="8" x2="10" y2="8"/><line x1="9" y1="12" x2="13" y2="12"/>'),
  listFlat: svg('<line x1="3" y1="4" x2="13" y2="4"/><line x1="3" y1="8" x2="13" y2="8"/><line x1="3" y1="12" x2="13" y2="12"/>'),
  refresh: svg('<path d="M13 8a5 5 0 1 1-1.5-3.5"/><path d="M13 2.8v2.7h-2.7"/>'),
  db: svg('<ellipse cx="8" cy="4" rx="5" ry="2"/><path d="M3 4v8c0 1.1 2.2 2 5 2s5-.9 5-2V4"/><path d="M3 8c0 1.1 2.2 2 5 2s5-.9 5-2"/>'),
  sparkle: svg('<path d="M8 2l1.2 3.6L13 7l-3.8 1.4L8 12l-1.2-3.6L3 7l3.8-1.4z"/><path d="M12.8 11l.5 1.5 1.5.5-1.5.5-.5 1.5-.5-1.5-1.5-.5 1.5-.5z"/>'),
  chat: svg('<path d="M2.5 3.5h11v7h-6l-3 2.5v-2.5h-2z"/>'),
  chart: svg('<line x1="3" y1="13" x2="13" y2="13"/><line x1="4.5" y1="13" x2="4.5" y2="8"/><line x1="8" y1="13" x2="8" y2="4"/><line x1="11.5" y1="13" x2="11.5" y2="10"/>'),
};

// ── DOM helpers ──

function el(
  tag: string,
  className?: string,
  html?: string,
): HTMLElement {
  const node = document.createElement(tag);
  if (className) {
    node.className = className;
  }
  if (html !== undefined) {
    node.innerHTML = html;
  }
  return node;
}

function iconBtn(
  icon: string,
  title: string,
  onClick: (e: MouseEvent) => void,
): HTMLElement {
  const btn = el("button", "iconbtn", icon) as HTMLButtonElement;
  btn.title = title;
  btn.addEventListener("click", (e) => {
    e.stopPropagation();
    onClick(e);
  });
  return btn;
}

function relTime(iso: string): string {
  const then = new Date(iso).getTime();
  if (Number.isNaN(then)) {
    return "";
  }
  const mins = Math.max(0, Math.round((Date.now() - then) / 60000));
  if (mins < 60) {
    return `${mins}m`;
  }
  const hours = Math.round(mins / 60);
  if (hours < 24) {
    return `${hours}h`;
  }
  const days = Math.round(hours / 24);
  if (days < 30) {
    return `${days}d`;
  }
  return new Date(iso).toISOString().slice(0, 10);
}

// ── Rendering ──

const root = document.getElementById("root") as HTMLElement;
let searchInput: HTMLInputElement | null = null;

function isInstant(): boolean {
  return INSTANT.includes(method);
}

function sendQuery(value: string): void {
  send({ type: "query", value, method });
}

function render(): void {
  const focusHadSearch = document.activeElement === searchInput;
  const inputValue = searchInput?.value ?? "";
  root.textContent = "";

  root.appendChild(renderSearch(inputValue));

  if (!state) {
    root.appendChild(el("div", "empty", "Loading…"));
    return;
  }
  if (state.error) {
    root.appendChild(el("div", "error")).textContent = state.error;
    return;
  }

  const model = buildModel(state);
  root.appendChild(renderProjectsSection(model));
  root.appendChild(renderTagsSection(model));

  if (focusHadSearch) {
    searchInput?.focus();
  }
}

function renderSearch(value: string): HTMLElement {
  const wrap = el("div", "search");
  const box = el("div", "box" + (value ? " has-value" : ""));

  box.appendChild(el("span", "icon", ICONS.search));

  const input = el("input") as HTMLInputElement;
  input.type = "text";
  input.placeholder = "Search sessions…";
  input.value = value;
  input.addEventListener("input", () => {
    box.classList.toggle("has-value", input.value.length > 0);
    clearTimeout(debounce);
    if (isInstant()) {
      debounce = setTimeout(() => sendQuery(input.value), 250);
    } else if (input.value.trim().length === 0) {
      sendQuery("");
    }
    renderStatus();
  });
  input.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      input.value = "";
      clearTimeout(debounce);
      sendQuery("");
    } else if (e.key === "Enter") {
      clearTimeout(debounce);
      sendQuery(input.value);
    }
  });
  box.appendChild(input);
  searchInput = input;

  const clear = el("button", "clear", "✕") as HTMLButtonElement;
  clear.title = "Clear";
  clear.addEventListener("click", () => {
    input.value = "";
    clearTimeout(debounce);
    sendQuery("");
    input.focus();
  });
  box.appendChild(clear);
  wrap.appendChild(box);

  const modes = el("div", "modes");
  for (const m of state?.methods ?? (["fts", "fuzzy"] as SearchMethod[])) {
    const chip = el(
      "button",
      "chip" + (m === method ? " active" : ""),
      LABELS[m],
    ) as HTMLButtonElement;
    chip.title = `Search method: ${LABELS[m]}`;
    chip.addEventListener("click", () => {
      method = m;
      persist();
      clearTimeout(debounce);
      const q = searchInput?.value.trim() ?? "";
      if (q && isInstant()) {
        sendQuery(q);
      }
      render();
      searchInput?.focus();
    });
    modes.appendChild(chip);
  }
  wrap.appendChild(modes);

  const status = el("div", "statusline");
  status.id = "statusline";
  wrap.appendChild(status);
  queueMicrotask(renderStatus);

  return wrap;
}

function renderStatus(): void {
  const node = document.getElementById("statusline");
  if (!node) {
    return;
  }
  const q = searchInput?.value.trim() ?? "";
  if (state?.busy) {
    node.textContent = "searching…";
  } else if (!isInstant() && q.length > 0) {
    node.textContent = "Enter ↵ to search";
  } else if (state?.filter && state.filter.query.length > 0) {
    const model = state ? buildModel(state) : null;
    node.textContent = model
      ? `${model.visibleSessions} of ${model.totalSessions} sessions`
      : "";
  } else {
    node.textContent = "";
  }
}

function sectionHeader(
  title: string,
  count: string,
  tools: HTMLElement[],
): { header: HTMLElement; isOpen: () => boolean } {
  const key = `section:${title}`;
  const open = () => !expanded.has(`${key}:closed`);
  const header = el("div", "section-header");
  const twisty = el("span", "twisty", ICONS.chevron);
  twisty.style.transform = open() ? "rotate(90deg)" : "";
  header.appendChild(twisty);
  header.appendChild(el("span", undefined, title));
  header.appendChild(el("span", "count", count));
  header.appendChild(el("span", "spacer"));
  const toolbox = el("span", "tools");
  for (const t of tools) {
    toolbox.appendChild(t);
  }
  header.appendChild(toolbox);
  header.addEventListener("click", () => {
    if (open()) {
      expanded.add(`${key}:closed`);
    } else {
      expanded.delete(`${key}:closed`);
    }
    persist();
    render();
  });
  return { header, isOpen: open };
}

function renderProjectsSection(model: HubModel): HTMLElement {
  const section = el("div", "section");
  const tree = state?.viewMode === "tree";
  const tools = [
    iconBtn(
      tree ? ICONS.listFlat : ICONS.listTree,
      tree ? "View as flat list" : "View as directory tree",
      () => send({ type: "setViewMode", mode: tree ? "list" : "tree" }),
    ),
    iconBtn(ICONS.chart, "Statistics", () => send({ type: "stats" })),
    iconBtn(ICONS.db, "Refresh index", () => send({ type: "index" })),
    iconBtn(ICONS.refresh, "Refresh", () => send({ type: "refresh" })),
  ];
  const { header, isOpen } = sectionHeader(
    "Projects",
    String(model.projects.length === 0 ? "" : countProjects(model.projects)),
    tools,
  );
  section.appendChild(header);
  if (!isOpen()) {
    return section;
  }

  if (model.projects.length === 0) {
    section.appendChild(
      el(
        "div",
        "empty",
        model.filterActive
          ? "No sessions match the filter."
          : "No indexed sessions yet. Hit the database icon to index.",
      ),
    );
    return section;
  }
  for (const node of model.projects) {
    section.appendChild(renderProjectNode(node));
  }
  return section;
}

function countProjects(nodes: ProjectNode[]): number {
  let n = 0;
  for (const node of nodes) {
    if (node.kind === "project") {
      n += 1;
    } else {
      n += node.here.length + countProjects(node.children);
    }
  }
  return n;
}

function renderProjectNode(node: ProjectNode): HTMLElement {
  if (node.kind === "project") {
    return renderProjectEntry(node.entry, node.label);
  }

  const wrap = el("div");
  const key = `dir:${node.path}`;
  const isOpen = expanded.has(key);
  const row = el("div", "row dir" + (isOpen ? " expanded" : ""));
  row.appendChild(el("span", "twisty", ICONS.chevron));
  row.appendChild(el("span", "icon", ICONS.folder));
  const label = el("span", "label grow");
  label.textContent = node.label;
  row.appendChild(label);
  row.addEventListener("click", () => {
    toggle(key);
  });
  wrap.appendChild(row);

  if (isOpen) {
    const children = el("div", "children");
    for (const entry of node.here) {
      children.appendChild(renderProjectEntry(entry, entry.group.name));
    }
    for (const child of node.children) {
      children.appendChild(renderProjectNode(child));
    }
    wrap.appendChild(children);
  }
  return wrap;
}

function renderProjectEntry(entry: ProjectEntry, label: string): HTMLElement {
  const group = entry.group;
  const wrap = el("div");
  const key = `project:${group.path}`;
  const isOpen = expanded.has(key);

  const row = el("div", "row project" + (isOpen ? " expanded" : ""));
  row.title = group.description
    ? `${group.path}\n\n${group.description}`
    : group.path;
  row.appendChild(el("span", "twisty", ICONS.chevron));
  row.appendChild(el("span", "icon", ICONS.folder));
  const labelEl = el("span", "label");
  labelEl.textContent = label;
  row.appendChild(labelEl);
  for (const tag of group.tags ?? []) {
    const chip = el("span", "tagchip");
    chip.textContent = tag;
    row.appendChild(chip);
  }
  row.appendChild(el("span", "grow"));
  const badge = el("span", "badge meta hide-on-hover");
  badge.textContent = String(entry.sessions.length || group.session_count);
  row.appendChild(badge);

  const actions = el("span", "actions");
  actions.appendChild(
    iconBtn(ICONS.plus, "New session here", () =>
      send({ type: "newSession", dir: group.path }),
    ),
  );
  actions.appendChild(
    iconBtn(ICONS.chat, "Chat about this project", () =>
      send({ type: "chat", dir: group.path }),
    ),
  );
  actions.appendChild(
    iconBtn(ICONS.sparkle, "Generate project summary (LLM)", () =>
      send({ type: "summarize", path: group.path, label: group.name }),
    ),
  );
  actions.appendChild(
    iconBtn(ICONS.info, "Project details", () =>
      send({ type: "openProject", path: group.path }),
    ),
  );
  actions.appendChild(
    iconBtn(ICONS.tag, "Add tag to project", () =>
      send({ type: "addTag", kind: "project", id: group.path, label: group.name }),
    ),
  );
  if ((group.tags ?? []).length > 0) {
    actions.appendChild(
      iconBtn(ICONS.untag, "Remove tag from project", () =>
        send({
          type: "removeTag",
          kind: "project",
          id: group.path,
          label: group.name,
          tags: group.tags ?? [],
        }),
      ),
    );
  }
  row.appendChild(actions);

  row.addEventListener("click", () => toggle(key));
  wrap.appendChild(row);

  if (isOpen) {
    const children = el("div", "children");
    for (const session of entry.sessions) {
      children.appendChild(renderSessionRow(session, false));
    }
    if (entry.sessions.length === 0) {
      children.appendChild(el("div", "empty", "No sessions."));
    }
    wrap.appendChild(children);
  }
  return wrap;
}

function renderSessionRow(
  session: SessionSummary,
  showProject: boolean,
): HTMLElement {
  const row = el("div", "row session");
  row.title = `${session.title ?? session.session_id}\n${session.project}`;
  row.appendChild(el("span", `dot ${session.source}`));

  const label = el("span", "label grow");
  label.textContent =
    session.title ?? firstLine(session.snippet) ?? session.session_id;
  row.appendChild(label);

  for (const tag of session.tags.slice(0, 3)) {
    const chip = el("span", "tagchip");
    chip.textContent = tag;
    row.appendChild(chip);
  }

  const meta = el("span", "meta hide-on-hover");
  meta.textContent = showProject
    ? `${lastSegment(session.project)} · ${relTime(session.timestamp)}`
    : relTime(session.timestamp);
  row.appendChild(meta);

  const actions = el("span", "actions");
  actions.appendChild(
    iconBtn(ICONS.play, "Resume in terminal", () =>
      send({ type: "resume", sessionId: session.session_id }),
    ),
  );
  actions.appendChild(
    iconBtn(ICONS.plus, "New session in this project", () =>
      send({
        type: "newSession",
        dir: session.project,
        sessionId: session.session_id,
      }),
    ),
  );
  actions.appendChild(
    iconBtn(ICONS.edit, "Rename", () =>
      send({ type: "rename", sessionId: session.session_id }),
    ),
  );
  actions.appendChild(
    iconBtn(ICONS.tag, "Add tag", () =>
      send({
        type: "addTag",
        kind: "session",
        id: session.session_id,
        label: session.title ?? session.session_id,
      }),
    ),
  );
  row.appendChild(actions);

  row.addEventListener("click", () =>
    send({ type: "open", sessionId: session.session_id, title: session.title }),
  );
  return row;
}

function renderTagsSection(model: HubModel): HTMLElement {
  const section = el("div", "section");
  const { header, isOpen } = sectionHeader(
    "Tags",
    model.tags.length > 0 ? String(model.tags.length) : "",
    [],
  );
  section.appendChild(header);
  if (!isOpen()) {
    return section;
  }

  if (model.tags.length === 0) {
    section.appendChild(
      el("div", "empty", "No tags yet. Tag a session or a project."),
    );
    return section;
  }

  for (const entry of model.tags) {
    const wrap = el("div");
    const key = `tag:${entry.tag}`;
    const isTagOpen = expanded.has(key);
    const row = el("div", "row" + (isTagOpen ? " expanded" : ""));
    row.appendChild(el("span", "twisty", ICONS.chevron));
    row.appendChild(el("span", "icon", ICONS.tag));
    const label = el("span", "label grow");
    label.textContent = entry.tag;
    row.appendChild(label);
    const badge = el("span", "badge");
    badge.textContent = String(entry.count);
    row.appendChild(badge);
    row.addEventListener("click", () => toggle(key));
    wrap.appendChild(row);

    if (isTagOpen) {
      const children = el("div", "children");
      for (const project of entry.projects) {
        children.appendChild(renderProjectEntry(project, project.group.name));
      }
      for (const session of entry.sessions) {
        children.appendChild(renderSessionRow(session, true));
      }
      wrap.appendChild(children);
    }
    section.appendChild(wrap);
  }
  return section;
}

function toggle(key: string): void {
  if (expanded.has(key)) {
    expanded.delete(key);
  } else {
    expanded.add(key);
  }
  persist();
  render();
}

function firstLine(text: string): string {
  return text.split("\n")[0]?.trim() ?? "";
}

function lastSegment(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

// ── Wire up ──

window.addEventListener("message", (event: MessageEvent<ExtToWeb>) => {
  const msg = event.data;
  if (msg.type === "state") {
    state = msg.state;
    if (!state.methods.includes(method)) {
      method = state.methods[0] ?? "fts";
      persist();
    }
    render();
  } else if (msg.type === "focus") {
    searchInput?.focus();
  }
});

render();
send({ type: "ready" });
