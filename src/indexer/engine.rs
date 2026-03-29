use anyhow::Result;
use std::path::Path;
use tantivy::collector::TopDocs;
use tantivy::query::{AllQuery, BooleanQuery, EmptyQuery, Occur, Query, QueryParser, RegexQuery};
use tantivy::schema::*;
use tantivy::{DateTime as TantivyDateTime, Index, IndexWriter};

use crate::indexer::chunker;
use crate::indexer::state::IndexState;
use crate::models::{Chunk, SearchResult, Session, Source};
use crate::sources::SessionSource;

pub struct IndexEngine {
    index: Index,
    schema: Schema,
    state: IndexState,
}

fn build_schema() -> Schema {
    let mut builder = Schema::builder();
    builder.add_text_field("chunk_id", STRING | STORED);
    builder.add_text_field("session_id", STRING | STORED);
    builder.add_text_field("source", STRING | STORED);
    builder.add_text_field("project", STRING | STORED);
    builder.add_text_field("text", TEXT | STORED);
    builder.add_date_field("timestamp", INDEXED | STORED | FAST);
    builder.add_text_field("title", STRING | STORED);
    builder.build()
}

impl IndexEngine {
    pub fn open(data_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(data_dir)?;

        let schema = build_schema();
        let index_path = data_dir.join("tantivy");
        std::fs::create_dir_all(&index_path)?;

        let index = if index_path.join("meta.json").exists() {
            Index::open_in_dir(&index_path)?
        } else {
            Index::create_in_dir(&index_path, schema.clone())?
        };

        let state = IndexState::open(&data_dir.join("index_state.db"))?;

        Ok(Self {
            index,
            schema,
            state,
        })
    }

    pub fn index_source(&self, source: &dyn SessionSource, force: bool) -> Result<IndexStats> {
        let sessions = source.list_sessions()?;
        let mut stats = IndexStats::default();
        stats.total_sessions = sessions.len();

        let sessions_to_index: Vec<&Session> = if force {
            sessions.iter().collect()
        } else {
            sessions
                .iter()
                .filter(|s| !self.state.is_current(s))
                .collect()
        };

        stats.new_sessions = sessions_to_index.len();
        if sessions_to_index.is_empty() {
            return Ok(stats);
        }

        let mut writer: IndexWriter = self.index.writer(50_000_000)?;

        for session in &sessions_to_index {
            let messages = match source.load_messages(session) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!(
                        "Warning: failed to load session {}: {}",
                        session.session_id, e
                    );
                    continue;
                }
            };

            let chunks = chunker::chunk_session(session, &messages);
            stats.total_chunks += chunks.len();

            // Delete old chunks for this session (for re-indexing)
            let _chunk_id_field = self.schema.get_field("chunk_id").unwrap();
            let session_id_field = self.schema.get_field("session_id").unwrap();
            writer.delete_term(tantivy::Term::from_field_text(
                session_id_field,
                &session.session_id,
            ));

            for chunk in &chunks {
                self.add_chunk(&mut writer, chunk)?;
            }

