use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};
use rusqlite::Connection;
use rusqlite::ffi::sqlite3_auto_extension;
use sessfind_common::{DumpChunk, SearchResult, Source};
use std::collections::HashSet;
use std::path::Path;

const EMBEDDING_DIM: usize = 384;

pub struct SemanticStore {
    conn: Connection,
    indexed: HashSet<String>,
}

impl SemanticStore {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Register sqlite-vec as auto extension before opening connection
        #[allow(clippy::missing_transmute_annotations)]
        unsafe {
            sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }

        let conn = Connection::open(path)?;

        // Create metadata table
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS chunk_meta (
                chunk_id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                source TEXT NOT NULL,
                project TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                title TEXT,
                snippet TEXT NOT NULL
            );",
        )?;

        // Create vec0 virtual table for embeddings
        conn.execute_batch(&format!(
            "CREATE VIRTUAL TABLE IF NOT EXISTS vec_chunks USING vec0(
                chunk_id TEXT PRIMARY KEY,
                embedding float[{EMBEDDING_DIM}]
            );"
        ))?;

        // Load existing chunk IDs for fast dedup
        let indexed: HashSet<String> = {
            let mut stmt = conn.prepare("SELECT chunk_id FROM chunk_meta")?;
            stmt.query_map([], |row| row.get::<_, String>(0))?
                .filter_map(|r| r.ok())
                .collect()
        };

        Ok(Self { conn, indexed })
    }

    pub fn is_chunk_indexed(&self, chunk_id: &str) -> bool {
        self.indexed.contains(chunk_id)
    }

    pub fn insert(&mut self, chunk: &DumpChunk, embedding: &[f32]) -> Result<()> {
        // Build snippet: first 2 non-empty lines
        let snippet: String = chunk
            .text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .take(2)
            .collect::<Vec<_>>()
            .join(" | ");

        let ts = chunk.timestamp.timestamp();

        self.conn.execute(
            "INSERT OR REPLACE INTO chunk_meta (chunk_id, session_id, source, project, timestamp, title, snippet)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                chunk.chunk_id,
                chunk.session_id,
                chunk.source.as_str(),
                chunk.project,
                ts,
                chunk.title,
                snippet,
            ],
        )?;

        // Convert embedding to bytes for sqlite-vec
        let embedding_bytes: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();

        // vec0 virtual tables don't support INSERT OR REPLACE, so delete first if exists
        self.conn.execute(
            "DELETE FROM vec_chunks WHERE chunk_id = ?1",
            [&chunk.chunk_id],
        )?;

        self.conn.execute(
            "INSERT INTO vec_chunks (chunk_id, embedding) VALUES (?1, ?2)",
            rusqlite::params![chunk.chunk_id, embedding_bytes],
        )?;

        self.indexed.insert(chunk.chunk_id.clone());
        Ok(())
    }

    #[allow(dead_code)]
    pub fn delete_session(&self, session_id: &str) -> Result<()> {
        // Get chunk_ids for session
        let mut stmt = self
            .conn
            .prepare("SELECT chunk_id FROM chunk_meta WHERE session_id = ?1")?;
        let chunk_ids: Vec<String> = stmt
            .query_map([session_id], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect();

        for cid in &chunk_ids {
            self.conn
                .execute("DELETE FROM vec_chunks WHERE chunk_id = ?1", [cid])?;
        }
        self.conn
            .execute("DELETE FROM chunk_meta WHERE session_id = ?1", [session_id])?;
        Ok(())
    }

    pub fn search(
        &self,
        query_embedding: &[f32],
        limit: usize,
        source_filter: Option<&str>,
        project_filter: Option<&str>,
        after: Option<DateTime<Utc>>,
        before: Option<DateTime<Utc>>,
    ) -> Result<Vec<SearchResult>> {
        let embedding_bytes: Vec<u8> = query_embedding
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();

        // Fetch more than limit to allow for post-filtering
        let fetch_limit = limit * 5;

        let mut stmt = self.conn.prepare(
            "SELECT
                v.chunk_id,
                v.distance,
                m.session_id,
                m.source,
                m.project,
                m.timestamp,
                m.title,
                m.snippet
            FROM vec_chunks v
            JOIN chunk_meta m ON v.chunk_id = m.chunk_id
            WHERE v.embedding MATCH ?1
            AND k = ?2
            ORDER BY v.distance",
        )?;

        let rows = stmt.query_map(rusqlite::params![embedding_bytes, fetch_limit], |row| {
            Ok(RawRow {
                chunk_id: row.get(0)?,
                distance: row.get(1)?,
                session_id: row.get(2)?,
                source: row.get(3)?,
                project: row.get(4)?,
                timestamp: row.get(5)?,
                title: row.get(6)?,
                snippet: row.get(7)?,
            })
        })?;

        let mut results = Vec::new();
        let mut seen_sessions = HashSet::new();

        for row in rows {
            let row = row?;

            // Apply filters
            if let Some(sf) = source_filter {
                if row.source != sf {
                    continue;
                }
            }
            if let Some(pf) = project_filter {
                if !row.project.to_lowercase().contains(&pf.to_lowercase()) {
                    continue;
                }
            }
            if let Some(after_dt) = after {
                let ts = Utc.timestamp_opt(row.timestamp, 0).unwrap();
                if ts < after_dt {
                    continue;
                }
            }
            if let Some(before_dt) = before {
                let ts = Utc.timestamp_opt(row.timestamp, 0).unwrap();
                if ts > before_dt {
                    continue;
                }
            }

            // Dedup by session
            if !seen_sessions.insert(row.session_id.clone()) {
                continue;
            }

            let timestamp = Utc.timestamp_opt(row.timestamp, 0).unwrap();
            // Convert distance to similarity score (1 - distance for cosine)
            let score = 1.0 - row.distance;

            results.push(SearchResult {
                chunk_id: row.chunk_id,
                session_id: row.session_id,
                source: Source::parse_source(&row.source).unwrap_or(Source::ClaudeCode),
                project: row.project,
                timestamp,
                title: row.title,
                snippet: row.snippet,
                score,
            });

            if results.len() >= limit {
                break;
            }
        }

        Ok(results)
    }

    pub fn count(&self) -> Result<usize> {
        let count: usize = self
            .conn
            .query_row("SELECT COUNT(*) FROM chunk_meta", [], |row| row.get(0))?;
        Ok(count)
    }
}

