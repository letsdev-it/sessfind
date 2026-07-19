import type { SessionSummary, Source } from "../sessfind/types";

export interface ProjectDetailsInput {
  title: string;
  kind: "auto" | "user";
  /** Root (user project) or the grouped directory (auto project). */
  rootDir: string;
  /** Extra directories (user projects only). */
  dirs: string[];
  pinnedSessions: string[];
  description: string | null;
  /** Tags attached to the project directory itself. */
  tags?: string[];
  /** Sessions belonging to this project. */
  sessions: SessionSummary[];
}

/**
 * Render a project overview as Markdown: metadata plus data-derived metrics
 * (counts per source, activity span, top tags, recent sessions). Pure — no
 * `vscode` import — so it is unit-testable.
 */
export function renderProject(input: ProjectDetailsInput): string {
  const lines: string[] = [];
  lines.push(`# ${input.title}`, "");
  if (input.description) {
    lines.push(input.description, "");
  }

  lines.push("## Overview", "");
  lines.push(
    `- **Kind:** ${input.kind === "user" ? "user project" : "auto project (grouped by directory)"}`,
  );
  lines.push(`- **Root:** \`${input.rootDir}\``);
  for (const dir of input.dirs) {
    lines.push(`- **Extra dir:** \`${dir}\``);
  }
  if (input.pinnedSessions.length > 0) {
    lines.push(`- **Pinned sessions:** ${input.pinnedSessions.length}`);
  }
  if ((input.tags ?? []).length > 0) {
    lines.push(
      `- **Project tags:** ${(input.tags ?? []).map((t) => `\`${t}\``).join(", ")}`,
    );
  }
  lines.push("");

  lines.push("## Metrics", "");
  const sessions = [...input.sessions].sort((a, b) =>
    b.timestamp.localeCompare(a.timestamp),
  );
  lines.push(`- **Sessions:** ${sessions.length}`);
  if (sessions.length > 0) {
    const bySource = countBySource(sessions);
    const parts = [...bySource.entries()].map(([s, n]) => `${s}: ${n}`);
    lines.push(`- **By source:** ${parts.join(", ")}`);
    const newest = sessions[0].timestamp;
    const oldest = sessions[sessions.length - 1].timestamp;
    lines.push(`- **First activity:** ${formatDate(oldest)}`);
    lines.push(`- **Last activity:** ${formatDate(newest)}`);
    const days = activeDays(sessions);
    lines.push(`- **Active days:** ${days}`);
    const tags = topTags(sessions, 8);
    if (tags.length > 0) {
      lines.push(
        `- **Top tags:** ${tags.map(([t, n]) => `\`${t}\` (${n})`).join(", ")}`,
      );
    }
  }
  lines.push("");

  if (sessions.length > 0) {
    lines.push("## Recent sessions", "");
    lines.push("| Date | Source | Title | Tags |");
    lines.push("| --- | --- | --- | --- |");
    for (const s of sessions.slice(0, 15)) {
      const title = (s.title ?? firstLine(s.snippet)).replaceAll("|", "\\|");
      const tags = s.tags.join(", ");
      lines.push(
        `| ${formatDate(s.timestamp)} | ${s.source} | ${title} | ${tags} |`,
      );
    }
    lines.push("");
  }

  return lines.join("\n");
}

function countBySource(sessions: SessionSummary[]): Map<Source, number> {
  const map = new Map<Source, number>();
  for (const s of sessions) {
    map.set(s.source, (map.get(s.source) ?? 0) + 1);
  }
  return map;
}

function activeDays(sessions: SessionSummary[]): number {
  const days = new Set(sessions.map((s) => s.timestamp.slice(0, 10)));
  return days.size;
}

function topTags(sessions: SessionSummary[], limit: number): [string, number][] {
  const counts = new Map<string, number>();
  for (const s of sessions) {
    for (const t of s.tags) {
      counts.set(t, (counts.get(t) ?? 0) + 1);
    }
  }
  return [...counts.entries()]
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .slice(0, limit);
}

function firstLine(text: string): string {
  return text.split("\n")[0]?.trim() ?? "";
}

function formatDate(iso: string): string {
  return iso.slice(0, 10);
}
