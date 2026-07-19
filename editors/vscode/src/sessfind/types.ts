// TypeScript mirrors of the JSON produced by the `sessfind` binary.
// Keep in sync with crates/sessfind-common/src/lib.rs. The binary advertises
// its JSON contract version via `capabilities.json_api_version`.

/** The JSON API version this extension is written against. */
export const SUPPORTED_JSON_API_VERSION = 1;

export type Source = "claude" | "opencode" | "copilot" | "cursor" | "codex";

export interface CommandSpec {
  args: string[];
  cwd: string | null;
}

export interface SessionSummary {
  session_id: string;
  source: Source;
  project: string;
  title: string | null;
  timestamp: string;
  snippet: string;
  tags: string[];
  resume: CommandSpec;
  new_session: CommandSpec;
}

export interface ProjectGroup {
  path: string;
  name: string;
  session_count: number;
  last_activity: string;
  sources: Source[];
}

export interface UserProject {
  name: string;
  root_dir: string;
  dirs: string[];
  pinned_sessions: string[];
  description: string | null;
  created_at: string;
}

export interface TagCount {
  tag: string;
  session_count: number;
}

export interface ToolInfo {
  name: string;
  new_session: CommandSpec;
}

export interface SearchMethods {
  fts: boolean;
  fuzzy: boolean;
  semantic: boolean;
  llm: string[];
}

export interface Capabilities {
  version: string;
  json_api_version: number;
  features: string[];
  search_methods: SearchMethods;
  data_dir: string;
}

export interface SearchResult {
  chunk_id: string;
  session_id: string;
  source: Source;
  project: string;
  timestamp: string;
  title: string | null;
  snippet: string;
  score: number;
}

export interface SessionDetail {
  session: SessionSummary;
  chunks: SearchResult[];
}

export type SearchMethod = "fts" | "fuzzy" | "semantic" | "llm";
