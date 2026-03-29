use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Session {
    pub source: Source,
    pub session_id: String,
    pub project: String,
    pub directory: String,
    pub title: Option<String>,
    pub started_at: DateTime<Utc>,
    pub model: Option<String>,
    pub file_path: String,
    pub file_mtime: i64,
    pub file_size: u64,
}

#[derive(Debug, Clone)]
pub struct Message {
    pub role: Role,
    pub text: String,
    pub timestamp: Option<DateTime<Utc>>,
    pub tool_names: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Chunk {
    pub chunk_id: String,
    pub session_id: String,
    pub source: Source,
    pub text: String,
    pub project: String,
    pub timestamp: DateTime<Utc>,
    pub title: Option<String>,
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Source {
    ClaudeCode,
    OpenCode,
    Copilot,
}

impl Source {
    pub fn as_str(&self) -> &'static str {
        match self {
            Source::ClaudeCode => "claude",
            Source::OpenCode => "opencode",
            Source::Copilot => "copilot",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "claude" => Some(Source::ClaudeCode),
            "opencode" => Some(Source::OpenCode),
            "copilot" => Some(Source::Copilot),
            _ => None,
        }
    }
}

impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
}
