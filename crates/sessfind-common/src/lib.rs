use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ── Chunk size constants ──

pub const CHUNK_MAX_CHARS: usize = 6000;
pub const CHUNK_MIN_CHARS: usize = 50;

// ── Data directory ──

pub fn data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("sessfind")
}

// ── Source enum ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    #[serde(rename = "claude")]
    ClaudeCode,
    OpenCode,
    Copilot,
    Cursor,
    Codex,
}

impl Source {
    pub fn as_str(&self) -> &'static str {
        match self {
            Source::ClaudeCode => "claude",
            Source::OpenCode => "opencode",
            Source::Copilot => "copilot",
            Source::Cursor => "cursor",
            Source::Codex => "codex",
        }
    }

    pub fn parse_source(s: &str) -> Option<Self> {
        match s {
            "claude" => Some(Source::ClaudeCode),
            "opencode" => Some(Source::OpenCode),
            "copilot" => Some(Source::Copilot),
            "cursor" => Some(Source::Cursor),
            "codex" => Some(Source::Codex),
            _ => None,
        }
    }
}

impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ── SearchResult ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub chunk_id: String,
    pub session_id: String,
    pub source: Source,
    pub project: String,
    pub timestamp: DateTime<Utc>,
    pub title: Option<String>,
    pub snippet: String,
    pub score: f32,
}

// ── SearchParams ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchParams {
    pub query: String,
    pub limit: usize,
    pub source: Option<String>,
    pub project: Option<String>,
    pub after: Option<DateTime<Utc>>,
    pub before: Option<DateTime<Utc>>,
}

// ── CommandSpec (resume / new-session commands) ──

/// A command to launch in a terminal: `args[0]` is the binary, the rest are
/// its arguments. `cwd` is the directory the command must run in (Claude Code
/// requires being in the project directory to find the session).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandSpec {
    pub args: Vec<String>,
    pub cwd: Option<String>,
}

/// Build the command that resumes an existing session in the given directory.
pub fn resume_command(source: Source, session_id: &str, dir: &str) -> CommandSpec {
    let args = match source {
        Source::ClaudeCode => vec!["claude".into(), "--resume".into(), session_id.into()],
        Source::Copilot => vec!["copilot".into(), format!("--resume={session_id}")],
        Source::OpenCode => vec!["opencode".into(), "--session".into(), session_id.into()],
        Source::Cursor => vec!["cursor".into(), dir.into()],
        Source::Codex => vec!["codex".into(), "resume".into(), session_id.into()],
    };
    CommandSpec {
        args,
        cwd: Some(dir.into()),
    }
}

/// Build the command that starts a fresh session of the given tool in a directory.
pub fn new_session_command(source: Source, dir: &str) -> CommandSpec {
    let args = match source {
        Source::ClaudeCode => vec!["claude".into()],
        Source::Copilot => vec!["copilot".into()],
        Source::OpenCode => vec!["opencode".into()],
        Source::Cursor => vec!["cursor".into(), dir.into()],
        Source::Codex => vec!["codex".into()],
    };
    CommandSpec {
        args,
        cwd: Some(dir.into()),
    }
}

// ── JSON API types (consumed by the VS Code extension and future frontends) ──
//
// Evolution contract: changes to these types must be additive-only. Breaking
// changes require bumping `Capabilities::json_api_version`.

/// One indexed session, as listed by `sessfind sessions list --json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub source: Source,
    /// Absolute directory path the session ran in (drives grouping and resume cwd).
    pub project: String,
    /// Display title: the user's custom name when set, else the tool's title.
    pub title: Option<String>,
    /// User-assigned name override, when one exists (also reflected in `title`).
    #[serde(default)]
    pub custom_name: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub snippet: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub resume: CommandSpec,
    pub new_session: CommandSpec,
}

/// A project derived automatically by grouping sessions on their directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectGroup {
    pub path: String,
    /// Last path component, for display.
    pub name: String,
    pub session_count: usize,
    pub last_activity: DateTime<Utc>,
    pub sources: Vec<Source>,
    /// Tags attached to the whole directory (inherited by its sessions).
    #[serde(default)]
    pub tags: Vec<String>,
}

/// An installed AI CLI tool, with a ready-to-run new-session command for a
/// given directory. Produced by `sessfind tools list --dir <dir> --json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfo {
    pub name: String,
    pub new_session: CommandSpec,
}

/// A tag with the number of sessions carrying it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagCount {
    pub tag: String,
    pub session_count: usize,
}

