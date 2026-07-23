// Message protocol between the extension host and the hub webview.
// Pure types — imported by both sides.

import type {
  ProjectGroup,
  SearchMethod,
  SessionSummary,
  Source,
} from "../sessfind/types";

export type ViewMode = "list" | "tree";

/** One ranked engine match: the session and the best snippet found in it. */
export interface RankedMatch {
  session_key: string;
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
  defaultMethod: SearchMethod;
  features: string[];
  viewMode: ViewMode;
  filter: FilterPayload | null;
  busy: boolean;
  searchError: string | null;
  warnings: string[];
  /** Non-null when the binary is unavailable/incompatible; UI shows it. */
  error: string | null;
}

export type WebToExt =
  | { type: "ready" }
  | { type: "query"; value: string; method: SearchMethod }
  | { type: "open"; sessionId: string; source: Source; title: string | null }
  | { type: "openProject"; path: string }
  | { type: "resume"; sessionId: string; source: Source }
  | { type: "newSession"; dir: string; sessionId?: string; source?: Source }
  | { type: "rename"; sessionId: string; source: Source }
  | {
      type: "addTag";
      kind: "session" | "project";
      id: string;
      label: string;
      source?: Source;
    }
  | {
      type: "removeTag";
      kind: "session" | "project";
      id: string;
      label: string;
      tags: string[];
      source?: Source;
    }
  | { type: "setViewMode"; mode: ViewMode }
  | { type: "chat"; dir: string }
  | { type: "stats" }
  | { type: "refresh" }
  | { type: "index" };

export type ExtToWeb = { type: "state"; state: HubState } | { type: "focus" };
