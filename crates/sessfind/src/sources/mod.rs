pub mod claude_code;
pub mod copilot;
pub mod cursor;
pub mod opencode;

use crate::models::{Message, Session};
use anyhow::Result;

pub trait SessionSource {
    fn name(&self) -> &'static str;
    fn list_sessions(&self) -> Result<Vec<Session>>;
    fn load_messages(&self, session: &Session) -> Result<Vec<Message>>;
}