            self.state.mark_indexed(session)?;
        }

        writer.commit()?;
        Ok(stats)
    }

    fn add_chunk(&self, writer: &mut IndexWriter, chunk: &Chunk) -> Result<()> {
        let chunk_id = self.schema.get_field("chunk_id").unwrap();
        let session_id = self.schema.get_field("session_id").unwrap();
        let source = self.schema.get_field("source").unwrap();
        let project = self.schema.get_field("project").unwrap();
        let text = self.schema.get_field("text").unwrap();
        let timestamp = self.schema.get_field("timestamp").unwrap();
        let title = self.schema.get_field("title").unwrap();

        let ts = TantivyDateTime::from_timestamp_secs(chunk.timestamp.timestamp());

        let mut doc = tantivy::TantivyDocument::new();
        doc.add_text(chunk_id, &chunk.chunk_id);
        doc.add_text(session_id, &chunk.session_id);
        doc.add_text(source, chunk.source.as_str());
        doc.add_text(project, &chunk.project);
        doc.add_text(text, &chunk.text);
        doc.add_date(timestamp, ts);
        doc.add_text(title, chunk.title.as_deref().unwrap_or(""));

        writer.add_document(doc)?;
        Ok(())
    }

    pub fn search(&self, params: &SearchParams) -> Result<Vec<SearchResult>> {
        let reader = self.index.reader()?;
        let searcher = reader.searcher();

        let text_field = self.schema.get_field("text").unwrap();
        let query = parse_fts_user_query(&self.index, text_field, &params.query)?;

        let top_docs = searcher.search(&query, &TopDocs::with_limit(params.limit * 3))?;

        let chunk_id_f = self.schema.get_field("chunk_id").unwrap();
        let session_id_f = self.schema.get_field("session_id").unwrap();
        let source_f = self.schema.get_field("source").unwrap();
        let project_f = self.schema.get_field("project").unwrap();
        let text_f = self.schema.get_field("text").unwrap();
        let timestamp_f = self.schema.get_field("timestamp").unwrap();
        let title_f = self.schema.get_field("title").unwrap();

        let mut results = Vec::new();

        for (score, doc_address) in top_docs {
            let doc: tantivy::TantivyDocument = searcher.doc(doc_address)?;

            let source_val = doc
                .get_first(source_f)
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if let Some(filter) = &params.source {
                if source_val != filter {
                    continue;
                }
            }

            let project_val = doc
                .get_first(project_f)
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if let Some(filter) = &params.project {
                if !project_val.to_lowercase().contains(&filter.to_lowercase()) {
                    continue;
                }
            }

            let ts_val = doc
                .get_first(timestamp_f)
                .and_then(|v| v.as_datetime())
                .map(|dt| {
                    chrono::DateTime::from_timestamp(dt.into_timestamp_secs(), 0)
                        .unwrap_or_default()
                })
                .unwrap_or_default();

            if let Some(after) = params.after {
                if ts_val < after {
                    continue;
                }
            }
            if let Some(before) = params.before {
                if ts_val > before {
                    continue;
                }
            }

            let full_text = doc
                .get_first(text_f)
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let snippet = make_snippet(full_text, &params.query, 150);

            let title_val = doc
                .get_first(title_f)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            results.push(SearchResult {
                chunk_id: doc
                    .get_first(chunk_id_f)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                session_id: doc
                    .get_first(session_id_f)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                source: Source::from_str(source_val).unwrap_or(Source::ClaudeCode),
                project: project_val.to_string(),
                timestamp: ts_val,
                title: if title_val.is_empty() {
                    None
                } else {
                    Some(title_val)
                },
                snippet,
                score,
            });

            if results.len() >= params.limit {
                break;
            }
        }

        Ok(results)
    }

    pub fn get_session_chunks(&self, session_id: &str) -> Result<Vec<SearchResult>> {
        let reader = self.index.reader()?;
        let searcher = reader.searcher();

        let session_id_field = self.schema.get_field("session_id").unwrap();
        let query = tantivy::query::TermQuery::new(
            tantivy::Term::from_field_text(session_id_field, session_id),
            tantivy::schema::IndexRecordOption::Basic,
        );

        let top_docs = searcher.search(&query, &TopDocs::with_limit(1000))?;

        let chunk_id_f = self.schema.get_field("chunk_id").unwrap();
        let session_id_f = self.schema.get_field("session_id").unwrap();
        let source_f = self.schema.get_field("source").unwrap();
        let project_f = self.schema.get_field("project").unwrap();
        let text_f = self.schema.get_field("text").unwrap();
        let timestamp_f = self.schema.get_field("timestamp").unwrap();
        let title_f = self.schema.get_field("title").unwrap();

        let mut results: Vec<SearchResult> = top_docs
            .into_iter()
            .filter_map(|(score, addr)| {
                let doc: tantivy::TantivyDocument = searcher.doc(addr).ok()?;
                let ts_val = doc
                    .get_first(timestamp_f)
                    .and_then(|v| v.as_datetime())
                    .map(|dt| {
                        chrono::DateTime::from_timestamp(dt.into_timestamp_secs(), 0)
                            .unwrap_or_default()
                    })
                    .unwrap_or_default();
                let title_val = doc
                    .get_first(title_f)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                Some(SearchResult {
                    chunk_id: doc.get_first(chunk_id_f).and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    session_id: doc.get_first(session_id_f).and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    source: Source::from_str(doc.get_first(source_f).and_then(|v| v.as_str()).unwrap_or("")).unwrap_or(Source::ClaudeCode),
                    project: doc.get_first(project_f).and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    timestamp: ts_val,
                    title: if title_val.is_empty() { None } else { Some(title_val) },
                    snippet: doc.get_first(text_f).and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    score,
                })
            })
            .collect();

        // Sort by chunk_id to maintain order
        results.sort_by(|a, b| a.chunk_id.cmp(&b.chunk_id));
        Ok(results)
    }

    pub fn list_all_chunks(&self) -> Result<Vec<SearchResult>> {
        let reader = self.index.reader()?;
        let searcher = reader.searcher();

        let chunk_id_f = self.schema.get_field("chunk_id").unwrap();
        let session_id_f = self.schema.get_field("session_id").unwrap();
        let source_f = self.schema.get_field("source").unwrap();
        let project_f = self.schema.get_field("project").unwrap();
        let text_f = self.schema.get_field("text").unwrap();
        let timestamp_f = self.schema.get_field("timestamp").unwrap();
        let title_f = self.schema.get_field("title").unwrap();

        let query = tantivy::query::AllQuery;
        let top_docs = searcher.search(&query, &TopDocs::with_limit(100_000))?;

        let mut results: Vec<SearchResult> = top_docs
            .into_iter()
            .filter_map(|(score, addr)| {
                let doc: tantivy::TantivyDocument = searcher.doc(addr).ok()?;
                let ts_val = doc
                    .get_first(timestamp_f)
                    .and_then(|v| v.as_datetime())
                    .map(|dt| {
                        chrono::DateTime::from_timestamp(dt.into_timestamp_secs(), 0)
                            .unwrap_or_default()
                    })
                    .unwrap_or_default();
                let title_val = doc
                    .get_first(title_f)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let text = doc
                    .get_first(text_f)
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                // First line as snippet preview
                let preview: String = text
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .take(2)
                    .collect::<Vec<_>>()
                    .join(" | ");

                Some(SearchResult {
                    chunk_id: doc.get_first(chunk_id_f).and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    session_id: doc.get_first(session_id_f).and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    source: Source::from_str(doc.get_first(source_f).and_then(|v| v.as_str()).unwrap_or("")).unwrap_or(Source::ClaudeCode),
                    project: doc.get_first(project_f).and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    timestamp: ts_val,
                    title: if title_val.is_empty() { None } else { Some(title_val) },
                    snippet: preview,
                    score,
                })
            })
            .collect();

        results.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(results)
    }

    #[allow(dead_code)]
    pub fn clear_index(&self) -> Result<()> {
        let mut writer: IndexWriter = self.index.writer(50_000_000)?;
        writer.delete_all_documents()?;
        writer.commit()?;
        self.state.clear()?;
        Ok(())
    }

    pub fn session_count(&self, source: Option<&str>) -> Result<usize> {
        self.state.count(source)
    }
}

