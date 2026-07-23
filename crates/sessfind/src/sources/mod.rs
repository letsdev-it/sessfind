pub mod claude_code;
pub mod codex;
pub mod copilot;
pub mod cursor;
pub mod opencode;

use crate::models::{Message, Session, Source};
use anyhow::Result;

pub trait SessionSource {
    fn name(&self) -> &'static str;
    fn list_sessions(&self) -> Result<Vec<Session>>;
    fn load_messages(&self, session: &Session) -> Result<Vec<Message>>;
}

pub fn source_for(source: Source) -> Box<dyn SessionSource> {
    match source {
        Source::ClaudeCode => Box::new(claude_code::ClaudeCodeSource::new()),
        Source::OpenCode => Box::new(opencode::OpenCodeSource::new()),
        Source::Copilot => Box::new(copilot::CopilotSource::new()),
        Source::Cursor => Box::new(cursor::CursorSource::new()),
        Source::Codex => Box::new(codex::CodexSource::new()),
    }
}

pub fn all_sources() -> Vec<Box<dyn SessionSource>> {
    [
        Source::ClaudeCode,
        Source::OpenCode,
        Source::Copilot,
        Source::Cursor,
        Source::Codex,
    ]
    .into_iter()
    .map(source_for)
    .collect()
}
