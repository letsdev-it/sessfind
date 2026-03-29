use anyhow::Result;
use std::path::Path;
use tantivy::collector::TopDocs;
use tantivy::query::{
    AllQuery, BooleanQuery, EmptyQuery, Occur, PhrasePrefixQuery, Query, QueryParser,
};
use tantivy::schema::*;
use tantivy::tokenizer::{LowerCaser, RemoveLongFilter, SimpleTokenizer, Stemmer, TextAnalyzer};
use tantivy::{DateTime as TantivyDateTime, Index, IndexWriter};

use crate::indexer::chunker;
use crate::indexer::state::IndexState;
use crate::models::{Chunk, SearchResult, Session, Source};
use crate::sources::SessionSource;

/// Custom tokenizer name used for the `text` field (simple + lowercase + stemmer).
const TOKENIZER_NAME: &str = "en_stem";

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

    let text_options = TextOptions::default()
        .set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer(TOKENIZER_NAME)
                .set_index_option(IndexRecordOption::WithFreqsAndPositions),
        )
        .set_stored();
    builder.add_text_field("text", text_options);

    builder.add_date_field("timestamp", INDEXED | STORED | FAST);
    builder.add_text_field("title", STRING | STORED);
    builder.build()
}

/// Register the stemming tokenizer on the index so both indexing and querying use it.
fn register_tokenizer(index: &Index) {
    let analyzer = TextAnalyzer::builder(SimpleTokenizer::default())
        .filter(RemoveLongFilter::limit(40))
        .filter(LowerCaser)
        .filter(Stemmer::default()) // English stemmer
        .build();
    index.tokenizers().register(TOKENIZER_NAME, analyzer);
}

impl IndexEngine {
    pub fn open(data_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(data_dir)?;

        let schema = build_schema();
        let index_path = data_dir.join("tantivy");
        std::fs::create_dir_all(&index_path)?;

        let mut rebuilt = false;
        let index = if index_path.join("meta.json").exists() {
            let existing = Index::open_in_dir(&index_path)?;
            // Detect tokenizer change: if the existing schema's text field doesn't
            // use our tokenizer, wipe and recreate so stemming takes effect.
            let needs_rebuild = {
                let s = existing.schema();
                let ok = s.get_field("text").ok().map_or(false, |f| {
                    if let FieldType::Str(ref opts) = *s.get_field_entry(f).field_type() {
                        opts.get_indexing_options()
                            .map_or(false, |o| o.tokenizer() == TOKENIZER_NAME)
                    } else {
                        false
                    }
                });
                !ok
            };
            if needs_rebuild {
                drop(existing);
                std::fs::remove_dir_all(&index_path)?;
                std::fs::create_dir_all(&index_path)?;
                eprintln!("Recreating search index (tokenizer upgrade)…");
                rebuilt = true;
                Index::create_in_dir(&index_path, schema.clone())?
            } else {
                existing
            }
        } else {
            Index::create_in_dir(&index_path, schema.clone())?
        };

        register_tokenizer(&index);

        let state = IndexState::open(&data_dir.join("index_state.db"))?;

        // If the tantivy index was wiped (tokenizer upgrade), clear state so
        // every session is re-indexed on the next run.
        if rebuilt {
            state.clear()?;
        }

        Ok(Self {
            index,
            schema,
            state,
        })
    }

