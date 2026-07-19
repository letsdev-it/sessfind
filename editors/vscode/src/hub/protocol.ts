// Message protocol between the extension host and the hub webview.
// Pure types — imported by both sides.

import type {
  ProjectGroup,
  SearchMethod,
  SessionSummary,
} from "../sessfind/types";

export type ViewMode = "list" | "tree";

/** Serializable form of the active filter (Sets don't survive postMessage). */
export interface FilterPayload {
  query: string;
  engineIds: string[];
  engineOnly: boolean;
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
  | { type: "refresh" }
  | { type: "index" };

export type ExtToWeb = { type: "state"; state: HubState } | { type: "focus" };
