use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use crate::models::{Message, Role, Session, Source};
use crate::sources::SessionSource;

pub struct CopilotSource {
    session_dir: PathBuf,
}

fn session_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".copilot")
        .join("session-state")
}

impl CopilotSource {
    pub fn new() -> Self {
        Self {
            session_dir: session_dir(),
        }
    }
}

#[derive(Deserialize)]
struct CopilotEvent {
    #[serde(rename = "type")]
    event_type: String,
    data: serde_json::Value,
    timestamp: Option<String>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct WorkspaceYaml {
    id: Option<String>,
    cwd: Option<String>,
    summary: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
}

impl SessionSource for CopilotSource {
    fn name(&self) -> &'static str {
        "copilot"
    }

    fn list_sessions(&self) -> Result<Vec<Session>> {
        let session_dir = &self.session_dir;
        if !session_dir.exists() {
            return Ok(vec![]);
        }

        let mut sessions = Vec::new();

        for entry in fs::read_dir(session_dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let session_id = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            let events_path = path.join("events.jsonl");
            if !events_path.exists() {
                continue;
            }

            // Read workspace.yaml for metadata
            let workspace_path = path.join("workspace.yaml");
            let (cwd, summary, created_at, _updated_at) = if workspace_path.exists() {
                let content = fs::read_to_string(&workspace_path).unwrap_or_default();
                let ws: WorkspaceYaml = serde_yaml::from_str(&content).unwrap_or(WorkspaceYaml {
                    id: None,
                    cwd: None,
                    summary: None,
                    created_at: None,
                    updated_at: None,
                });
                (ws.cwd, ws.summary, ws.created_at, ws.updated_at)
            } else {
                (None, None, None, None)
            };

            let started_at = created_at
                .as_deref()
                .and_then(|ts| ts.parse::<DateTime<Utc>>().ok())
                .unwrap_or_else(Utc::now);

            let metadata = fs::metadata(&events_path)?;
            let file_mtime = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            let file_size = metadata.len();

            let project = cwd.clone().unwrap_or_else(|| "unknown".to_string());

            sessions.push(Session {
                source: Source::Copilot,
                session_id,
                project,
                directory: cwd.unwrap_or_default(),
                title: summary,
                started_at,
                model: None,
                file_path: events_path.to_string_lossy().to_string(),
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

            let event: CopilotEvent = match serde_json::from_str(&line) {
                Ok(e) => e,
                Err(_) => continue,
            };

            let (role, text_field) = match event.event_type.as_str() {
                "user.message" => (Role::User, "content"),
                "assistant.message" => (Role::Assistant, "content"),
                _ => continue,
            };

            let text = event
                .data
                .get(text_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            if text.trim().is_empty() {
                continue;
            }

            let timestamp = event
                .timestamp
                .as_deref()
                .and_then(|ts| ts.parse::<DateTime<Utc>>().ok());

            // Extract tool names from toolRequests if present
            let tool_names: Vec<String> = event
                .data
                .get("toolRequests")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
                        .map(|s| s.to_string())
                        .collect()
                })
                .unwrap_or_default();

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
        vec![(self.session_dir.clone(), true)]
    }
}
