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
  /** Stable source-qualified identity; absent only with older compatible binaries. */
  session_key?: string;
  session_id: string;
  source: Source;
  project: string;
  /** Display title: the user's custom name when set, else the tool's title. */
  title: string | null;
  /** User-assigned name override, when one exists. */
  custom_name?: string | null;
  timestamp: string;
  snippet: string;
  /** Tags attached directly to this session, excluding inherited project tags. */
  direct_tags?: string[];
  /** Effective tags: direct plus inherited from the project directory. */
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
  /** Tags attached to the whole directory. */
  tags?: string[];
  /** LLM-generated project summary, when one has been produced. */
  description?: string | null;
}

export interface TagCount {
  tag: string;
  session_count: number;
}

export interface ToolInfo {
  name: string;
  new_session: CommandSpec;
  /** Can open an interactive session with an initial prompt (project chat). */
  chat_capable?: boolean;
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

export interface SourceFreshness {
  status: "absent" | "available" | "stale" | "failed";
  sessions: number;
  last_success?: string | null;
  last_attempt?: string | null;
  error?: string | null;
}

/** Additive shape returned by `sessfind stats --json`. */
export interface StatsPayload {
  sessions?: Record<string, number>;
  sources?: Partial<Record<Source, SourceFreshness>>;
  semantic?: { available?: boolean; model?: string; indexed_chunks?: number };
  llm_backends?: { name: string; model: string | null }[];
  watcher?: { installed?: boolean; state?: string; error?: string };
  data_dir?: string;
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

export function sessionKey(
  session: Pick<SessionSummary, "session_key" | "source" | "session_id">,
): string {
  return session.session_key || `${session.source}:${session.session_id}`;
}