/// Which search methods this binary can serve right now.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchMethods {
    pub fts: bool,
    pub fuzzy: bool,
    pub semantic: bool,
    /// Names of detected LLM backends (empty = LLM search unavailable).
    pub llm: Vec<String>,
}

/// Output of `sessfind capabilities` — lets clients gate features on what the
/// installed binary supports.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capabilities {
    pub version: String,
    pub json_api_version: u32,
    pub features: Vec<String>,
    pub search_methods: SearchMethods,
    pub data_dir: String,
}

// ── DumpChunk (for dump-chunks JSONL exchange) ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DumpChunk {
    pub chunk_id: String,
    pub session_id: String,
    pub source: Source,
    pub project: String,
    pub timestamp: DateTime<Utc>,
    pub title: Option<String>,
    pub text: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_as_str() {
        assert_eq!(Source::ClaudeCode.as_str(), "claude");
        assert_eq!(Source::OpenCode.as_str(), "opencode");
        assert_eq!(Source::Copilot.as_str(), "copilot");
        assert_eq!(Source::Cursor.as_str(), "cursor");
        assert_eq!(Source::Codex.as_str(), "codex");
    }

    #[test]
    fn source_from_str() {
        assert_eq!(Source::parse_source("claude"), Some(Source::ClaudeCode));
        assert_eq!(Source::parse_source("opencode"), Some(Source::OpenCode));
        assert_eq!(Source::parse_source("copilot"), Some(Source::Copilot));
        assert_eq!(Source::parse_source("cursor"), Some(Source::Cursor));
        assert_eq!(Source::parse_source("codex"), Some(Source::Codex));
        assert_eq!(Source::parse_source("unknown"), None);
    }

    #[test]
    fn source_display() {
        assert_eq!(format!("{}", Source::ClaudeCode), "claude");
        assert_eq!(format!("{}", Source::Copilot), "copilot");
        assert_eq!(format!("{}", Source::Cursor), "cursor");
        assert_eq!(format!("{}", Source::Codex), "codex");
    }

    #[test]
    fn source_serde_roundtrip() {
        let src = Source::ClaudeCode;
        let json = serde_json::to_string(&src).unwrap();
        assert_eq!(json, "\"claude\"");
        let back: Source = serde_json::from_str(&json).unwrap();
        assert_eq!(back, src);
    }

    #[test]
    fn source_serde_all_variants() {
        for (variant, expected) in [
            (Source::ClaudeCode, "\"claude\""),
            (Source::OpenCode, "\"opencode\""),
            (Source::Copilot, "\"copilot\""),
            (Source::Cursor, "\"cursor\""),
            (Source::Codex, "\"codex\""),
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            assert_eq!(json, expected);
            let back: Source = serde_json::from_str(&json).unwrap();
            assert_eq!(back, variant);
        }
    }

    #[test]
    fn search_result_serde_roundtrip() {
        let result = SearchResult {
            chunk_id: "claude:abc:0".into(),
            session_id: "abc".into(),
            source: Source::ClaudeCode,
            project: "/home/user/project".into(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2025-01-15T10:30:00Z")
                .unwrap()
                .with_timezone(&Utc),
            title: Some("Test session".into()),
            snippet: "USER: hello".into(),
            score: 0.95,
        };
        let json = serde_json::to_string(&result).unwrap();
        let back: SearchResult = serde_json::from_str(&json).unwrap();
        assert_eq!(back.chunk_id, "claude:abc:0");
        assert_eq!(back.source, Source::ClaudeCode);
        assert_eq!(back.title.as_deref(), Some("Test session"));
        assert!((back.score - 0.95).abs() < f32::EPSILON);
    }

    #[test]
    fn dump_chunk_serde_roundtrip() {
        let chunk = DumpChunk {
            chunk_id: "opencode:xyz:0".into(),
            session_id: "xyz".into(),
            source: Source::OpenCode,
            project: "/tmp/proj".into(),
            timestamp: Utc::now(),
            title: None,
            text: "USER: how do I parse JSON?\nASSISTANT: Use serde_json.".into(),
        };
        let json = serde_json::to_string(&chunk).unwrap();
        let back: DumpChunk = serde_json::from_str(&json).unwrap();
        assert_eq!(back.chunk_id, "opencode:xyz:0");
        assert_eq!(back.source, Source::OpenCode);
        assert!(back.title.is_none());
        assert!(back.text.contains("serde_json"));
    }

    #[test]
    fn search_params_serde() {
        let params = SearchParams {
            query: "rust async".into(),
            limit: 10,
            source: Some("claude".into()),
            project: None,
            after: None,
            before: None,
        };
        let json = serde_json::to_string(&params).unwrap();
        let back: SearchParams = serde_json::from_str(&json).unwrap();
        assert_eq!(back.query, "rust async");
        assert_eq!(back.limit, 10);
        assert_eq!(back.source.as_deref(), Some("claude"));
    }

    #[test]
    fn resume_command_per_source() {
        let cmd = resume_command(Source::ClaudeCode, "abc", "/proj");
        assert_eq!(cmd.args, vec!["claude", "--resume", "abc"]);
        assert_eq!(cmd.cwd.as_deref(), Some("/proj"));

        let cmd = resume_command(Source::Copilot, "abc", "/proj");
        assert_eq!(cmd.args, vec!["copilot", "--resume=abc"]);

        let cmd = resume_command(Source::OpenCode, "abc", "/proj");
        assert_eq!(cmd.args, vec!["opencode", "--session", "abc"]);

        let cmd = resume_command(Source::Cursor, "abc", "/proj");
        assert_eq!(cmd.args, vec!["cursor", "/proj"]);

        let cmd = resume_command(Source::Codex, "abc", "/proj");
        assert_eq!(cmd.args, vec!["codex", "resume", "abc"]);
    }

    #[test]
    fn new_session_command_per_source() {
        let cmd = new_session_command(Source::ClaudeCode, "/proj");
        assert_eq!(cmd.args, vec!["claude"]);
        assert_eq!(cmd.cwd.as_deref(), Some("/proj"));

        let cmd = new_session_command(Source::Cursor, "/proj");
        assert_eq!(cmd.args, vec!["cursor", "/proj"]);

        let cmd = new_session_command(Source::Codex, "/proj");
        assert_eq!(cmd.args, vec!["codex"]);
    }

    #[test]
    fn session_summary_serde_roundtrip() {
        let summary = SessionSummary {
            session_id: "abc".into(),
            source: Source::ClaudeCode,
            project: "/home/user/project".into(),
            title: Some("Test".into()),
            custom_name: None,
            timestamp: Utc::now(),
            snippet: "USER: hello".into(),
            tags: vec!["work".into()],
            resume: resume_command(Source::ClaudeCode, "abc", "/home/user/project"),
            new_session: new_session_command(Source::ClaudeCode, "/home/user/project"),
        };
        let json = serde_json::to_string(&summary).unwrap();
        let back: SessionSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(back.session_id, "abc");
        assert_eq!(back.tags, vec!["work"]);
        assert_eq!(back.resume.args, vec!["claude", "--resume", "abc"]);
    }

    #[test]
    fn session_summary_tags_default_to_empty() {
        // Older producers may omit `tags` — clients must still parse.
        let json = r#"{
            "session_id": "abc", "source": "claude", "project": "/p",
            "title": null, "timestamp": "2025-01-15T10:30:00Z", "snippet": "s",
            "resume": {"args": ["claude"], "cwd": "/p"},
            "new_session": {"args": ["claude"], "cwd": "/p"}
        }"#;
        let back: SessionSummary = serde_json::from_str(json).unwrap();
        assert!(back.tags.is_empty());
        assert!(back.custom_name.is_none());
    }

    #[test]
    fn tool_info_serde_roundtrip() {
        let tool = ToolInfo {
            name: "claude".into(),
            new_session: new_session_command(Source::ClaudeCode, "/proj"),
        };
        let json = serde_json::to_string(&tool).unwrap();
        let back: ToolInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "claude");
        assert_eq!(back.new_session.args, vec!["claude"]);
        assert_eq!(back.new_session.cwd.as_deref(), Some("/proj"));
    }

    #[test]
    fn capabilities_serde_roundtrip() {
        let caps = Capabilities {
            version: "0.9.0".into(),
            json_api_version: 1,
            features: vec!["search-json".into()],
            search_methods: SearchMethods {
                fts: true,
                fuzzy: true,
                semantic: false,
                llm: vec!["claude".into()],
            },
            data_dir: "/data".into(),
        };
        let json = serde_json::to_string(&caps).unwrap();
        let back: Capabilities = serde_json::from_str(&json).unwrap();
        assert_eq!(back.json_api_version, 1);
        assert_eq!(back.search_methods.llm, vec!["claude"]);
    }

    #[test]
    fn command_spec_serde_roundtrip() {
        let cmd = resume_command(Source::ClaudeCode, "abc", "/proj");
        let json = serde_json::to_string(&cmd).unwrap();
        let back: CommandSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(back.args, cmd.args);
        assert_eq!(back.cwd, cmd.cwd);
    }

    #[test]
    fn data_dir_ends_with_sessfind() {
        let dir = data_dir();
        assert!(dir.ends_with("sessfind"));
    }
}
