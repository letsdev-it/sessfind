import type {
  ProjectGroup,
  SessionSummary,
  StatsPayload,
} from "../sessfind/types";
export type { StatsPayload } from "../sessfind/types";

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
  const freshness = stats.sources ?? {};
  if (Object.keys(freshness).length > 0) {
    lines.push("## Source freshness", "");
    lines.push("| Source | Status | Last successful sync |");
    lines.push("| --- | --- | --- |");
    for (const source of ["claude", "opencode", "copilot", "cursor", "codex"] as const) {
      const state = freshness[source];
      if (state) {
        lines.push(
          `| ${source} | ${state.status} | ${state.last_success ?? "never"} |`,
        );
        if (state.error) {
          lines.push(`|  | error | ${state.error.replaceAll("|", "\\|")} |`);
        }
      }
    }
    lines.push("");
  }

  const perDay = new Map<string, number>();
  for (const s of sessions) {
    const day = s.timestamp.slice(0, 10);
    perDay.set(day, (perDay.get(day) ?? 0) + 1);
  }

  // Activity: last 14 days as bars.
  lines.push("## Activity (last 14 days)", "");
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

  // Contribution heatmap: last 12 weeks (GitHub-style, weeks as rows).
  lines.push(renderHeatmap(perDay, today), "");

  // Busiest hours of the day.
  lines.push(renderBusiestHours(sessions), "");

  // Work log: last 7 days, grouped by day then project.
  lines.push(renderWorkLog(sessions, today), "");

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
  lines.push(`- Watcher: ${stats.watcher?.state ?? "unknown"}`);
  if (stats.data_dir) {
    lines.push(`- Index location: \`${stats.data_dir}\``);
  }
  lines.push("");

  return lines.join("\n");
}

const HEAT = [" ", "░", "▒", "▓", "█"];

/** GitHub-style contribution heatmap for the last 12 weeks. */
function renderHeatmap(perDay: Map<string, number>, today: Date): string {
  const weeks = 12;
  // Start on the Monday 12 weeks back.
  const start = new Date(today);
  start.setUTCHours(0, 0, 0, 0);
  const dow = (start.getUTCDay() + 6) % 7; // Mon=0
  start.setUTCDate(start.getUTCDate() - dow - (weeks - 1) * 7);

  let peak = 0;
  for (const n of perDay.values()) {
    peak = Math.max(peak, n);
  }
  const level = (n: number): string => {
    if (n === 0) {
      return HEAT[0];
    }
    const idx = 1 + Math.min(3, Math.floor((n / Math.max(1, peak)) * 3.999));
    return HEAT[idx];
  };

  const labels = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
  const rows: string[] = [];
  for (let d = 0; d < 7; d++) {
    let line = `${labels[d]} `;
    for (let w = 0; w < weeks; w++) {
      const cell = new Date(start);
      cell.setUTCDate(start.getUTCDate() + w * 7 + d);
      const key = cell.toISOString().slice(0, 10);
      line += cell > today ? " " : level(perDay.get(key) ?? 0);
    }
    rows.push(line);
  }
  return [
    "## Contribution heatmap (12 weeks)",
    "",
    "```",
    ...rows,
    `    less ${HEAT.slice(1).join("")} more`,
    "```",
  ].join("\n");
}

/** Bar chart of sessions bucketed by hour of local day. */
function renderBusiestHours(sessions: SessionSummary[]): string {
  const buckets = new Array(24).fill(0);
  for (const s of sessions) {
    const d = new Date(s.timestamp);
    if (!Number.isNaN(d.getTime())) {
      buckets[d.getHours()] += 1;
    }
  }
  const max = Math.max(1, ...buckets);
  const rows: string[] = [];
  for (let h = 0; h < 24; h++) {
    const bar = "█".repeat(Math.round((buckets[h] / max) * 20));
    const hh = String(h).padStart(2, "0");
    rows.push(`${hh}:00  ${bar}${buckets[h] > 0 ? ` ${buckets[h]}` : ""}`);
  }
  return ["## Busiest hours", "", "```", ...rows, "```"].join("\n");
}

/** Per-day work log for the last 7 days: projects touched and session titles. */
function renderWorkLog(sessions: SessionSummary[], today: Date): string {
  const cutoff = new Date(today.getTime() - 7 * 86_400_000);
  const recent = sessions
    .filter((s) => new Date(s.timestamp) >= cutoff)
    .sort((a, b) => b.timestamp.localeCompare(a.timestamp));

  const lines: string[] = ["## Work log (last 7 days)", ""];
  if (recent.length === 0) {
    lines.push("_No sessions in the last 7 days._");
    return lines.join("\n");
  }

  const byDay = new Map<string, SessionSummary[]>();
  for (const s of recent) {
    const day = s.timestamp.slice(0, 10);
    (byDay.get(day) ?? byDay.set(day, []).get(day)!).push(s);
  }

  for (const [day, daySessions] of byDay) {
    lines.push(`### ${day} — ${daySessions.length} session${daySessions.length === 1 ? "" : "s"}`);
    lines.push("");
    const byProject = new Map<string, SessionSummary[]>();
    for (const s of daySessions) {
      const name = lastSegment(s.project);
      (byProject.get(name) ?? byProject.set(name, []).get(name)!).push(s);
    }
    for (const [project, projectSessions] of byProject) {
      lines.push(`- **${project}**`);
      for (const s of projectSessions) {
        const title = (s.title ?? firstLine(s.snippet)).replaceAll("|", "\\|");
        lines.push(`  - ${s.timestamp.slice(11, 16)} [${s.source}] ${title}`);
      }
    }
    lines.push("");
  }
  return lines.join("\n").trimEnd();
}

function lastSegment(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

function firstLine(text: string): string {
  return text.split("\n")[0]?.trim() ?? "";
}
