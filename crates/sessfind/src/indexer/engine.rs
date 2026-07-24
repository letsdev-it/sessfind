use anyhow::Result;
use std::collections::HashSet;
use std::path::Path;
use tantivy::collector::{Count, TopDocs};
use tantivy::query::{
    AllQuery, BooleanQuery, EmptyQuery, FuzzyTermQuery, Occur, PhrasePrefixQuery, Query,
    QueryParser,
};
use tantivy::schema::*;
use tantivy::tokenizer::{LowerCaser, RemoveLongFilter, SimpleTokenizer, Stemmer, TextAnalyzer};
use tantivy::{DateTime as TantivyDateTime, Index, IndexWriter};

use crate::indexer::chunker;
use crate::indexer::state::{IndexState, SourceSyncState};
use crate::models::{Chunk, SearchResult, Session, Source};
use crate::sources::SessionSource;

/// Custom tokenizer name used for the `text` field (simple + lowercase + stemmer).
const TOKENIZER_NAME: &str = "en_stem";

pub struct IndexEngine {
    index: Index,
    schema: Schema,
    state: IndexState,
    requires_reindex: bool,
}

fn build_schema() -> Schema {
    let mut builder = Schema::builder();
    builder.add_text_field("chunk_id", STRING | STORED);
    builder.add_text_field("session_key", STRING | STORED);
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
    let search_text_options = TextOptions::default().set_indexing_options(
        TextFieldIndexing::default()
            .set_tokenizer(TOKENIZER_NAME)
            .set_index_option(IndexRecordOption::WithFreqsAndPositions),
    );
    builder.add_text_field("search_text", search_text_options);

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

fn indexed_session_keys(index: &Index, schema: &Schema) -> Result<HashSet<String>> {
    let reader = index.reader()?;
    let searcher = reader.searcher();
    let session_key = schema.get_field("session_key")?;
    let docs = searcher.search(&AllQuery, &TopDocs::with_limit(100_000))?;

    Ok(docs
        .into_iter()
        .filter_map(|(_, address)| {
            let document: tantivy::TantivyDocument = searcher.doc(address).ok()?;
            document
                .get_first(session_key)
                .and_then(|value| value.as_str())
                .map(str::to_owned)
        })
        .collect())
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
                let tokenizer_ok = s.get_field("text").ok().is_some_and(|f| {
                    if let FieldType::Str(ref opts) = *s.get_field_entry(f).field_type() {
                        opts.get_indexing_options()
                            .is_some_and(|o| o.tokenizer() == TOKENIZER_NAME)
                    } else {
                        false
                    }
                });
                !tokenizer_ok
                    || s.get_field("session_key").is_err()
                    || s.get_field("search_text").is_err()
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

        let state = IndexState::open(&data_dir.join("index_state.db"))?;

        let catalog_mismatch = state.session_keys()? != indexed_session_keys(&index, &schema)?;
        // The SQLite state is only valid when it describes the documents that
        // are actually present in Tantivy. If a migration or interrupted run
        // leaves them out of sync, keeping the state would permanently skip
        // the missing sessions as "unchanged".
        let requires_reindex = rebuilt || catalog_mismatch;
        if requires_reindex {
            if !rebuilt {
                eprintln!("Recreating search index (catalog recovery)…");
            }
            state.clear()?;
        }

        register_tokenizer(&index);

        Ok(Self {
            index,
            schema,
            state,
            requires_reindex,
        })
    }

    pub fn requires_reindex(&self) -> bool {
        self.requires_reindex
    }

    pub fn index_source(&self, source: &dyn SessionSource, force: bool) -> Result<IndexStats> {
        let source_name = source.name();
        match self.index_source_inner(source, force) {
            Ok(stats) => Ok(stats),
            Err(error) => {
                let _ = self
                    .state
                    .mark_source_failure(source_name, &error.to_string());
                Err(error)
            }
        }
    }

    fn index_source_inner(&self, source: &dyn SessionSource, force: bool) -> Result<IndexStats> {
        let source_name = source.name();
        let sessions = source.list_sessions()?;
        let mut stats = IndexStats::default();

        let state_ids = self.state.session_ids(source_name)?;
        let document_ids = self.indexed_session_ids(source_name)?;
        let indexed_ids: HashSet<String> = state_ids.union(&document_ids).cloned().collect();
        let discovered_ids: HashSet<String> = sessions
            .iter()
            .map(|session| session.session_id.clone())
            .collect();
        let removed_ids: Vec<String> = indexed_ids.difference(&discovered_ids).cloned().collect();
        let sessions_to_index: Vec<&Session> = sessions
            .iter()
            .filter(|session| force || !self.state.is_current(session))
            .collect();

        for session in &sessions {
            if self.state.is_current(session) && !force {
                stats.unchanged_sessions += 1;
            }
        }
        stats.removed_sessions = removed_ids.len();

        if sessions_to_index.is_empty() && removed_ids.is_empty() {
            stats.total_chunks = self.chunk_count(Some(source_name))?;
            self.state.mark_source_success(source_name)?;
            return Ok(stats);
        }

        let mut writer: IndexWriter = self.index.writer(50_000_000)?;
        let session_key_field = self.schema.get_field("session_key").unwrap();
        for session_id in &removed_ids {
            writer.delete_term(tantivy::Term::from_field_text(
                session_key_field,
                &format!("{source_name}:{session_id}"),
            ));
        }
        let mut indexed_sessions = Vec::new();
        for session in &sessions_to_index {
            let messages = match source.load_messages(session) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!(
                        "Warning: failed to load session {}: {}",
                        session.session_id, e
                    );
                    stats.skipped_sessions += 1;
                    continue;
                }
            };

            let chunks = chunker::chunk_session(session, &messages);
            if chunks.is_empty() {
                // A session without indexable content has no Tantivy document,
                // so it must not be recorded as indexed in SQLite. Otherwise
                // the catalog integrity check would see a permanent mismatch.
                stats.skipped_sessions += 1;
                continue;
            }
            stats.total_chunks += chunks.len();
            if indexed_ids.contains(&session.session_id) {
                stats.updated_sessions += 1;
            } else {
                stats.new_sessions += 1;
            }

            // Delete old chunks for this session (for re-indexing)
            writer.delete_term(tantivy::Term::from_field_text(
                session_key_field,
                &format!("{}:{}", session.source.as_str(), session.session_id),
            ));

            for chunk in &chunks {
                self.add_chunk(&mut writer, chunk)?;
            }

            indexed_sessions.push(*session);
        }

        writer.commit()?;
        for session in indexed_sessions {
            self.state.mark_indexed(session)?;
        }
        self.state.remove_sessions(source_name, &removed_ids)?;
        stats.total_chunks = self.chunk_count(Some(source_name))?;
        self.state.mark_source_success(source_name)?;
        Ok(stats)
    }

    fn add_chunk(&self, writer: &mut IndexWriter, chunk: &Chunk) -> Result<()> {
        let chunk_id = self.schema.get_field("chunk_id").unwrap();
        let session_key = self.schema.get_field("session_key").unwrap();
        let session_id = self.schema.get_field("session_id").unwrap();
        let source = self.schema.get_field("source").unwrap();
        let project = self.schema.get_field("project").unwrap();
        let text = self.schema.get_field("text").unwrap();
        let search_text = self.schema.get_field("search_text").unwrap();
        let timestamp = self.schema.get_field("timestamp").unwrap();
        let title = self.schema.get_field("title").unwrap();

        let ts = TantivyDateTime::from_timestamp_secs(chunk.timestamp.timestamp());

        let mut doc = tantivy::TantivyDocument::new();
        doc.add_text(chunk_id, &chunk.chunk_id);
        doc.add_text(
            session_key,
            format!("{}:{}", chunk.source.as_str(), chunk.session_id),
        );
        doc.add_text(session_id, &chunk.session_id);
        doc.add_text(source, chunk.source.as_str());
        doc.add_text(project, &chunk.project);
        doc.add_text(text, &chunk.text);
        doc.add_text(
            search_text,
            format!(
                "{}\n{}\n{}",
                chunk.title.as_deref().unwrap_or(""),
                chunk.project,
                chunk.text
            ),
        );
        doc.add_date(timestamp, ts);
        doc.add_text(title, chunk.title.as_deref().unwrap_or(""));

        writer.add_document(doc)?;
        Ok(())
    }

    fn indexed_session_ids(&self, source_name: &str) -> Result<HashSet<String>> {
        let reader = self.index.reader()?;
        let searcher = reader.searcher();
        let session_key = self.schema.get_field("session_key")?;
        let prefix = format!("{source_name}:");
        let docs = searcher.search(&AllQuery, &TopDocs::with_limit(100_000))?;

        Ok(docs
            .into_iter()
            .filter_map(|(_, address)| {
                let document: tantivy::TantivyDocument = searcher.doc(address).ok()?;
                document
                    .get_first(session_key)
                    .and_then(|value| value.as_str())
                    .and_then(|key| key.strip_prefix(&prefix))
                    .map(str::to_owned)
            })
            .collect())
    }

    pub fn search(&self, params: &SearchParams) -> Result<Vec<SearchResult>> {
        let reader = self.index.reader()?;
        let searcher = reader.searcher();

        let text_field = self.schema.get_field("search_text").unwrap();
        let query = parse_fts_user_query(&self.index, text_field, &params.query)?;

        let top_docs = searcher.search(&query, &TopDocs::with_limit(params.limit * 3))?;

        self.collect_results(&searcher, &top_docs, params)
    }

    /// Fuzzy search using Levenshtein distance on individual terms.
    /// Each word in the query becomes a FuzzyTermQuery (distance 1-2 depending on length).
    /// Words are combined with OR (any match), results ranked by score.
    pub fn search_fuzzy(&self, params: &SearchParams) -> Result<Vec<SearchResult>> {
        let reader = self.index.reader()?;
        let searcher = reader.searcher();

        let text_field = self.schema.get_field("search_text").unwrap();

        // Tokenize query: lowercase and split on whitespace
        let terms: Vec<String> = params
            .query
            .to_lowercase()
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        if terms.is_empty() {
            return Ok(vec![]);
        }

        // Build a BooleanQuery: each term is a FuzzyTermQuery (SHOULD = OR)
        let sub_queries: Vec<(Occur, Box<dyn Query>)> = terms
            .iter()
            .map(|word| {
                let distance = if word.len() <= 3 { 1 } else { 2 };
                let term = tantivy::Term::from_field_text(text_field, word);
                let fq = FuzzyTermQuery::new(term, distance, true);
                (Occur::Should, Box::new(fq) as Box<dyn Query>)
            })
            .collect();

        let query = BooleanQuery::new(sub_queries);
        let top_docs = searcher.search(&query, &TopDocs::with_limit(params.limit * 3))?;

        self.collect_results(&searcher, &top_docs, params)
    }

    fn collect_results(
        &self,
        searcher: &tantivy::Searcher,
        top_docs: &[(f32, tantivy::DocAddress)],
        params: &SearchParams,
    ) -> Result<Vec<SearchResult>> {
        let chunk_id_f = self.schema.get_field("chunk_id").unwrap();
        let session_id_f = self.schema.get_field("session_id").unwrap();
        let source_f = self.schema.get_field("source").unwrap();
        let project_f = self.schema.get_field("project").unwrap();
        let text_f = self.schema.get_field("text").unwrap();
        let timestamp_f = self.schema.get_field("timestamp").unwrap();
        let title_f = self.schema.get_field("title").unwrap();

        let mut results = Vec::new();

        for &(score, doc_address) in top_docs {
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

    pub fn get_session_chunks_for_source(
        &self,
        session_id: &str,
        source: Source,
    ) -> Result<Vec<SearchResult>> {
        let mut chunks = self.get_session_chunks(session_id)?;
        chunks.retain(|chunk| chunk.source == source);
        Ok(chunks)
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

        results.sort_by_key(|r| std::cmp::Reverse(r.timestamp));
        Ok(results)
    }

    /// All indexed sessions, one entry per session, newest first.
    pub fn list_sessions(&self) -> Result<Vec<SearchResult>> {
        let chunks = self.list_all_chunks()?;
        Ok(crate::search::results::dedup_by_session(
            &chunks,
            crate::search::results::SortOrder::TimeDesc,
        ))
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

    fn chunk_count(&self, source: Option<&str>) -> Result<usize> {
        let reader = self.index.reader()?;
        let searcher = reader.searcher();
        if let Some(source) = source {
            let field = self.schema.get_field("source").unwrap();
            let query = tantivy::query::TermQuery::new(
                tantivy::Term::from_field_text(field, source),
                IndexRecordOption::Basic,
            );
            Ok(searcher.search(&query, &Count)?)
        } else {
            Ok(searcher.search(&AllQuery, &Count)?)
        }
    }

    pub fn source_sync_states(&self) -> Result<Vec<SourceSyncState>> {
        self.state.source_sync_states()
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
    pub new_sessions: usize,
    pub updated_sessions: usize,
    pub removed_sessions: usize,
    pub unchanged_sessions: usize,
    pub skipped_sessions: usize,
    pub total_chunks: usize,
}

pub use crate::models::SearchParams;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Message, Role};
    use chrono::Utc;
    use tantivy::collector::TopDocs;
    use tempfile::TempDir;

    struct TestSource {
        name: &'static str,
        sessions: Vec<Session>,
        text: &'static str,
    }

    impl SessionSource for TestSource {
        fn name(&self) -> &'static str {
            self.name
        }

        fn list_sessions(&self) -> Result<Vec<Session>> {
            Ok(self.sessions.clone())
        }

        fn load_messages(&self, _session: &Session) -> Result<Vec<Message>> {
            Ok(vec![Message {
                role: Role::User,
                text: self.text.into(),
                timestamp: None,
                tool_names: vec![],
            }])
        }
    }

    fn test_session(source: Source, id: &str, mtime: i64) -> Session {
        Session {
            source,
            session_id: id.into(),
            project: format!("/project/{}", source.as_str()),
            directory: format!("/project/{}", source.as_str()),
            title: Some(format!("{} title", source.as_str())),
            started_at: Utc::now(),
            model: None,
            file_path: format!("/tmp/{}/{id}", source.as_str()),
            file_mtime: mtime,
            file_size: 10,
        }
    }

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

    #[test]
    fn reconciliation_removes_only_the_selected_source() {
        let temp = TempDir::new().unwrap();
        let engine = IndexEngine::open(temp.path()).unwrap();
        let claude = TestSource {
            name: "claude",
            sessions: vec![test_session(Source::ClaudeCode, "shared", 1)],
            text: "claude conversation",
        };
        let codex = TestSource {
            name: "codex",
            sessions: vec![test_session(Source::Codex, "shared", 1)],
            text: "codex conversation",
        };

        engine.index_source(&claude, false).unwrap();
        engine.index_source(&codex, false).unwrap();
        assert_eq!(
            engine
                .get_session_chunks_for_source("shared", Source::ClaudeCode)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            engine
                .get_session_chunks_for_source("shared", Source::Codex)
                .unwrap()
                .len(),
            1
        );

        let removed = TestSource {
            name: "claude",
            sessions: vec![],
            text: "",
        };
        let stats = engine.index_source(&removed, false).unwrap();
        assert_eq!(stats.removed_sessions, 1);
        assert!(
            engine
                .get_session_chunks_for_source("shared", Source::ClaudeCode)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            engine
                .get_session_chunks_for_source("shared", Source::Codex)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn catalog_mismatch_clears_state_and_stale_documents() {
        let temp = TempDir::new().unwrap();
        {
            let engine = IndexEngine::open(temp.path()).unwrap();
            let source = TestSource {
                name: "claude",
                sessions: vec![test_session(Source::ClaudeCode, "one", 1)],
                text: "indexed conversation",
            };
            engine.index_source(&source, false).unwrap();

            // Simulate an interrupted migration: SQLite claims another session
            // is indexed, but Tantivy has no document for it.
            engine
                .state
                .mark_indexed(&test_session(Source::Codex, "missing", 1))
                .unwrap();
        }

        let recovered = IndexEngine::open(temp.path()).unwrap();
        assert!(recovered.requires_reindex());
        assert_eq!(recovered.session_count(None).unwrap(), 0);
        let source = TestSource {
            name: "claude",
            sessions: vec![test_session(Source::ClaudeCode, "one", 1)],
            text: "indexed conversation",
        };
        recovered.index_source(&source, false).unwrap();
        assert_eq!(recovered.list_sessions().unwrap().len(), 1);
    }

    #[test]
    fn source_failure_preserves_last_success_and_marks_stale() {
        struct FailingSource;
        impl SessionSource for FailingSource {
            fn name(&self) -> &'static str {
                "claude"
            }
            fn list_sessions(&self) -> Result<Vec<Session>> {
                anyhow::bail!("locked")
            }
            fn load_messages(&self, _session: &Session) -> Result<Vec<Message>> {
                unreachable!()
            }
        }

        let temp = TempDir::new().unwrap();
        let engine = IndexEngine::open(temp.path()).unwrap();
        let source = TestSource {
            name: "claude",
            sessions: vec![test_session(Source::ClaudeCode, "one", 1)],
            text: "hello",
        };
        engine.index_source(&source, false).unwrap();
        assert!(engine.index_source(&FailingSource, false).is_err());

        let state = engine
            .source_sync_states()
            .unwrap()
            .into_iter()
            .find(|state| state.source == "claude")
            .unwrap();
        assert!(state.last_success.is_some());
        assert_eq!(state.last_error.as_deref(), Some("locked"));
        assert_eq!(engine.session_count(Some("claude")).unwrap(), 1);
    }
}
