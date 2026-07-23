import { describe, expect, it } from "vitest";
import type { SessionDetail } from "../sessfind/types";
import { renderSession } from "./renderSession";

const detail: SessionDetail = {
  session: {
    session_id: "abc",
    source: "claude",
    project: "/home/me/proj",
    title: "Fix the build",
    timestamp: "2026-01-15T10:30:00Z",
    snippet: "",
    tags: ["work", "rust"],
    resume: { args: ["claude", "--resume", "abc"], cwd: "/home/me/proj" },
    new_session: { args: ["claude"], cwd: "/home/me/proj" },
  },
  chunks: [
    {
      chunk_id: "claude:abc:0",
      session_id: "abc",
      source: "claude",
      project: "/home/me/proj",
      timestamp: "2026-01-15T10:30:00Z",
      title: "Fix the build",
      snippet: "USER: why is CI red?\nASSISTANT: the lockfile is stale\n[tools: Read, Bash]",
      score: 1,
    },
  ],
};

describe("renderSession", () => {
  it("renders a title heading and metadata", () => {
    const md = renderSession(detail);
    expect(md).toContain("# Fix the build");
    expect(md).toContain("**Source:** claude");
    expect(md).toContain("`/home/me/proj`");
    expect(md).toContain("**Tags:** `work`, `rust`");
  });

  it("splits USER/ASSISTANT into sections and quotes tool lines", () => {
    const md = renderSession(detail);
    expect(md).toContain("### User");
    expect(md).toContain("why is CI red?");
    expect(md).toContain("### Assistant");
    expect(md).toContain("the lockfile is stale");
    expect(md).toContain("> [tools: Read, Bash]");
  });

  it("falls back to session id when title is null", () => {
    const noTitle: SessionDetail = {
      ...detail,
      session: { ...detail.session, title: null },
    };
    expect(renderSession(noTitle)).toContain("# abc");
  });
});
