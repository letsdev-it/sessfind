pub mod claude_code;
pub mod codex;
pub mod copilot;
pub mod cursor;
pub mod opencode;

use std::path::PathBuf;

use crate::models::{Message, Session};
use anyhow::Result;

pub trait SessionSource {
    fn name(&self) -> &'static str;
    fn list_sessions(&self) -> Result<Vec<Session>>;
    fn load_messages(&self, session: &Session) -> Result<Vec<Message>>;

    /// Directories the watcher should monitor for this source.
    ///
    /// Returns `(path, recursive)` pairs. The watcher only registers paths
    /// that exist on disk.
    fn watch_dirs(&self) -> Vec<(PathBuf, bool)>;
}
