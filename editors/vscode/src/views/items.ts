import * as vscode from "vscode";
import type { ProjectGroup, SessionSummary } from "../sessfind/types";
import { SessionDocumentProvider } from "../preview/sessionDocumentProvider";

/** Tree node for one indexed session. `contextValue` drives context menus. */
export class SessionItem extends vscode.TreeItem {
  constructor(readonly session: SessionSummary) {
    super(
      session.title ?? session.snippet ?? session.session_id,
      vscode.TreeItemCollapsibleState.None,
    );
    this.id = `session:${session.session_id}`;
    this.description = `${session.source} · ${formatDate(session.timestamp)}`;
    this.tooltip = buildTooltip(session);
    this.contextValue = "session";
    this.iconPath = new vscode.ThemeIcon(
      session.custom_name ? "comment-draft" : "comment-discussion",
    );
    this.command = {
      command: "sessfind.openSession",
      title: "Open Session",
      arguments: [session.session_id, session.title],
    };
    this.resourceUri = SessionDocumentProvider.uriFor(
      session.session_id,
      session.title,
    );
  }
}

export class ProjectGroupItem extends vscode.TreeItem {
  constructor(
    readonly group: ProjectGroup,
    labelOverride?: string,
  ) {
    super(labelOverride ?? group.name, vscode.TreeItemCollapsibleState.Collapsed);
    this.id = `project:${group.path}`;
    const tags = group.tags ?? [];
    const tagPart = tags.length > 0 ? ` · [${tags.join(", ")}]` : "";
    this.description = `${group.session_count} · ${group.sources.join(", ")}${tagPart}`;
    this.tooltip = buildProjectTooltip(group);
    this.contextValue = "autoProject";
    this.iconPath = new vscode.ThemeIcon("folder");
  }
}

/** Intermediate directory node in the "tree" display mode of Projects. */
export class DirectoryItem extends vscode.TreeItem {
  constructor(
    readonly path: string,
    label: string,
  ) {
    super(label, vscode.TreeItemCollapsibleState.Expanded);
    this.id = `dirnode:${path}`;
    this.contextValue = "directory";
    this.iconPath = vscode.ThemeIcon.Folder;
    this.tooltip = path;
  }
}

export class TagItem extends vscode.TreeItem {
  constructor(
    readonly tag: string,
    count: number,
  ) {
    super(tag, vscode.TreeItemCollapsibleState.Collapsed);
    this.id = `tag:${tag}`;
    this.description = String(count);
    this.contextValue = "tag";
    this.iconPath = new vscode.ThemeIcon("tag");
  }
}

/** Leaf shown when a collection is empty. */
export class MessageItem extends vscode.TreeItem {
  constructor(message: string) {
    super(message, vscode.TreeItemCollapsibleState.None);
    this.contextValue = "message";
  }
}

function buildTooltip(s: SessionSummary): vscode.MarkdownString {
  const md = new vscode.MarkdownString();
  md.appendMarkdown(`**${s.title ?? s.session_id}**\n\n`);
  if (s.custom_name) {
    md.appendMarkdown(`- Renamed (custom name)\n`);
  }
  md.appendMarkdown(`- Source: ${s.source}\n`);
  md.appendMarkdown(`- Project: \`${s.project}\`\n`);
  md.appendMarkdown(`- Date: ${formatDate(s.timestamp)}\n`);
  if (s.tags.length > 0) {
    md.appendMarkdown(`- Tags: ${s.tags.join(", ")}\n`);
  }
  md.appendMarkdown(`- ID: \`${s.session_id}\`\n`);
  return md;
}

function buildProjectTooltip(g: ProjectGroup): vscode.MarkdownString {
  const md = new vscode.MarkdownString();
  md.appendMarkdown(`**${g.name}**\n\n`);
  md.appendMarkdown(`- Path: \`${g.path}\`\n`);
  md.appendMarkdown(`- Sessions: ${g.session_count}\n`);
  if ((g.tags ?? []).length > 0) {
    md.appendMarkdown(`- Tags: ${(g.tags ?? []).join(", ")}\n`);
  }
  return md;
}

function formatDate(iso: string): string {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : d.toLocaleDateString();
}
