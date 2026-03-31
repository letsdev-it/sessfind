use crate::config::{CHUNK_MAX_CHARS, CHUNK_MIN_CHARS};
use crate::models::{Chunk, Message, Role, Session};

pub fn chunk_session(session: &Session, messages: &[Message]) -> Vec<Chunk> {
    let pairs = pair_messages(messages);
    let mut chunks = Vec::new();
    let mut chunk_index = 0;

    let mut pending_text = String::new();
    let mut pending_ts = session.started_at;

    for (user_msg, assistant_msg) in &pairs {
        let mut pair_text = String::new();

        if let Some(u) = user_msg {
            pair_text.push_str("USER: ");
            pair_text.push_str(&u.text);
            pair_text.push('\n');
            if let Some(ts) = u.timestamp {
                pending_ts = ts;
            }
        }

        if let Some(a) = assistant_msg {
            pair_text.push_str("ASSISTANT: ");
            pair_text.push_str(&a.text);
            if !a.tool_names.is_empty() {
                pair_text.push_str(&format!("\n[tools: {}]", a.tool_names.join(", ")));
            }
            pair_text.push('\n');
            if let Some(ts) = a.timestamp {
                pending_ts = ts;
            }
        }

        // Merge short exchanges into pending
        if pair_text.len() < CHUNK_MIN_CHARS && !pending_text.is_empty() {
            pending_text.push_str(&pair_text);
            continue;
        }

        // If pending + current would exceed max, flush pending first
        if !pending_text.is_empty() && pending_text.len() + pair_text.len() > CHUNK_MAX_CHARS {
            chunks.push(make_chunk(session, &pending_text, chunk_index, pending_ts));
            chunk_index += 1;
            pending_text.clear();
        }

        pending_text.push_str(&pair_text);

        // Flush if over max
        if pending_text.len() > CHUNK_MAX_CHARS {
            // Split into windows
            let windows = split_with_overlap(&pending_text, CHUNK_MAX_CHARS, 1200);
            for window in windows {
                chunks.push(make_chunk(session, &window, chunk_index, pending_ts));
                chunk_index += 1;
            }
            pending_text.clear();
        }
    }

    // Flush remaining
    if !pending_text.trim().is_empty() {
        chunks.push(make_chunk(session, &pending_text, chunk_index, pending_ts));
    }

    chunks
}

fn make_chunk(
    session: &Session,
    text: &str,
    index: usize,
    timestamp: chrono::DateTime<chrono::Utc>,
) -> Chunk {
    Chunk {
        chunk_id: format!(
            "{}:{}:{}",
            session.source.as_str(),
            session.session_id,
            index
        ),
        session_id: session.session_id.clone(),
        source: session.source,
        text: text.to_string(),
        project: session.project.clone(),
        timestamp,
        title: session.title.clone(),
    }
}

/// Pair user messages with their following assistant responses
fn pair_messages(messages: &[Message]) -> Vec<(Option<&Message>, Option<&Message>)> {
    let mut pairs = Vec::new();
    let mut i = 0;

    while i < messages.len() {
        if messages[i].role == Role::User {
            let user = Some(&messages[i]);
            i += 1;
            // Pair user with the first following assistant message
            let assistant = if i < messages.len() && messages[i].role == Role::Assistant {
                let a = Some(&messages[i]);
                i += 1;
                a
            } else {
                None
            };
            pairs.push((user, assistant));
            // Keep any additional consecutive assistant messages as separate pairs
            while i < messages.len() && messages[i].role == Role::Assistant {
                pairs.push((None, Some(&messages[i])));
                i += 1;
            }
        } else {
            // Orphan assistant message
            pairs.push((None, Some(&messages[i])));
            i += 1;
        }
    }

    pairs
}