/// Use simple OR-of-tokens parsing (supports trailing `*` per token). Full tantivy grammar when
/// boolean / field / grouping syntax is present.
fn should_use_simple_or_split(query: &str) -> bool {
    let upper = query.to_ascii_uppercase();
    if upper.contains(" AND ") || upper.contains(" OR ") || upper.contains(" NOT ") {
        return false;
    }
    if query.contains('(') || query.contains(')') {
        return false;
    }
    if query.contains(':') {
        return false;
    }
    true
}

/// Split on ASCII whitespace outside of `"` / `'`, honoring backslash escapes inside quotes.
fn split_fts_tokens_outside_quotes(query: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut buf = String::new();
    let mut in_dq = false;
    let mut in_sq = false;
    let mut escape = false;

    for c in query.chars() {
        if escape {
            buf.push(c);
            escape = false;
            continue;
        }
        if (in_dq || in_sq) && c == '\\' {
            escape = true;
            buf.push(c);
            continue;
        }
        match c {
            '"' if !in_sq => {
                in_dq = !in_dq;
                buf.push(c);
            }
            '\'' if !in_dq => {
                in_sq = !in_sq;
                buf.push(c);
            }
            c if c.is_whitespace() && !in_dq && !in_sq => {
                if !buf.is_empty() {
                    parts.push(std::mem::take(&mut buf));
                }
            }
            _ => buf.push(c),
        }
    }
    if !buf.is_empty() {
        parts.push(buf);
    }
    parts
}

fn strip_leading_occur(s: &str) -> (Occur, &str) {
    let s = s.trim_start();
    if let Some(rest) = s.strip_prefix('+') {
        (Occur::Must, rest.trim_start())
    } else if let Some(rest) = s.strip_prefix('-') {
        (Occur::MustNot, rest.trim_start())
    } else {
        (Occur::Should, s)
    }
}

