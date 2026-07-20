// Message protocol between the extension host and the hub webview.
// Pure types — imported by both sides.

import type {
  ProjectGroup,
  SearchMethod,
  SessionSummary,
} from "../sessfind/types";

export type ViewMode = "list" | "tree";

/** One ranked engine match: the session and the best snippet found in it. */
export interface RankedMatch {
  session_id: string;
  snippet: string;
}

/** Serializable form of the active filter (Sets don't survive postMessage). */
export interface FilterPayload {
  query: string;
  engineIds: string[];
  engineOnly: boolean;
  /**
   * Ranked engine matches (best first) with their snippets, for the Results
   * section. Empty for substring-only filters that never hit the engine.
   */
  matches: RankedMatch[];
}

export interface HubState {
  sessions: SessionSummary[];
  projects: ProjectGroup[];
  methods: SearchMethod[];
  viewMode: ViewMode;
  filter: FilterPayload | null;
  busy: boolean;
  /** Non-null when the binary is unavailable/incompatible; UI shows it. */
  error: string | null;
}

export type WebToExt =
  | { type: "ready" }
  | { type: "query"; value: string; method: SearchMethod }
  | { type: "open"; sessionId: string; title: string | null }
  | { type: "openProject"; path: string }
  | { type: "resume"; sessionId: string }
  | { type: "newSession"; dir: string; sessionId?: string }
  | { type: "rename"; sessionId: string }
  | { type: "addTag"; kind: "session" | "project"; id: string; label: string }
  | {
      type: "removeTag";
      kind: "session" | "project";
      id: string;
      label: string;
      tags: string[];
    }
  | { type: "setViewMode"; mode: ViewMode }
  | { type: "summarize"; path: string; label: string }
  | { type: "chat"; dir: string }
  | { type: "stats" }
  | { type: "refresh" }
  | { type: "index" };

export type ExtToWeb = { type: "state"; state: HubState } | { type: "focus" };
