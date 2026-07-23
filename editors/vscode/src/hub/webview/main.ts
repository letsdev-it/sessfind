// Webview script for the sessfind hub. Renders the full sidebar (search,
// results, recent, projects, tags) from state pushed by the extension, and
// sends user intents back. Rendering is plain DOM — moderate row counts.

import "./hub.css";
import {
  buildModel,
  type HubModel,
  type ProjectEntry,
  type ProjectNode,
  type ResultEntry,
} from "../model";
import { condenseSnippet, highlightSegments } from "../highlight";
import type { ExtToWeb, HubState, WebToExt } from "../protocol";
import type { SearchMethod, SessionSummary } from "../../sessfind/types";
import { closeMenu, openMenu, type MenuAction } from "./contextMenu";

declare function acquireVsCodeApi(): {
  postMessage(msg: WebToExt): void;
  getState(): { method?: SearchMethod; expanded?: string[] } | undefined;
  setState(state: { method?: SearchMethod; expanded?: string[] }): void;
};

const vscode = acquireVsCodeApi();
const persistedState = vscode.getState();

const INSTANT: SearchMethod[] = ["fts", "fuzzy"];
const LABELS: Record<SearchMethod, string> = {
  fts: "FTS",
  fuzzy: "Fuzzy",
  semantic: "Semantic",
  llm: "LLM",
};

// ── Local UI state ──

let state: HubState | null = null;
let method: SearchMethod = persistedState?.method ?? "fts";
let methodSelected = persistedState?.method !== undefined;
const expanded = new Set<string>(persistedState?.expanded ?? []);
let debounce: ReturnType<typeof setTimeout> | undefined;

// Keyboard navigation: navigable rows collected each render, plus active index.
let navRows: HTMLElement[] = [];
let activeIndex = -1;

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
  open: svg('<path d="M9 2.5h4.5V7"/><line x1="13.5" y1="2.5" x2="7.5" y2="8.5"/><path d="M11 9v3.5H3.5V5H7"/>'),
  chevron: svg('<path d="M6 3.5 10.5 8 6 12.5"/>'),
  folder: svg('<path d="M1.8 4.2c0-.6.4-1 1-1h3.4l1.4 1.6h5.6c.6 0 1 .4 1 1v6c0 .6-.4 1-1 1H2.8c-.6 0-1-.4-1-1z"/>'),
  listTree: svg('<line x1="3" y1="4" x2="7" y2="4"/><line x1="6" y1="8" x2="10" y2="8"/><line x1="9" y1="12" x2="13" y2="12"/>'),
  listFlat: svg('<line x1="3" y1="4" x2="13" y2="4"/><line x1="3" y1="8" x2="13" y2="8"/><line x1="3" y1="12" x2="13" y2="12"/>'),
  refresh: svg('<path d="M13 8a5 5 0 1 1-1.5-3.5"/><path d="M13 2.8v2.7h-2.7"/>'),
  db: svg('<ellipse cx="8" cy="4" rx="5" ry="2"/><path d="M3 4v8c0 1.1 2.2 2 5 2s5-.9 5-2V4"/><path d="M3 8c0 1.1 2.2 2 5 2s5-.9 5-2"/>'),
  chat: svg('<path d="M2.5 3.5h11v7h-6l-3 2.5v-2.5h-2z"/>'),
  chart: svg('<line x1="3" y1="13" x2="13" y2="13"/><line x1="4.5" y1="13" x2="4.5" y2="8"/><line x1="8" y1="13" x2="8" y2="4"/><line x1="11.5" y1="13" x2="11.5" y2="10"/>'),
  clock: svg('<circle cx="8" cy="8" r="5.7"/><path d="M8 5v3.2l2 1.3"/>'),
};

// ── DOM helpers ──

