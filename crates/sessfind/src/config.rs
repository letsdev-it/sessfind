use std::path::PathBuf;

pub use sessfind_common::CHUNK_MAX_CHARS;
pub use sessfind_common::CHUNK_MIN_CHARS;
pub use sessfind_common::data_dir;

pub fn claude_projects_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".claude")
        .join("projects")
}

pub fn opencode_db_path() -> PathBuf {
    // OpenCode stores its DB in XDG_DATA_HOME (~/.local/share on Linux/macOS)
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".local")
        .join("share")
        .join("opencode")
        .join("opencode.db")
}

pub fn copilot_session_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".copilot")
        .join("session-state")
}
