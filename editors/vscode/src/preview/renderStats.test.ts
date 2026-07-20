import { describe, expect, it } from "vitest";
import type { ProjectGroup, SessionSummary } from "../sessfind/types";
import { renderStats } from "./renderStats";

function session(id: string, day: string, tags: string[] = []): SessionSummary {
  return {
    session_id: id,
    source: "claude",
    project: "/p",
    title: null,
    timestamp: `${day}T10:00:00Z`,
    snippet: "",
    tags,
    resume: { args: ["claude"], cwd: null },
    new_session: { args: ["claude"], cwd: null },
  };
}

describe("renderStats", () => {
  const today = new Date("2026-07-19T12:00:00Z");
  const sessions = [
    session("a", "2026-07-19", ["work"]),
    session("b", "2026-07-19"),
    session("c", "2026-07-10", ["work", "rust"]),
  ];
  const projects: ProjectGroup[] = [
    {
      path: "/p",
      name: "p",
      session_count: 3,
      last_activity: "2026-07-19T10:00:00Z",
      sources: ["claude"],
      tags: [],
    },
  ];

  it("renders source counts and totals", () => {
    const md = renderStats(
      { sessions: { claude: 3, codex: 0, total: 3 } },
      sessions,
      projects,
      today,
    );
    expect(md).toContain("| claude | 3 |");
    expect(md).toContain("| **total** | **3** |");
  });

  it("renders a 14-day activity chart with counts", () => {
    const md = renderStats({}, sessions, projects, today);
    expect(md).toContain("2026-07-19");
    expect(md).toMatch(/2026-07-19 {2}█+ 2/);
    expect(md).toMatch(/2026-07-10 {2}█+ 1/);
    // A day with no sessions renders an empty bar.
    expect(md).toMatch(/2026-07-18 {2}\n/);
  });

  it("renders top projects and tags", () => {
    const md = renderStats({}, sessions, projects, today);
    expect(md).toContain("| p | 3 | 2026-07-19 |");
    expect(md).toContain("`work` (2)");
    expect(md).toContain("`rust` (1)");
  });

  it("renders a 12-week heatmap with a legend", () => {
    const md = renderStats({}, sessions, projects, today);
    expect(md).toContain("Contribution heatmap (12 weeks)");
    expect(md).toContain("Mon ");
    expect(md).toContain("Sun ");
    expect(md).toMatch(/less .*more/);
  });

  it("renders busiest-hours buckets", () => {
    const md = renderStats({}, sessions, projects, today);
    expect(md).toContain("## Busiest hours");
    // Sessions at 10:00Z — hour depends on TZ, but a labelled 24h grid exists.
    expect(md).toContain("00:00");
    expect(md).toContain("23:00");
  });

  it("renders a 7-day work log grouped by day and project", () => {
    const near = new Date("2026-07-18T09:30:00Z");
    const recent = [
      {
        ...sessions[0],
        session_id: "r",
        title: "Recent work",
        timestamp: near.toISOString(),
      },
    ];
    const md = renderStats({}, recent, projects, today);
    expect(md).toContain("## Work log (last 7 days)");
    expect(md).toContain("### 2026-07-18");
    expect(md).toContain("**p**");
    expect(md).toContain("Recent work");
  });

  it("work log notes when nothing is recent", () => {
    const old = [session("z", "2020-01-01")];
    const md = renderStats({}, old, projects, today);
    expect(md).toContain("_No sessions in the last 7 days._");
  });

  it("reports engines", () => {
    const md = renderStats(
      {
        semantic: { available: true, model: "e5", indexed_chunks: 42 },
        llm_backends: [{ name: "claude", model: "sonnet" }],
        data_dir: "/data",
      },
      [],
      [],
      today,
    );
    expect(md).toContain("Semantic: available (e5, 42 chunks)");
    expect(md).toContain("LLM backends: claude:sonnet");
    expect(md).toContain("`/data`");
  });
});