function el(tag: string, className?: string, html?: string): HTMLElement {
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

/** Append highlighted text (query terms wrapped in <mark>) safely. */
function appendHighlighted(
  parent: HTMLElement,
  text: string,
  query: string,
): void {
  for (const seg of highlightSegments(text, query)) {
    if (seg.mark) {
      const mark = document.createElement("mark");
      mark.textContent = seg.text;
      parent.appendChild(mark);
    } else {
      parent.appendChild(document.createTextNode(seg.text));
    }
  }
}

// ── Shared action definitions (hover buttons + context menu) ──

function sessionActions(session: SessionSummary): MenuAction[] {
  const actions: MenuAction[] = [
    {
      label: "Resume in terminal",
      icon: ICONS.play,
      run: () =>
        send({
          type: "resume",
          sessionId: session.session_id,
          source: session.source,
        }),
    },
    {
      label: "Open conversation",
      icon: ICONS.open,
      run: () =>
        send({
          type: "open",
          sessionId: session.session_id,
          source: session.source,
          title: session.title,
        }),
    },
    {
      label: "New session in project",
      icon: ICONS.plus,
      run: () =>
        send({
          type: "newSession",
          dir: session.project,
          sessionId: session.session_id,
          source: session.source,
        }),
    },
  ];
  if (state?.features.includes("session-rename")) {
    actions.push({
      label: "Rename…",
      icon: ICONS.edit,
      run: () =>
        send({
          type: "rename",
          sessionId: session.session_id,
          source: session.source,
        }),
    });
  }
  if (state?.features.includes("tags")) {
    actions.push({
      label: "Add tag…",
      icon: ICONS.tag,
      run: () =>
        send({
          type: "addTag",
          kind: "session",
          id: session.session_id,
          label: session.title ?? session.session_id,
          source: session.source,
        }),
    });
  }
  const removableTags = session.direct_tags ?? session.tags;
  if (
    state?.features.includes("tags") &&
    state.features.includes("direct-session-tags") &&
    removableTags.length > 0
  ) {
    actions.push({
      label: "Remove tag…",
      icon: ICONS.untag,
      run: () =>
        send({
          type: "removeTag",
          kind: "session",
          id: session.session_id,
          label: session.title ?? session.session_id,
          tags: removableTags,
          source: session.source,
        }),
    });
  }
  return actions;
}

function projectActions(group: ProjectEntry["group"]): MenuAction[] {
  const actions: MenuAction[] = [
    {
      label: "New session here",
      icon: ICONS.plus,
      run: () => send({ type: "newSession", dir: group.path }),
    },
  ];
  if (state?.features.includes("project-chat")) {
    actions.push({
      label: "Chat about this project",
      icon: ICONS.chat,
      run: () => send({ type: "chat", dir: group.path }),
    });
  }
  actions.push({
      label: "Project details",
      icon: ICONS.info,
      run: () => send({ type: "openProject", path: group.path }),
    });
  if (state?.features.includes("project-tags")) {
    actions.push({
      label: "Add tag…",
      icon: ICONS.tag,
      run: () =>
        send({
          type: "addTag",
          kind: "project",
          id: group.path,
          label: group.name,
        }),
    });
  }
  if (
    state?.features.includes("project-tags") &&
    (group.tags ?? []).length > 0
  ) {
    actions.push({
      label: "Remove tag…",
      icon: ICONS.untag,
      run: () =>
        send({
          type: "removeTag",
          kind: "project",
          id: group.path,
          label: group.name,
          tags: group.tags ?? [],
        }),
    });
  }
  return actions;
}

/** Render the first N actions as inline hover buttons. */
function hoverButtons(actions: MenuAction[], limit: number): HTMLElement {
  const span = el("span", "actions");
  for (const a of actions.slice(0, limit)) {
    span.appendChild(iconBtn(a.icon ?? "", a.label, () => a.run()));
  }
  return span;
}

function attachContextMenu(row: HTMLElement, actions: MenuAction[]): void {
  row.addEventListener("contextmenu", (e) => {
    e.preventDefault();
    e.stopPropagation();
    openMenu(e.clientX, e.clientY, actions);
  });
}

/** Register a row for keyboard navigation and set its primary activation. */
function makeNavigable(row: HTMLElement, activate: () => void): void {
  row.dataset.nav = "1";
  row.tabIndex = -1;
  (row as HTMLElement & { _activate?: () => void })._activate = activate;
  row.addEventListener("mouseenter", () => setActive(navRows.indexOf(row)));
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
  closeMenu();
  root.textContent = "";
  navRows = [];
  activeIndex = -1;

  root.appendChild(renderSearch(inputValue));

  if (!state) {
    root.appendChild(renderSkeleton());
    return;
  }
  if (state.error) {
    root.appendChild(el("div", "error")).textContent = state.error;
    return;
  }
  for (const warning of state.warnings) {
    root.appendChild(el("div", "warning")).textContent = warning;
  }

  const model = buildModel(state);
  if (model.query.length > 0) {
    root.appendChild(renderResultsSection(model));
  } else if (model.recent.length > 0) {
    root.appendChild(renderRecentSection(model));
  }
  root.appendChild(renderProjectsSection(model));
  root.appendChild(renderTagsSection(model));

  // Re-collect navigable rows in document order.
  navRows = [...root.querySelectorAll<HTMLElement>('[data-nav="1"]')];

  if (focusHadSearch) {
    searchInput?.focus();
  }
}

function renderSkeleton(): HTMLElement {
  const wrap = el("div", "skeleton");
  for (let i = 0; i < 6; i++) {
    const line = el("div", "skel-row");
    line.style.width = `${55 + ((i * 13) % 40)}%`;
    wrap.appendChild(line);
  }
  return wrap;
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
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      focusList(0);
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
      methodSelected = true;
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
    node.classList.remove("error");
  } else if (state?.searchError) {
    node.textContent = `Search failed: ${firstLine(state.searchError)}`;
    node.classList.add("error");
  } else if (!isInstant() && q.length > 0) {
    node.textContent = "Enter ↵ to search";
    node.classList.remove("error");
  } else if (state?.filter && state.filter.query.length > 0) {
    const model = state ? buildModel(state) : null;
    node.textContent = model
      ? `${model.results.length} result${model.results.length === 1 ? "" : "s"}`
      : "";
    node.classList.remove("error");
  } else {
    node.textContent = "";
    node.classList.remove("error");
  }
}