    pub fn index_source(&self, source: &dyn SessionSource, force: bool) -> Result<IndexStats> {
        let sessions = source.list_sessions()?;
        let mut stats = IndexStats {
            total_sessions: sessions.len(),
            ..Default::default()
        };

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

            if let Some(filter) = &params.source
                && source_val != filter
            {
                continue;
            }

            let project_val = doc
                .get_first(project_f)
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if let Some(filter) = &params.project
                && !project_val.to_lowercase().contains(&filter.to_lowercase())
            {
                continue;
            }

            let ts_val = doc
                .get_first(timestamp_f)
                .and_then(|v| v.as_datetime())
                .map(|dt| {
                    chrono::DateTime::from_timestamp(dt.into_timestamp_secs(), 0)
                        .unwrap_or_default()
                })
                .unwrap_or_default();

            if let Some(after) = params.after
                && ts_val < after
            {
                continue;
            }
            if let Some(before) = params.before
                && ts_val > before
            {
                continue;
            }

            let full_text = doc.get_first(text_f).and_then(|v| v.as_str()).unwrap_or("");

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
                source: Source::parse_source(source_val).unwrap_or(Source::ClaudeCode),
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
                    source: Source::parse_source(
                        doc.get_first(source_f)
                            .and_then(|v| v.as_str())
                            .unwrap_or(""),
                    )
                    .unwrap_or(Source::ClaudeCode),
                    project: doc
                        .get_first(project_f)
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    timestamp: ts_val,
                    title: if title_val.is_empty() {
                        None
                    } else {
                        Some(title_val)
                    },
                    snippet: doc
                        .get_first(text_f)
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
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
                let text = doc.get_first(text_f).and_then(|v| v.as_str()).unwrap_or("");
                // First line as snippet preview
                let preview: String = text
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .take(2)
                    .collect::<Vec<_>>()
                    .join(" | ");

                Some(SearchResult {
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
                    source: Source::parse_source(
                        doc.get_first(source_f)
                            .and_then(|v| v.as_str())
                            .unwrap_or(""),
                    )
                    .unwrap_or(Source::ClaudeCode),
                    project: doc
                        .get_first(project_f)
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    timestamp: ts_val,
                    title: if title_val.is_empty() {
                        None
                    } else {
                        Some(title_val)
                    },
                    snippet: preview,
                    score,
                })
            })
            .collect();

        results.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(results)
    }

    /// Dump all chunks with full text (for semantic plugin).
    pub fn dump_all_chunks(&self) -> Result<Vec<sessfind_common::DumpChunk>> {
        let reader = self.index.reader()?;
        let searcher = reader.searcher();

        let chunk_id_f = self.schema.get_field("chunk_id").unwrap();
        let session_id_f = self.schema.get_field("session_id").unwrap();
        let source_f = self.schema.get_field("source").unwrap();
        let project_f = self.schema.get_field("project").unwrap();
        let text_f = self.schema.get_field("text").unwrap();
        let timestamp_f = self.schema.get_field("timestamp").unwrap();
        let title_f = self.schema.get_field("title").unwrap();

        let query = AllQuery;
        let top_docs = searcher.search(&query, &TopDocs::with_limit(100_000))?;

        let mut chunks: Vec<sessfind_common::DumpChunk> = top_docs
            .into_iter()
            .filter_map(|(_score, addr)| {
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

                Some(sessfind_common::DumpChunk {
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
                    source: Source::parse_source(
                        doc.get_first(source_f)
                            .and_then(|v| v.as_str())
                            .unwrap_or(""),
                    )
                    .unwrap_or(Source::ClaudeCode),
                    project: doc
                        .get_first(project_f)
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    timestamp: ts_val,
                    title: if title_val.is_empty() {
                        None
                    } else {
                        Some(title_val)
                    },
                    text: doc
                        .get_first(text_f)
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                })
            })
            .collect();

        chunks.sort_by(|a, b| a.chunk_id.cmp(&b.chunk_id));
        Ok(chunks)
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

/// Build a tantivy query from user input.
///
/// Handles prefix wildcards (`hel*`) via RegexQuery, boolean operators
/// (`+must -exclude`), exact phrases (`"hello world"`), and plain terms.
/// The stemming tokenizer is applied to non-wildcard terms by the QueryParser.
fn parse_fts_user_query(index: &Index, text_field: Field, query: &str) -> Result<Box<dyn Query>> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Box::new(EmptyQuery));
    }

    // Detect any trailing-* tokens that need custom prefix handling.
    let tokens: Vec<&str> = query.split_whitespace().collect();
    let has_star = tokens.iter().any(|t| {
        let t = t.trim_start_matches(['+', '-']);
        t.ends_with('*') && t.len() > 1 && !t.starts_with('"')
    });

    if !has_star {
        let qp = QueryParser::for_index(index, vec![text_field]);
        return Ok(Box::new(qp.parse_query(query)?));
    }

    // Build boolean query mixing prefix regexes with normal terms.
    let qp = QueryParser::for_index(index, vec![text_field]);
    let mut subs: Vec<(Occur, Box<dyn Query>)> = Vec::new();

    for raw in &tokens {
        let (occur, body) = if let Some(rest) = raw.strip_prefix('+') {
            (Occur::Must, rest)
        } else if let Some(rest) = raw.strip_prefix('-') {
            (Occur::MustNot, rest)
        } else {
            (Occur::Should, *raw)
        };

        if body.ends_with('*') && body.len() > 1 && !body.starts_with('"') {
            let base = &body[..body.len() - 1];
            let lower = base.to_lowercase();
            let term = tantivy::Term::from_field_text(text_field, &lower);
            let sub: Box<dyn Query> = Box::new(PhrasePrefixQuery::new(vec![term]));
            subs.push((occur, sub));
        } else {
            let sub = qp.parse_query(body).map_err(|e| anyhow::anyhow!(e))?;
            subs.push((occur, Box::new(sub)));
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
            let t = t.trim_start_matches(['+', '-']);
            let t = t.strip_suffix('*').unwrap_or(t);
            t.to_lowercase().chars().collect::<Vec<_>>()
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

pub use crate::models::SearchParams;

#[cfg(test)]
mod tests {
    use super::*;
    use tantivy::collector::TopDocs;

    fn make_test_index() -> (Index, Schema, Field) {
        let schema = build_schema();
        let index = Index::create_in_ram(schema.clone());
        register_tokenizer(&index);
        let text_field = schema.get_field("text").unwrap();

        let mut writer = index.writer(15_000_000).unwrap();
        let chunk_id = schema.get_field("chunk_id").unwrap();
        let session_id = schema.get_field("session_id").unwrap();
        let source = schema.get_field("source").unwrap();
        let project = schema.get_field("project").unwrap();
        let timestamp = schema.get_field("timestamp").unwrap();
        let title = schema.get_field("title").unwrap();

        let mut doc = tantivy::TantivyDocument::new();
        doc.add_text(chunk_id, "c1");
        doc.add_text(session_id, "s1");
        doc.add_text(source, "ClaudeCode");
        doc.add_text(project, "test-project");
        doc.add_text(text_field, "hello world running tests with helpers");
        doc.add_date(timestamp, TantivyDateTime::from_timestamp_secs(1700000000));
        doc.add_text(title, "");
        writer.add_document(doc).unwrap();
        writer.commit().unwrap();

        (index, schema, text_field)
    }

    #[test]
    fn prefix_query_matches() {
        let (index, _, text_field) = make_test_index();
        // "hel*" should match "hello" and "helpers"
        let q = parse_fts_user_query(&index, text_field, "hel*").unwrap();
        let reader = index.reader().unwrap();
        let searcher = reader.searcher();
        let results = searcher.search(&q, &TopDocs::with_limit(10)).unwrap();
        assert!(!results.is_empty(), "hel* should match hello/helpers");
    }

    #[test]
    fn stemming_matches() {
        let (index, _, text_field) = make_test_index();
        // "runs" should match "running" via stemmer (both stem to "run")
        let q = parse_fts_user_query(&index, text_field, "runs").unwrap();
        let reader = index.reader().unwrap();
        let searcher = reader.searcher();
        let results = searcher.search(&q, &TopDocs::with_limit(10)).unwrap();
        assert!(!results.is_empty(), "runs should match running via stemmer");
    }
}
