use chrono::{DateTime, Utc};

// Re-export shared types from common crate
pub use sessfind_common::SearchParams;
pub use sessfind_common::SearchResult;
pub use sessfind_common::Source;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
}
