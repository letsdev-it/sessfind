use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::path::PathBuf;
use walkdir::WalkDir;

use crate::models::{Message, Role, Session, Source};
use crate::sources::SessionSource;

pub struct CodexSource {
    sessions_dir: PathBuf,
}

fn sessions_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".codex")
        .join("sessions")
}

impl CodexSource {
    pub fn new() -> Self {
        Self {
            sessions_dir: sessions_dir(),
        }
    }
}

// Top-level JSONL event
#[derive(Deserialize)]
struct CodexEvent {
    timestamp: Option<String>,
    #[serde(rename = "type")]
    event_type: String,
    payload: serde_json::Value,
}

// session_meta payload
#[derive(Deserialize)]
struct SessionMeta {
    id: Option<String>,
    cwd: Option<String>,
}

// response_item payload
#[derive(Deserialize)]
struct ResponseItem {
    role: Option<String>,
    content: Option<Vec<ContentBlock>>,
}

// turn_context payload (for model info)
#[derive(Deserialize)]
struct TurnContext {
    model: Option<String>,
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    block_type: Option<String>,
    text: Option<String>,
    name: Option<String>,
}

fn find_session_files(sessions_dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in WalkDir::new(sessions_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "jsonl") && path.is_file() {
            files.push(path.to_path_buf());
        }
    }
    files
}

/// Extract text from content blocks, returning (text, tool_names).
fn extract_content(blocks: &[ContentBlock]) -> (String, Vec<String>) {
    let mut texts = Vec::new();
    let mut tool_names = Vec::new();

    for block in blocks {
        match block.block_type.as_deref() {
            Some("input_text") | Some("output_text") => {
                if let Some(text) = &block.text {
                    // Skip system/environment context injected by Codex
                    if !text.starts_with("<environment_context>")
                        && !text.starts_with("<plugins_instructions>")
                    {
                        texts.push(text.as_str());
                    }
                }
            }
            Some("function_call") => {
                if let Some(name) = &block.name {
                    tool_names.push(name.clone());
                }
            }
            _ => {}
        }
    }

    (texts.join("\n"), tool_names)
}