fn split_with_overlap(text: &str, max_chars: usize, window_chars: usize) -> Vec<String> {
    let mut windows = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let overlap = max_chars.saturating_sub(window_chars);
    let mut start = 0;

    while start < chars.len() {
        let end = (start + max_chars).min(chars.len());
        windows.push(chars[start..end].iter().collect());
        if end >= chars.len() {
            break;
        }
        start += max_chars - overlap;
    }

    windows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Source;
    use chrono::Utc;

    fn make_session() -> Session {
        Session {
            source: Source::ClaudeCode,
            session_id: "test-session".into(),
            project: "/test/project".into(),
            directory: "/test/project".into(),
            title: Some("Test".into()),
            started_at: Utc::now(),
            model: None,
            file_path: "/tmp/test.jsonl".into(),
            file_mtime: 0,
            file_size: 0,
        }
    }

    fn msg(role: Role, text: &str) -> Message {
        Message {
            role,
            text: text.into(),
            timestamp: None,
            tool_names: vec![],
        }
    }

    #[test]
    fn pair_messages_simple() {
        let msgs = vec![msg(Role::User, "hello"), msg(Role::Assistant, "hi there")];
        let pairs = pair_messages(&msgs);
        assert_eq!(pairs.len(), 1);
        assert!(pairs[0].0.is_some());
        assert!(pairs[0].1.is_some());
        assert_eq!(pairs[0].0.unwrap().text, "hello");
        assert_eq!(pairs[0].1.unwrap().text, "hi there");
    }

    #[test]
    fn pair_messages_multiple_exchanges() {
        let msgs = vec![
            msg(Role::User, "q1"),
            msg(Role::Assistant, "a1"),
            msg(Role::User, "q2"),
            msg(Role::Assistant, "a2"),
        ];
        let pairs = pair_messages(&msgs);
        assert_eq!(pairs.len(), 2);
    }

    #[test]
    fn pair_messages_orphan_assistant() {
        let msgs = vec![msg(Role::Assistant, "orphan")];
        let pairs = pair_messages(&msgs);
        assert_eq!(pairs.len(), 1);
        assert!(pairs[0].0.is_none());
        assert_eq!(pairs[0].1.unwrap().text, "orphan");
    }

    #[test]
    fn pair_messages_user_without_assistant() {
        let msgs = vec![msg(Role::User, "question")];
        let pairs = pair_messages(&msgs);
        assert_eq!(pairs.len(), 1);
        assert!(pairs[0].0.is_some());
        assert!(pairs[0].1.is_none());
    }

    #[test]
    fn pair_messages_consecutive_assistants_kept() {
        let msgs = vec![
            msg(Role::User, "q"),
            msg(Role::Assistant, "a1"),
            msg(Role::Assistant, "a2"),
        ];
        let pairs = pair_messages(&msgs);
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0].0.unwrap().text, "q");
        assert_eq!(pairs[0].1.unwrap().text, "a1");
        assert!(pairs[1].0.is_none());
        assert_eq!(pairs[1].1.unwrap().text, "a2");
    }

    #[test]
    fn chunk_session_single_short_exchange() {
        let session = make_session();
        let msgs = vec![
            msg(Role::User, "What is Rust?"),
            msg(Role::Assistant, "A systems programming language."),
        ];
        let chunks = chunk_session(&session, &msgs);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].text.contains("USER: What is Rust?"));
        assert!(
            chunks[0]
                .text
                .contains("ASSISTANT: A systems programming language.")
        );
        assert_eq!(chunks[0].chunk_id, "claude:test-session:0");
        assert_eq!(chunks[0].session_id, "test-session");
    }

    #[test]
    fn chunk_session_empty() {
        let session = make_session();
        let chunks = chunk_session(&session, &[]);
        assert!(chunks.is_empty());
    }

    #[test]
    fn chunk_session_tool_names_included() {
        let session = make_session();
        let msgs = vec![
            msg(Role::User, "read the file"),
            Message {
                role: Role::Assistant,
                text: "Here is the content.".into(),
                timestamp: None,
                tool_names: vec!["Read".into(), "Grep".into()],
            },
        ];
        let chunks = chunk_session(&session, &msgs);
        assert!(chunks[0].text.contains("[tools: Read, Grep]"));
    }

    #[test]
    fn chunk_session_large_splits() {
        let session = make_session();
        // Create a message that exceeds CHUNK_MAX_CHARS
        let big_text = "x".repeat(CHUNK_MAX_CHARS + 1000);
        let msgs = vec![msg(Role::User, &big_text), msg(Role::Assistant, "ok")];
        let chunks = chunk_session(&session, &msgs);
        assert!(chunks.len() > 1, "Should split into multiple chunks");
        for chunk in &chunks {
            assert!(chunk.text.len() <= CHUNK_MAX_CHARS + 100); // some overhead for "USER: " prefix
        }
    }

    #[test]
    fn chunk_ids_sequential() {
        let session = make_session();
        let big_text = "x".repeat(CHUNK_MAX_CHARS + 1000);
        let msgs = vec![msg(Role::User, &big_text), msg(Role::Assistant, "ok")];
        let chunks = chunk_session(&session, &msgs);
        for (i, chunk) in chunks.iter().enumerate() {
            assert!(chunk.chunk_id.ends_with(&format!(":{i}")));
        }
    }

    #[test]
    fn split_with_overlap_basic() {
        let text = "a".repeat(100);
        let windows = split_with_overlap(&text, 40, 30);
        assert!(windows.len() >= 2);
        assert_eq!(windows[0].len(), 40);
        // Windows should overlap
        assert!(windows.len() < 10); // not too many
    }

    #[test]
    fn split_with_overlap_short_text() {
        let text = "short";
        let windows = split_with_overlap(text, 100, 50);
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0], "short");
    }
}