interface SectionOpts {
  defaultOpen?: boolean;
  tools?: HTMLElement[];
}

function sectionHeader(
  title: string,
  count: string,
  opts: SectionOpts = {},
): { header: HTMLElement; isOpen: () => boolean } {
  const key = `section:${title}`;
  const defaultOpen = opts.defaultOpen ?? true;
  const open = () =>
    defaultOpen ? !expanded.has(`${key}:closed`) : expanded.has(`${key}:open`);
  const header = el("div", "section-header");
  const twisty = el("span", "twisty", ICONS.chevron);
  twisty.style.transform = open() ? "rotate(90deg)" : "";
  header.appendChild(twisty);
  header.appendChild(el("span", "section-title", title));
  if (count) {
    header.appendChild(el("span", "count", count));
  }
  header.appendChild(el("span", "spacer"));
  const toolbox = el("span", "tools");
  for (const t of opts.tools ?? []) {
    toolbox.appendChild(t);
  }
  header.appendChild(toolbox);
  header.addEventListener("click", () => {
    const openKey = defaultOpen ? `${key}:closed` : `${key}:open`;
    if (expanded.has(openKey)) {
      expanded.delete(openKey);
    } else {
      expanded.add(openKey);
    }
    persist();
    render();
  });
  return { header, isOpen: open };
}