impl SessionSource for CodexSource {
    fn name(&self) -> &'static str {
        "codex"
    }

    fn list_sessions(&self) -> Result<Vec<Session>> {
        if !self.sessions_dir.exists() {
            return Ok(vec![]);
        }

        let mut sessions = Vec::new();

        for file_path in find_session_files(&self.sessions_dir) {
            let file = fs::File::open(&file_path)?;
            let reader = BufReader::new(file);

            let mut session_id = None;
            let mut cwd = None;
            let mut started_at = None;
            let mut model = None;
            let mut title = None;

            for line in reader.lines().take(30) {
                let line = line?;
                if line.is_empty() {
                    continue;
                }

                let event: CodexEvent = match serde_json::from_str(&line) {
                    Ok(e) => e,
                    Err(_) => continue,
                };

                match event.event_type.as_str() {
                    "session_meta" => {
                        if let Ok(meta) =
                            serde_json::from_value::<SessionMeta>(event.payload.clone())
                        {
                            session_id = meta.id;
                            cwd = meta.cwd;
                        }
                        if started_at.is_none() {
                            started_at = event
                                .timestamp
                                .as_deref()
                                .and_then(|ts| ts.parse::<DateTime<Utc>>().ok());
                        }
                    }
                    "turn_context" => {
                        if model.is_none()
                            && let Ok(ctx) =
                                serde_json::from_value::<TurnContext>(event.payload.clone())
                        {
                            model = ctx.model;
                        }
                    }
                    "response_item" => {
                        if title.is_none()
                            && let Ok(item) =
                                serde_json::from_value::<ResponseItem>(event.payload.clone())
                            && item.role.as_deref() == Some("user")
                            && let Some(blocks) = &item.content
                        {
                            let (text, _) = extract_content(blocks);
                            let text = text.trim().to_string();
                            if !text.is_empty() {
                                title = Some(text.chars().take(120).collect::<String>());
                            }
                        }
                    }
                    _ => {}
                }
            }

            // Fall back to extracting session ID from filename
            let session_id = session_id.unwrap_or_else(|| {
                file_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string()
            });

            let metadata = fs::metadata(&file_path)?;
            let file_mtime = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            let file_size = metadata.len();

            let project = cwd.clone().unwrap_or_else(|| "unknown".to_string());

            sessions.push(Session {
                source: Source::Codex,
                session_id,
                project,
                directory: cwd.unwrap_or_default(),
                title,
                started_at: started_at.unwrap_or_else(Utc::now),
                model,
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

            let event: CodexEvent = match serde_json::from_str(&line) {
                Ok(e) => e,
                Err(_) => continue,
            };

            if event.event_type != "response_item" {
                continue;
            }

            let item: ResponseItem = match serde_json::from_value(event.payload) {
                Ok(i) => i,
                Err(_) => continue,
            };

            let role = match item.role.as_deref() {
                Some("user") => Role::User,
                Some("assistant") => Role::Assistant,
                // Skip developer (system instructions) and other roles
                _ => continue,
            };

            let blocks = match item.content {
                Some(b) => b,
                None => continue,
            };

            let (text, tool_names) = extract_content(&blocks);

            if text.trim().is_empty() {
                continue;
            }

            let timestamp = event
                .timestamp
                .as_deref()
                .and_then(|ts| ts.parse::<DateTime<Utc>>().ok());

            messages.push(Message {
                role,
                text,
                timestamp,
                tool_names,
            });
        }

        Ok(messages)
    }

    fn watch_dirs(&self) -> Vec<(PathBuf, bool)> {
        vec![(self.sessions_dir.clone(), true)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_session_meta() {
        let json = r#"{"timestamp":"2026-03-29T21:05:52.143Z","type":"session_meta","payload":{"id":"019d3b6a-8a99-72b0-be89-0cd560e54c9d","cwd":"/Users/m/repos/test","source":"cli","model_provider":"openai"}}"#;
        let event: CodexEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.event_type, "session_meta");
        let meta: SessionMeta = serde_json::from_value(event.payload).unwrap();
        assert_eq!(
            meta.id.as_deref(),
            Some("019d3b6a-8a99-72b0-be89-0cd560e54c9d")
        );
        assert_eq!(meta.cwd.as_deref(), Some("/Users/m/repos/test"));
    }

    #[test]
    fn parse_response_item_user() {
        let json = r#"{"timestamp":"2026-03-29T21:06:00Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"Hello!"}]}}"#;
        let event: CodexEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.event_type, "response_item");
        let item: ResponseItem = serde_json::from_value(event.payload).unwrap();
        assert_eq!(item.role.as_deref(), Some("user"));
        let (text, tools) = extract_content(&item.content.unwrap());
        assert_eq!(text, "Hello!");
        assert!(tools.is_empty());
    }

    #[test]
    fn parse_response_item_assistant() {
        let json = r#"{"timestamp":"2026-03-29T21:06:01Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Hi there!"}]}}"#;
        let event: CodexEvent = serde_json::from_str(json).unwrap();
        let item: ResponseItem = serde_json::from_value(event.payload).unwrap();
        assert_eq!(item.role.as_deref(), Some("assistant"));
        let (text, _) = extract_content(&item.content.unwrap());
        assert_eq!(text, "Hi there!");
    }

    #[test]
    fn skip_developer_role() {
        let json = r#"{"timestamp":"2026-03-29T21:05:52Z","type":"response_item","payload":{"type":"message","role":"developer","content":[{"type":"input_text","text":"System instructions"}]}}"#;
        let event: CodexEvent = serde_json::from_str(json).unwrap();
        let item: ResponseItem = serde_json::from_value(event.payload).unwrap();
        assert_eq!(item.role.as_deref(), Some("developer"));
        // developer should not map to User or Assistant
    }

    #[test]
    fn skip_environment_context() {
        let blocks = vec![ContentBlock {
            block_type: Some("input_text".into()),
            text: Some("<environment_context>\n  <cwd>/foo</cwd>\n</environment_context>".into()),
            name: None,
        }];
        let (text, _) = extract_content(&blocks);
        assert!(text.is_empty());
    }

    #[test]
    fn extract_multiple_blocks() {
        let blocks = vec![
            ContentBlock {
                block_type: Some("output_text".into()),
                text: Some("First part.".into()),
                name: None,
            },
            ContentBlock {
                block_type: Some("output_text".into()),
                text: Some("Second part.".into()),
                name: None,
            },
        ];
        let (text, _) = extract_content(&blocks);
        assert_eq!(text, "First part.\nSecond part.");
    }

    #[test]
    fn extract_function_call() {
        let blocks = vec![
            ContentBlock {
                block_type: Some("output_text".into()),
                text: Some("Let me check.".into()),
                name: None,
            },
            ContentBlock {
                block_type: Some("function_call".into()),
                text: None,
                name: Some("shell".into()),
            },
        ];
        let (text, tools) = extract_content(&blocks);
        assert_eq!(text, "Let me check.");
        assert_eq!(tools, vec!["shell"]);
    }

    #[test]
    fn parse_turn_context() {
        let json = r#"{"timestamp":"2026-03-29T21:06:00Z","type":"turn_context","payload":{"turn_id":"abc","model":"gpt-5.4","cwd":"/tmp"}}"#;
        let event: CodexEvent = serde_json::from_str(json).unwrap();
        let ctx: TurnContext = serde_json::from_value(event.payload).unwrap();
        assert_eq!(ctx.model.as_deref(), Some("gpt-5.4"));
    }

    #[test]
    fn skip_non_response_events() {
        let json = r#"{"timestamp":"2026-03-29T21:06:00Z","type":"event_msg","payload":{"type":"turn_started","turn_id":"abc"}}"#;
        let event: CodexEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.event_type, "event_msg");
        // event_msg should be skipped in load_messages
    }
}
