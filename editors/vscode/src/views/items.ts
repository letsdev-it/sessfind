import * as vscode from "vscode";
import type { ProjectGroup, SessionSummary, UserProject } from "../sessfind/types";
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
    this.iconPath = new vscode.ThemeIcon("comment-discussion");
    this.command = {
      command: "sessfind.openSession",
      title: "Open Session",
      arguments: [session.session_id],
    };
    this.resourceUri = SessionDocumentProvider.uriFor(session.session_id);
  }
}

export class ProjectGroupItem extends vscode.TreeItem {
  constructor(readonly group: ProjectGroup) {
    super(group.name, vscode.TreeItemCollapsibleState.Collapsed);
    this.id = `project:${group.path}`;
    this.description = `${group.session_count} · ${group.sources.join(", ")}`;
    this.tooltip = group.path;
    this.contextValue = "autoProject";
    this.iconPath = new vscode.ThemeIcon("folder");
  }
}

export class UserProjectItem extends vscode.TreeItem {
  constructor(readonly project: UserProject) {
    super(project.name, vscode.TreeItemCollapsibleState.Collapsed);
    this.id = `userProject:${project.name}`;
    this.description = project.description ?? project.root_dir;
    this.tooltip = buildUserProjectTooltip(project);
    this.contextValue = "userProject";
    this.iconPath = new vscode.ThemeIcon("project");
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

/** A directory belonging to a user project (root or extra). */
export class ProjectDirItem extends vscode.TreeItem {
  constructor(
    readonly projectName: string,
    readonly dir: string,
    readonly isRoot: boolean,
  ) {
    super(dir, vscode.TreeItemCollapsibleState.None);
    this.id = `dir:${projectName}:${dir}`;
    this.description = isRoot ? "root" : undefined;
    this.contextValue = isRoot ? "projectDirRoot" : "projectDirExtra";
    this.iconPath = new vscode.ThemeIcon(isRoot ? "root-folder" : "folder");
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
  md.appendMarkdown(`- Source: ${s.source}\n`);
  md.appendMarkdown(`- Project: \`${s.project}\`\n`);
  md.appendMarkdown(`- Date: ${formatDate(s.timestamp)}\n`);
  if (s.tags.length > 0) {
    md.appendMarkdown(`- Tags: ${s.tags.join(", ")}\n`);
  }
  return md;
}

function buildUserProjectTooltip(p: UserProject): vscode.MarkdownString {
  const md = new vscode.MarkdownString();
  md.appendMarkdown(`**${p.name}**\n\n`);
  md.appendMarkdown(`- Root: \`${p.root_dir}\`\n`);
  if (p.dirs.length > 0) {
    md.appendMarkdown(`- Dirs: ${p.dirs.length}\n`);
  }
  if (p.pinned_sessions.length > 0) {
    md.appendMarkdown(`- Pinned: ${p.pinned_sessions.length}\n`);
  }
  return md;
}

function formatDate(iso: string): string {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : d.toLocaleDateString();
}
