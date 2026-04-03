use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::models::{Message, Role, Session, Source};
use crate::sources::SessionSource;

pub struct CursorSource {
    projects_dir: PathBuf,
}

fn projects_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cursor")
        .join("projects")
}

impl CursorSource {
    pub fn new() -> Self {
        Self {
            projects_dir: projects_dir(),
        }
    }
}

#[derive(Deserialize)]
struct CursorEntry {
    role: Option<String>,
    message: Option<CursorMessage>,
}

#[derive(Deserialize)]
struct CursorMessage {
    content: Option<serde_json::Value>,
}

/// Decode a Cursor project directory name back to a filesystem path.
///
/// Cursor encodes path separators as `-` (no leading dash):
/// - Unix:    `Users-m-repos-foo-bar` → `/Users/m/repos/foo-bar`
/// - Windows: `C-Users-m-repos-foo`  → `C:\Users\m\repos\foo`
fn decode_project_path(encoded: &str) -> String {
    let (remaining, root) = crate::platform::paths::decode_path_root(encoded, false);
    let segments: Vec<&str> = remaining.split('-').collect();
    crate::platform::paths::reconstruct_path(&segments, &root)
}

fn extract_text_from_content(content: &serde_json::Value) -> (String, Vec<String>) {
    let mut texts = Vec::new();
    let mut tool_names = Vec::new();

    match content {
        serde_json::Value::String(s) => {
            texts.push(s.clone());
        }
        serde_json::Value::Array(blocks) => {
            for block in blocks {
                if let Some(obj) = block.as_object() {
                    match obj.get("type").and_then(|t| t.as_str()) {
                        Some("text") => {
                            if let Some(text) = obj.get("text").and_then(|t| t.as_str()) {
                                texts.push(text.to_string());
                            }
                        }
                        Some("tool_use") => {
                            if let Some(name) = obj.get("name").and_then(|n| n.as_str()) {
                                tool_names.push(name.to_string());
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        _ => {}
    }

    (texts.join("\n"), tool_names)
}

/// Find Cursor agent transcript JSONL files.
///
/// Layout: `<projects_dir>/<project>/agent-transcripts/<uuid>/<uuid>.jsonl`
/// Skips `subagents/` subdirectories.
fn find_session_files(projects_dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in WalkDir::new(projects_dir)
        .min_depth(4)
        .max_depth(4)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "jsonl")
            && !path.to_str().is_some_and(|s| s.contains("subagents"))
        {
            files.push(path.to_path_buf());
        }
    }
    files
}

impl SessionSource for CursorSource {
    fn name(&self) -> &'static str {
        "cursor"
    }

    fn list_sessions(&self) -> Result<Vec<Session>> {
        if !self.projects_dir.exists() {
            return Ok(vec![]);
        }

        let mut sessions = Vec::new();

        for file_path in find_session_files(&self.projects_dir) {
            // Project name is the top-level directory under projects/
            // Layout: projects/<project>/agent-transcripts/<uuid>/<uuid>.jsonl
            let project_dir = file_path
                .parent() // <uuid> dir
                .and_then(|p| p.parent()) // agent-transcripts dir
                .and_then(|p| p.parent()) // <project> dir
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");
            let project = decode_project_path(project_dir);

            // Session ID from the UUID directory name
            let session_id = file_path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            let metadata = fs::metadata(&file_path)?;
            let file_mtime = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            let file_size = metadata.len();

            // Use file mtime as started_at (Cursor JSONL has no timestamps)
            let started_at = metadata
                .modified()
                .ok()
                .map(DateTime::<Utc>::from)
                .unwrap_or_else(Utc::now);

            // Read first user message as title
            let title = read_first_user_message(&file_path);

            sessions.push(Session {
                source: Source::Cursor,
                session_id,
                project: project.clone(),
                directory: project,
                title,
                started_at,
                model: None,
                file_path: file_path.to_string_lossy().to_string(),
                file_mtime,
                file_size,
            });
        }

        Ok(sessions)
    }

    fn load_messages(&self, session: &Session) -> Result<Vec<Message>> {
        let file = fs::File::open(&session.file_path)
            .with_context(|| format!("Failed to open {}", session.file_path))?;
        let reader = BufReader::new(file);
        let mut messages = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.is_empty() {
                continue;
            }

            let entry: CursorEntry = match serde_json::from_str(&line) {
                Ok(e) => e,
                Err(_) => continue,
            };

            let role = match entry.role.as_deref() {
                Some("user") => Role::User,
                Some("assistant") => Role::Assistant,
                _ => continue,
            };

            let content = match entry.message.and_then(|m| m.content) {
                Some(c) => c,
                None => continue,
            };

            let (text, tool_names) = extract_text_from_content(&content);

            if text.trim().is_empty() {
                continue;
            }

            messages.push(Message {
                role,
                text,
                timestamp: None,
                tool_names,
            });
        }

        Ok(messages)
    }

    fn watch_dirs(&self) -> Vec<(PathBuf, bool)> {
        vec![(self.projects_dir.clone(), true)]
    }
}

/// Read the first user message from a Cursor transcript as a session title.
fn read_first_user_message(path: &Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    let reader = BufReader::new(file);

    for line in reader.lines().take(20) {
        let line = line.ok()?;
        if line.is_empty() {
            continue;
        }
        let entry: CursorEntry = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue,
        };
        if entry.role.as_deref() != Some("user") {
            continue;
        }
        let content = entry.message?.content?;
        let (text, _) = extract_text_from_content(&content);
        let text = text.trim().to_string();
        if text.is_empty() {
            continue;
        }
        // Truncate to reasonable title length
        let title: String = text.chars().take(120).collect();
        return Some(title);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cursor_entry_user() {
        let json =
            r#"{"role":"user","message":{"content":[{"type":"text","text":"Hello, world!"}]}}"#;
        let entry: CursorEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.role.as_deref(), Some("user"));
        let content = entry.message.unwrap().content.unwrap();
        let (text, tools) = extract_text_from_content(&content);
        assert_eq!(text, "Hello, world!");
        assert!(tools.is_empty());
    }

    #[test]
    fn parse_cursor_entry_assistant() {
        let json = r#"{"role":"assistant","message":{"content":[{"type":"text","text":"I can help with that."}]}}"#;
        let entry: CursorEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.role.as_deref(), Some("assistant"));
        let content = entry.message.unwrap().content.unwrap();
        let (text, _) = extract_text_from_content(&content);
        assert_eq!(text, "I can help with that.");
    }

    #[test]
    fn parse_cursor_entry_multiple_blocks() {
        let json = r#"{"role":"assistant","message":{"content":[{"type":"text","text":"First"},{"type":"text","text":"Second"}]}}"#;
        let entry: CursorEntry = serde_json::from_str(json).unwrap();
        let content = entry.message.unwrap().content.unwrap();
        let (text, _) = extract_text_from_content(&content);
        assert_eq!(text, "First\nSecond");
    }

    #[test]
    fn parse_cursor_entry_with_tool_use() {
        let json = r#"{"role":"assistant","message":{"content":[{"type":"text","text":"Let me check."},{"type":"tool_use","name":"read_file"}]}}"#;
        let entry: CursorEntry = serde_json::from_str(json).unwrap();
        let content = entry.message.unwrap().content.unwrap();
        let (text, tools) = extract_text_from_content(&content);
        assert_eq!(text, "Let me check.");
        assert_eq!(tools, vec!["read_file"]);
    }

    #[test]
    fn skip_empty_messages() {
        let json = r#"{"role":"user","message":{"content":[{"type":"text","text":"   "}]}}"#;
        let entry: CursorEntry = serde_json::from_str(json).unwrap();
        let content = entry.message.unwrap().content.unwrap();
        let (text, _) = extract_text_from_content(&content);
        assert!(text.trim().is_empty());
    }

    #[test]
    fn unknown_role_skipped() {
        let json =
            r#"{"role":"system","message":{"content":[{"type":"text","text":"System msg"}]}}"#;
        let entry: CursorEntry = serde_json::from_str(json).unwrap();
        assert_ne!(entry.role.as_deref(), Some("user"));
        assert_ne!(entry.role.as_deref(), Some("assistant"));
    }

    #[test]
    fn content_as_string() {
        let content = serde_json::json!("plain text content");
        let (text, tools) = extract_text_from_content(&content);
        assert_eq!(text, "plain text content");
        assert!(tools.is_empty());
    }
}