function renderResultsSection(model: HubModel): HTMLElement {
  const section = el("div", "section");
  const { header, isOpen } = sectionHeader(
    "Results",
    String(model.results.length),
  );
  section.appendChild(header);
  if (!isOpen()) {
    return section;
  }
  if (model.results.length === 0) {
    section.appendChild(
      el("div", "empty", state?.busy ? "Searching…" : "No matches."),
    );
    return section;
  }
  for (const r of model.results) {
    section.appendChild(renderResultRow(r, model.query));
  }
  return section;
}

function renderResultRow(result: ResultEntry, query: string): HTMLElement {
  const session = result.session;
  const row = el("div", "row result");
  row.title = `${session.title ?? session.session_id}\n${session.project}`;

  const head = el("div", "result-head");
  head.appendChild(el("span", `dot ${session.source}`));
  const title = el("span", "label grow");
  appendHighlighted(
    title,
    session.title ?? firstLine(session.snippet) ?? session.session_id,
    query,
  );
  head.appendChild(title);
  head.appendChild(hoverButtons(sessionActions(session), 3));
  row.appendChild(head);

  const snippet = el("div", "result-snippet");
  appendHighlighted(snippet, condenseSnippet(result.snippet, query), query);
  row.appendChild(snippet);

  const meta = el("div", "result-meta");
  meta.textContent = `${lastSegment(session.project)} · ${relTime(session.timestamp)}`;
  row.appendChild(meta);

  attachContextMenu(row, sessionActions(session));
  makeNavigable(row, () =>
    send({
      type: "open",
      sessionId: session.session_id,
      source: session.source,
      title: session.title,
    }),
  );
  row.addEventListener("click", () =>
    send({
      type: "open",
      sessionId: session.session_id,
      source: session.source,
      title: session.title,
    }),
  );
  return row;
}

