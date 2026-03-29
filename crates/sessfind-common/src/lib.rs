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
}

impl Source {
    pub fn as_str(&self) -> &'static str {
        match self {
            Source::ClaudeCode => "claude",
            Source::OpenCode => "opencode",
            Source::Copilot => "copilot",
            Source::Cursor => "cursor",
        }
    }

    pub fn parse_source(s: &str) -> Option<Self> {
        match s {
            "claude" => Some(Source::ClaudeCode),
            "opencode" => Some(Source::OpenCode),
            "copilot" => Some(Source::Copilot),
            "cursor" => Some(Source::Cursor),
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
    }

    #[test]
    fn source_from_str() {
        assert_eq!(Source::parse_source("claude"), Some(Source::ClaudeCode));
        assert_eq!(Source::parse_source("opencode"), Some(Source::OpenCode));
        assert_eq!(Source::parse_source("copilot"), Some(Source::Copilot));
        assert_eq!(Source::parse_source("cursor"), Some(Source::Cursor));
        assert_eq!(Source::parse_source("unknown"), None);
    }

    #[test]
    fn source_display() {
        assert_eq!(format!("{}", Source::ClaudeCode), "claude");
        assert_eq!(format!("{}", Source::Copilot), "copilot");
        assert_eq!(format!("{}", Source::Cursor), "cursor");
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
    fn data_dir_ends_with_sessfind() {
        let dir = data_dir();
        assert!(dir.ends_with("sessfind"));
    }
}