fn is_unquoted_trailing_star_token(s: &str) -> bool {
    let t = s.trim();
    if t.is_empty() {
        return false;
    }
    if t.starts_with('"') || t.starts_with('\'') {
        return false;
    }
    t.ends_with('*') && !t[..t.len() - 1].contains('*')
}

fn escape_fst_regex_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\\' | '.' | '^' | '$' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' => {
                out.push('\\');
                out.push(c);
            }
            c => out.push(c),
        }
    }
    out
}

fn clause_string_for_parser(occur: Occur, body: &str) -> String {
    match occur {
        Occur::Must => format!("+{body}"),
        Occur::MustNot => format!("-{body}"),
        Occur::Should => body.to_string(),
    }
}

/// Build tantivy query; `token*` (suffix `*` only) becomes a regex prefix match on indexed terms.
fn parse_fts_user_query(index: &Index, text_field: Field, query: &str) -> Result<Box<dyn Query>> {
    let query = query.trim();
    let query_parser = QueryParser::for_index(index, vec![text_field]);

    if query.is_empty() {
        return Ok(Box::new(EmptyQuery));
    }

    if !should_use_simple_or_split(query) {
        return Ok(Box::new(query_parser.parse_query(query)?));
    }

    let tokens = split_fts_tokens_outside_quotes(query);
    if tokens.is_empty() {
        return Ok(Box::new(EmptyQuery));
    }

    let mut any_star = false;
    for t in &tokens {
        let (_, body) = strip_leading_occur(t);
        if is_unquoted_trailing_star_token(body) {
            any_star = true;
            break;
        }
    }

    if !any_star {
        return Ok(Box::new(query_parser.parse_query(query)?));
    }

    let mut subs: Vec<(Occur, Box<dyn Query>)> = Vec::with_capacity(tokens.len());
    for raw in tokens {
        let (occur, body) = strip_leading_occur(&raw);
        let body = body.trim();
        if body.is_empty() {
            continue;
        }

        if is_unquoted_trailing_star_token(body) {
            let sub: Box<dyn Query> = {
                let base = &body[..body.len() - 1];
                if base.is_empty() {
                    Box::new(AllQuery)
                } else {
                    let pat = format!(
                        "^{}.*",
                        escape_fst_regex_literal(&base.to_lowercase())
                    );
                    Box::new(
                        RegexQuery::from_pattern(&pat, text_field)
                            .map_err(|e| anyhow::anyhow!("regex query: {e}"))?,
                    )
                }
            };
            subs.push((occur, sub));
        } else {
            let sub = query_parser
                .parse_query(&clause_string_for_parser(occur, body))
                .map_err(|e| anyhow::anyhow!(e))?;
            subs.push((Occur::Should, sub));
        }
    }

    if subs.is_empty() {
        return Ok(Box::new(EmptyQuery));
    }
    if subs.len() == 1 {
        return Ok(subs.into_iter().next().unwrap().1);
    }
    Ok(Box::new(BooleanQuery::new(subs)))
}

fn make_snippet(text: &str, query: &str, max_len: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    let lower_text: Vec<char> = text.to_lowercase().chars().collect();
    let query_terms: Vec<Vec<char>> = query
        .split_whitespace()
        .map(|t| {
            let t = t.trim_start_matches(|c| c == '+' || c == '-');
            let t = if is_unquoted_trailing_star_token(t) {
                t[..t.len() - 1].to_lowercase()
            } else {
                t.to_lowercase()
            };
            t.chars().collect::<Vec<_>>()
        })
        .collect();

    // Find first occurrence of any query term (char-based position)
    let mut best_pos = 0;
    for term in &query_terms {
        if let Some(pos) = lower_text
            .windows(term.len())
            .position(|w| w == term.as_slice())
        {
            best_pos = pos;
            break;
        }
    }

    // Center snippet around the match
    let start = best_pos.saturating_sub(max_len / 2);
    let end = (start + max_len).min(chars.len());
    let snippet: String = chars[start..end].iter().collect();

    if start > 0 {
        format!("...{}", snippet.trim_start())
    } else {
        snippet
    }
}

#[derive(Debug, Default)]
pub struct IndexStats {
    pub total_sessions: usize,
    pub new_sessions: usize,
    pub total_chunks: usize,
}

pub struct SearchParams {
    pub query: String,
    pub limit: usize,
    pub source: Option<String>,
    pub project: Option<String>,
    pub after: Option<chrono::DateTime<chrono::Utc>>,
    pub before: Option<chrono::DateTime<chrono::Utc>>,
}
