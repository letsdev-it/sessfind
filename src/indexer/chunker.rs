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
        if !pending_text.is_empty()
            && pending_text.len() + pair_text.len() > CHUNK_MAX_CHARS
        {
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
        chunk_id: format!("{}:{}:{}", session.source.as_str(), session.session_id, index),
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
            // Collect assistant response(s) - take the first one
            let assistant = if i < messages.len() && messages[i].role == Role::Assistant {
                let a = Some(&messages[i]);
                i += 1;
                // Skip additional assistant messages (continuations)
                while i < messages.len() && messages[i].role == Role::Assistant {
                    i += 1;
                }
                a
            } else {
                None
            };
            pairs.push((user, assistant));
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