struct RawRow {
    chunk_id: String,
    distance: f32,
    session_id: String,
    source: String,
    project: String,
    timestamp: i64,
    title: Option<String>,
    snippet: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use tempfile::TempDir;

    fn make_chunk(id: &str, session: &str, source: Source, text: &str) -> DumpChunk {
        DumpChunk {
            chunk_id: id.into(),
            session_id: session.into(),
            source,
            project: "/test/project".into(),
            timestamp: Utc::now(),
            title: Some("Test session".into()),
            text: text.into(),
        }
    }

    fn random_embedding() -> Vec<f32> {
        (0..EMBEDDING_DIM)
            .map(|i| (i as f32 * 0.01).sin())
            .collect()
    }

    #[test]
    fn open_creates_db() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = SemanticStore::open(&db_path).unwrap();
        assert_eq!(store.count().unwrap(), 0);
        assert!(db_path.exists());
    }

    #[test]
    fn insert_and_count() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let mut store = SemanticStore::open(&db_path).unwrap();

        let chunk = make_chunk("c:s1:0", "s1", Source::ClaudeCode, "Hello world");
        let emb = random_embedding();
        store.insert(&chunk, &emb).unwrap();

        assert_eq!(store.count().unwrap(), 1);
        assert!(store.is_chunk_indexed("c:s1:0"));
        assert!(!store.is_chunk_indexed("nonexistent"));
    }

    #[test]
    fn insert_multiple_and_search() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let mut store = SemanticStore::open(&db_path).unwrap();

        // Insert 3 chunks from 2 sessions
        let emb1: Vec<f32> = (0..EMBEDDING_DIM).map(|i| (i as f32 * 0.1).sin()).collect();
        let emb2: Vec<f32> = (0..EMBEDDING_DIM).map(|i| (i as f32 * 0.2).cos()).collect();
        let emb3: Vec<f32> = (0..EMBEDDING_DIM)
            .map(|i| (i as f32 * 0.1).sin() + 0.01)
            .collect(); // similar to emb1

        store
            .insert(
                &make_chunk("c:s1:0", "s1", Source::ClaudeCode, "Rust programming"),
                &emb1,
            )
            .unwrap();
        store
            .insert(
                &make_chunk("c:s1:1", "s1", Source::ClaudeCode, "More Rust"),
                &emb2,
            )
            .unwrap();
        store
            .insert(
                &make_chunk("o:s2:0", "s2", Source::OpenCode, "Python scripting"),
                &emb3,
            )
            .unwrap();

        assert_eq!(store.count().unwrap(), 3);

        // Search with emb similar to emb1 — should return s1 first (closer), then s2
        let results = store.search(&emb1, 10, None, None, None, None).unwrap();
        assert!(!results.is_empty());
        // Results should be deduped by session
        let session_ids: Vec<&str> = results.iter().map(|r| r.session_id.as_str()).collect();
        assert!(session_ids.contains(&"s1"));
    }

    #[test]
    fn search_with_source_filter() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let mut store = SemanticStore::open(&db_path).unwrap();

        let emb = random_embedding();
        store
            .insert(
                &make_chunk("c:s1:0", "s1", Source::ClaudeCode, "hello"),
                &emb,
            )
            .unwrap();
        store
            .insert(&make_chunk("o:s2:0", "s2", Source::OpenCode, "world"), &emb)
            .unwrap();

        let results = store
            .search(&emb, 10, Some("claude"), None, None, None)
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, Source::ClaudeCode);
    }

    #[test]
    fn search_with_project_filter() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let mut store = SemanticStore::open(&db_path).unwrap();

        let emb = random_embedding();
        let mut chunk1 = make_chunk("c:s1:0", "s1", Source::ClaudeCode, "hello");
        chunk1.project = "/home/user/myproject".into();
        let mut chunk2 = make_chunk("c:s2:0", "s2", Source::ClaudeCode, "world");
        chunk2.project = "/home/user/other".into();

        store.insert(&chunk1, &emb).unwrap();
        store.insert(&chunk2, &emb).unwrap();

        let results = store
            .search(&emb, 10, None, Some("myproject"), None, None)
            .unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].project.contains("myproject"));
    }

    #[test]
    fn delete_session() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let mut store = SemanticStore::open(&db_path).unwrap();

        let emb = random_embedding();
        store
            .insert(
                &make_chunk("c:s1:0", "s1", Source::ClaudeCode, "hello"),
                &emb,
            )
            .unwrap();
        store
            .insert(
                &make_chunk("c:s1:1", "s1", Source::ClaudeCode, "world"),
                &emb,
            )
            .unwrap();
        store
            .insert(&make_chunk("o:s2:0", "s2", Source::OpenCode, "other"), &emb)
            .unwrap();

        assert_eq!(store.count().unwrap(), 3);
        store.delete_session("s1").unwrap();
        assert_eq!(store.count().unwrap(), 1);
    }

    #[test]
    fn insert_replace_existing() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let mut store = SemanticStore::open(&db_path).unwrap();

        let emb1 = random_embedding();
        let emb2: Vec<f32> = (0..EMBEDDING_DIM).map(|_| 0.5).collect();

        store
            .insert(&make_chunk("c:s1:0", "s1", Source::ClaudeCode, "v1"), &emb1)
            .unwrap();
        assert_eq!(store.count().unwrap(), 1);

        // Re-insert same chunk_id — should replace
        store
            .insert(&make_chunk("c:s1:0", "s1", Source::ClaudeCode, "v2"), &emb2)
            .unwrap();
        assert_eq!(store.count().unwrap(), 1);
    }

    #[test]
    fn search_empty_store() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = SemanticStore::open(&db_path).unwrap();

        let emb = random_embedding();
        let results = store.search(&emb, 10, None, None, None, None).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn reopen_persists_data() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");

        {
            let mut store = SemanticStore::open(&db_path).unwrap();
            let emb = random_embedding();
            store
                .insert(
                    &make_chunk("c:s1:0", "s1", Source::ClaudeCode, "persisted"),
                    &emb,
                )
                .unwrap();
            assert_eq!(store.count().unwrap(), 1);
        }

        // Reopen
        let store = SemanticStore::open(&db_path).unwrap();
        assert_eq!(store.count().unwrap(), 1);
        assert!(store.is_chunk_indexed("c:s1:0"));
    }
}
