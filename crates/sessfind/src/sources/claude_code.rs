use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::config;
use crate::models::{Message, Role, Session, Source};
use crate::sources::SessionSource;

pub struct ClaudeCodeSource {
    projects_dir: PathBuf,
}

impl ClaudeCodeSource {
    pub fn new() -> Self {
        Self {
            projects_dir: config::claude_projects_dir(),
        }
    }
}

// JSONL line types we care about
#[derive(Deserialize)]
#[allow(dead_code)]
struct RawEntry {
    #[serde(rename = "type")]
    entry_type: Option<String>,
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    cwd: Option<String>,
    slug: Option<String>,
    #[serde(rename = "gitBranch")]
    git_branch: Option<String>,
    timestamp: Option<String>,
    message: Option<RawMessage>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct RawMessage {
    role: Option<String>,
    content: Option<serde_json::Value>,
    model: Option<String>,
}

fn decode_project_path(encoded: &str) -> String {
    // "-Users-m-repos-foo-bar" -> "/Users/m/repos/foo-bar"
    // The encoding replaces '/' with '-', so we need to find path separators
    // Heuristic: known path prefixes help us decode correctly
    if !encoded.starts_with('-') {
        return encoded.to_string();
    }

    // Try to find the actual directory on disk by progressively resolving segments
    let without_leading = &encoded[1..]; // strip leading '-'
    let segments: Vec<&str> = without_leading.split('-').collect();

    let mut path = String::from("/");
    let mut i = 0;
    while i < segments.len() {
        // Try single segment first
        let candidate = format!("{}{}", path, segments[i]);
        if std::path::Path::new(&candidate).exists() {
            path = format!("{}/", candidate);
            i += 1;
        } else {
            // Try joining with next segments using '-' (for dirs like "session-seek")
            let mut found = false;
            for j in (i + 1..segments.len()).rev() {
                let joined = segments[i..=j].join("-");
                let candidate = format!("{}{}", path, joined);
                if std::path::Path::new(&candidate).exists() {
                    path = format!("{}/", candidate);
                    i = j + 1;
                    found = true;
                    break;
                }
            }
            if !found {
                // Fallback: treat remaining as single joined segment
                let remaining = segments[i..].join("-");
                path = format!("{}{}", path, remaining);
                break;
            }
        }
    }

    // Remove trailing slash
    path.trim_end_matches('/').to_string()
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
                        // Skip thinking, tool_result, etc.
                        _ => {}
                    }
                }
            }
        }
        _ => {}
    }

    let raw = texts.join("\n");
    (clean_message_text(&raw), tool_names)
}

/// Strip internal XML tags and meta content from Claude Code messages.
fn clean_message_text(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut remaining = text;

    while let Some(start) = remaining.find('<') {
        // Add text before the tag
        result.push_str(&remaining[..start]);

        if let Some(end) = remaining[start..].find('>') {
            let tag_content = &remaining[start + 1..start + end];
            let tag_name = tag_content
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_start_matches('/');

            // Skip known internal tags and their content
            match tag_name {
                "local-command-caveat"
                | "local-command-stdout"
                | "command-name"
                | "command-message"
                | "command-args"
                | "system-reminder"
                | "user-prompt-submit-hook"
                | "antml:thinking" => {
                    // Find closing tag and skip everything
                    let close_tag = format!("</{tag_name}>");
                    if let Some(close_pos) = remaining.find(&close_tag) {
                        remaining = &remaining[close_pos + close_tag.len()..];
                    } else {
                        remaining = &remaining[start + end + 1..];
                    }
                    continue;
                }
                _ => {
                    // Unknown tag — keep as-is
                    result.push_str(&remaining[start..start + end + 1]);
                    remaining = &remaining[start + end + 1..];
                    continue;
                }
            }
        } else {
            // No closing '>' — keep the rest
            result.push_str(&remaining[start..]);
            break;
        }
    }
    result.push_str(remaining);

    // Clean up common meta lines
    result
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty()
                && !trimmed.starts_with("[Request interrupted by user")
                && !trimmed.starts_with("[Response interrupted by")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn find_session_files(projects_dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in WalkDir::new(projects_dir)
        .min_depth(2)
        .max_depth(2)
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

impl SessionSource for ClaudeCodeSource {
    fn name(&self) -> &'static str {
        "claude"
    }

    fn list_sessions(&self) -> Result<Vec<Session>> {
        if !self.projects_dir.exists() {
            return Ok(vec![]);
        }

        let mut sessions = Vec::new();

        for file_path in find_session_files(&self.projects_dir) {
            // Extract project name from parent dir
            let project_dir = file_path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");
            let project = decode_project_path(project_dir);

            // Session ID from filename
            let session_id = file_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            // File metadata for incremental indexing
            let metadata = fs::metadata(&file_path)?;
            let file_mtime = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            let file_size = metadata.len();

            // Read first few lines to get metadata
            let file = fs::File::open(&file_path)?;
            let reader = BufReader::new(file);
            let mut started_at = None;
            let mut model = None;
            let mut title = None;
            let mut cwd = None;

            for line in reader.lines().take(10) {
                let line = line?;
                if line.is_empty() {
                    continue;
                }
                let entry: RawEntry = match serde_json::from_str(&line) {
                    Ok(e) => e,
                    Err(_) => continue,
                };

                if entry.entry_type.as_deref() == Some("user")
                    || entry.entry_type.as_deref() == Some("assistant")
                {
                    if started_at.is_none() {
                        if let Some(ts) = &entry.timestamp {
                            started_at = ts.parse::<DateTime<Utc>>().ok();
                        }
                    }
                    if cwd.is_none() {
                        cwd = entry.cwd.clone();
                    }
                    if title.is_none() {
                        title = entry.slug.clone();
                    }
                    if model.is_none() {
                        if let Some(msg) = &entry.message {
                            model = msg.model.clone();
                        }
                    }
                }
            }

            sessions.push(Session {
                source: Source::ClaudeCode,
                session_id,
                project: project.clone(),
                directory: cwd.unwrap_or(project),
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

            let entry: RawEntry = match serde_json::from_str(&line) {
                Ok(e) => e,
                Err(_) => continue,
            };

            let role = match entry.entry_type.as_deref() {
                Some("user") => Role::User,
                Some("assistant") => Role::Assistant,
                _ => continue,
            };

            let msg = match &entry.message {
                Some(m) => m,
                None => continue,
            };

            let content = match &msg.content {
                Some(c) => c,
                None => continue,
            };

            let (text, tool_names) = extract_text_from_content(content);

            // Skip empty or system-only messages
            if text.trim().is_empty() {
                continue;
            }

            let timestamp = entry
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
}
