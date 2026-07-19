import type { ProjectGroup, SessionSummary } from "../sessfind/types";

/** Shape of `sessfind stats --json` (loosely typed; fields are additive). */
export interface StatsPayload {
  sessions?: Record<string, number>;
  semantic?: { available?: boolean; model?: string; indexed_chunks?: number };
  llm_backends?: { name: string; model: string | null }[];
  data_dir?: string;
}

/**
 * Render a data-based statistics page as Markdown: totals per source,
 * a 14-day activity sparkline, most active projects and tags. Pure —
 * unit-testable, no vscode import.
 */
export function renderStats(
  stats: StatsPayload,
  sessions: SessionSummary[],
  projects: ProjectGroup[],
  today: Date = new Date(),
): string {
  const lines: string[] = [];
  lines.push("# Sessfind statistics", "");

  // Sessions per source.
  const counts = stats.sessions ?? {};
  lines.push("## Sessions by source", "");
  lines.push("| Source | Sessions |");
  lines.push("| --- | ---: |");
  for (const source of ["claude", "opencode", "copilot", "cursor", "codex"]) {
    if (counts[source] !== undefined) {
      lines.push(`| ${source} | ${counts[source]} |`);
    }
  }
  lines.push(`| **total** | **${counts.total ?? sessions.length}** |`, "");

  // Activity: last 14 days.
  lines.push("## Activity (last 14 days)", "");
  const perDay = new Map<string, number>();
  for (const s of sessions) {
    perDay.set(s.timestamp.slice(0, 10), (perDay.get(s.timestamp.slice(0, 10)) ?? 0) + 1);
  }
  const days: { day: string; count: number }[] = [];
  for (let i = 13; i >= 0; i--) {
    const d = new Date(today.getTime() - i * 86_400_000);
    const day = d.toISOString().slice(0, 10);
    days.push({ day, count: perDay.get(day) ?? 0 });
  }
  const max = Math.max(1, ...days.map((d) => d.count));
  lines.push("```");
  for (const { day, count } of days) {
    const bar = "█".repeat(Math.round((count / max) * 24));
    lines.push(`${day}  ${bar}${count > 0 ? ` ${count}` : ""}`);
  }
  lines.push("```", "");

  // Most active projects.
  const topProjects = [...projects]
    .sort((a, b) => b.session_count - a.session_count)
    .slice(0, 10);
  if (topProjects.length > 0) {
    lines.push("## Most active projects", "");
    lines.push("| Project | Sessions | Last activity |");
    lines.push("| --- | ---: | --- |");
    for (const p of topProjects) {
      lines.push(
        `| ${p.name} | ${p.session_count} | ${p.last_activity.slice(0, 10)} |`,
      );
    }
    lines.push("");
  }

  // Tags.
  const tagCounts = new Map<string, number>();
  for (const s of sessions) {
    for (const t of s.tags) {
      tagCounts.set(t, (tagCounts.get(t) ?? 0) + 1);
    }
  }
  if (tagCounts.size > 0) {
    lines.push("## Tags", "");
    const sorted = [...tagCounts.entries()].sort((a, b) => b[1] - a[1]);
    lines.push(
      sorted.map(([tag, n]) => `\`${tag}\` (${n})`).join(", "),
      "",
    );
  }

  // Engines.
  lines.push("## Search engines", "");
  const semantic = stats.semantic;
  lines.push(
    `- Semantic: ${
      semantic?.available
        ? `available (${semantic.model ?? "?"}, ${semantic.indexed_chunks ?? "?"} chunks)`
        : "not installed"
    }`,
  );
  const backends = stats.llm_backends ?? [];
  lines.push(
    `- LLM backends: ${
      backends.length > 0
        ? backends.map((b) => b.name + (b.model ? `:${b.model}` : "")).join(", ")
        : "none detected"
    }`,
  );
  if (stats.data_dir) {
    lines.push(`- Index location: \`${stats.data_dir}\``);
  }
  lines.push("");

  return lines.join("\n");
}