function renderRecentSection(model: HubModel): HTMLElement {
  const section = el("div", "section");
  const { header, isOpen } = sectionHeader("Recent", "", {
    tools: [
      iconBtn(ICONS.clock, "Statistics", () => send({ type: "stats" })),
    ],
  });
  section.appendChild(header);
  if (!isOpen()) {
    return section;
  }
  for (const session of model.recent) {
    section.appendChild(renderSessionRow(session, true, ""));
  }
  return section;
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
    model.projects.length === 0 ? "" : String(countProjects(model.projects)),
    { tools },
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
  makeNavigable(row, () => toggle(key));
  row.addEventListener("click", () => toggle(key));
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
  const actions = projectActions(group);

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
    row.appendChild(tagChip(tag));
  }
  row.appendChild(el("span", "grow"));
  const badge = el("span", "badge meta hide-on-hover");
  badge.textContent = String(entry.sessions.length || group.session_count);
  row.appendChild(badge);
  row.appendChild(hoverButtons(actions, 4));

  attachContextMenu(row, actions);
  makeNavigable(row, () => toggle(key));
  row.addEventListener("click", () => toggle(key));
  wrap.appendChild(row);

  if (isOpen) {
    const children = el("div", "children");
    for (const session of entry.sessions) {
      children.appendChild(renderSessionRow(session, false, ""));
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
  query: string,
): HTMLElement {
  const row = el("div", "row session");
  row.title = `${session.title ?? session.session_id}\n${session.project}`;
  row.appendChild(el("span", `dot ${session.source}`));

  const label = el("span", "label grow");
  appendHighlighted(
    label,
    session.title ?? firstLine(session.snippet) ?? session.session_id,
    query,
  );
  row.appendChild(label);

  for (const tag of session.tags.slice(0, 3)) {
    row.appendChild(tagChip(tag));
  }

  const meta = el("span", "meta hide-on-hover");
  meta.textContent = showProject
    ? `${lastSegment(session.project)} · ${relTime(session.timestamp)}`
    : relTime(session.timestamp);
  row.appendChild(meta);

  const actions = sessionActions(session);
  row.appendChild(hoverButtons(actions, 3));

  attachContextMenu(row, actions);
  makeNavigable(row, () =>
    send({
      type: "open",
      sessionId: session.session_id,
      source: session.source,
      title: session.title,
    }),
  );
  row.addEventListener("click", () =>
    send({
      type: "open",
      sessionId: session.session_id,
      source: session.source,
      title: session.title,
    }),
  );
  return row;
}

function renderTagsSection(model: HubModel): HTMLElement {
  const section = el("div", "section");
  const { header, isOpen } = sectionHeader(
    "Tags",
    model.tags.length > 0 ? String(model.tags.length) : "",
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
    row.appendChild(el("span", "icon tag-icon", ICONS.tag));
    const label = el("span", "label grow");
    label.textContent = entry.tag;
    row.appendChild(label);
    const badge = el("span", "badge");
    badge.textContent = String(entry.count);
    row.appendChild(badge);
    makeNavigable(row, () => toggle(key));
    row.addEventListener("click", () => toggle(key));
    wrap.appendChild(row);

    if (isTagOpen) {
      const children = el("div", "children");
      for (const project of entry.projects) {
        children.appendChild(renderProjectEntry(project, project.group.name));
      }
      for (const session of entry.sessions) {
        children.appendChild(renderSessionRow(session, true, ""));
      }
      wrap.appendChild(children);
    }
    section.appendChild(wrap);
  }
  return section;
}

// ── Tag chips (stable hue per tag) ──

function tagChip(tag: string): HTMLElement {
  const chip = el("span", "tagchip");
  chip.textContent = tag;
  const hue = hashHue(tag);
  chip.style.setProperty("--tag-hue", String(hue));
  return chip;
}

function hashHue(s: string): number {
  let h = 0;
  for (let i = 0; i < s.length; i++) {
    h = (h * 31 + s.charCodeAt(i)) % 360;
  }
  return h;
}

// ── Keyboard navigation ──

function setActive(index: number): void {
  if (activeIndex >= 0 && navRows[activeIndex]) {
    navRows[activeIndex].classList.remove("active");
  }
  activeIndex = index;
  if (activeIndex >= 0 && navRows[activeIndex]) {
    navRows[activeIndex].classList.add("active");
  }
}

function focusList(index: number): void {
  if (navRows.length === 0) {
    return;
  }
  setActive(Math.max(0, Math.min(index, navRows.length - 1)));
  navRows[activeIndex].scrollIntoView({ block: "nearest" });
  navRows[activeIndex].focus?.();
}

document.addEventListener("keydown", (e) => {
  if (e.key === "ArrowDown" && document.activeElement === searchInput) {
    return; // handled by the input's own listener
  }
  if (navRows.length === 0) {
    return;
  }
  if (e.key === "ArrowDown") {
    e.preventDefault();
    focusList(activeIndex + 1);
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    if (activeIndex <= 0) {
      searchInput?.focus();
      setActive(-1);
    } else {
      focusList(activeIndex - 1);
    }
  } else if (e.key === "Enter" && activeIndex >= 0) {
    e.preventDefault();
    const row = navRows[activeIndex] as HTMLElement & {
      _activate?: () => void;
    };
    row._activate?.();
  } else if (
    e.key === "ContextMenu" ||
    (e.shiftKey && e.key === "F10")
  ) {
    if (activeIndex >= 0) {
      e.preventDefault();
      const rect = navRows[activeIndex].getBoundingClientRect();
      navRows[activeIndex].dispatchEvent(
        new MouseEvent("contextmenu", {
          bubbles: true,
          clientX: rect.left + 20,
          clientY: rect.bottom,
        }),
      );
    }
  }
});

function firstLine(text: string): string {
  return text.split("\n")[0]?.trim() ?? "";
}

function lastSegment(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
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

// ── Wire up ──

window.addEventListener("message", (event: MessageEvent<ExtToWeb>) => {
  const msg = event.data;
  if (msg.type === "state") {
    state = msg.state;
    if (!methodSelected) {
      method = state.defaultMethod;
    }
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
