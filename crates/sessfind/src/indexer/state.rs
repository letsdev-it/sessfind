use anyhow::Result;
use chrono::{DateTime, NaiveDateTime, Utc};
use rusqlite::Connection;
use std::collections::HashSet;
use std::path::Path;

use crate::models::Session;

pub struct IndexState {
    conn: Connection,
}

#[derive(Debug, Clone)]
pub struct SourceSyncState {
    pub source: String,
    pub last_success: Option<DateTime<Utc>>,
    pub last_attempt: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

impl IndexState {
    pub fn open(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS indexed_sessions (
                source TEXT NOT NULL,
                session_id TEXT NOT NULL,
                file_path TEXT NOT NULL,
                file_mtime INTEGER NOT NULL,
                file_size INTEGER NOT NULL,
                indexed_at TEXT NOT NULL,
                PRIMARY KEY (source, session_id)
            );
            CREATE TABLE IF NOT EXISTS source_sync (
                source TEXT PRIMARY KEY,
                last_success TEXT,
                last_attempt TEXT NOT NULL,
                last_error TEXT
            );",
        )?;
        Ok(Self { conn })
    }

    pub fn is_current(&self, session: &Session) -> bool {
        let result: Result<(i64, u64), _> = self.conn.query_row(
            "SELECT file_mtime, file_size FROM indexed_sessions WHERE source = ?1 AND session_id = ?2",
            (&session.source.as_str(), &session.session_id),
            |row| Ok((row.get(0)?, row.get(1)?)),
        );
        match result {
            Ok((mtime, size)) => mtime == session.file_mtime && size == session.file_size,
            Err(_) => false,
        }
    }

    pub fn session_ids(&self, source: &str) -> Result<HashSet<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT session_id FROM indexed_sessions WHERE source = ?1")?;
        let rows = stmt.query_map([source], |row| row.get::<_, String>(0))?;
        Ok(rows.collect::<rusqlite::Result<HashSet<_>>>()?)
    }

    pub fn session_keys(&self) -> Result<HashSet<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT source, session_id FROM indexed_sessions")?;
        let rows = stmt.query_map([], |row| {
            let source: String = row.get(0)?;
            let session_id: String = row.get(1)?;
            Ok(format!("{source}:{session_id}"))
        })?;
        Ok(rows.collect::<rusqlite::Result<HashSet<_>>>()?)
    }

    pub fn mark_indexed(&self, session: &Session) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO indexed_sessions (source, session_id, file_path, file_mtime, file_size, indexed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))",
            (
                session.source.as_str(),
                &session.session_id,
                &session.file_path,
                session.file_mtime,
                session.file_size,
            ),
        )?;
        Ok(())
    }

    pub fn remove_sessions(&self, source: &str, session_ids: &[String]) -> Result<()> {
        for session_id in session_ids {
            self.conn.execute(
                "DELETE FROM indexed_sessions WHERE source = ?1 AND session_id = ?2",
                (source, session_id),
            )?;
        }
        Ok(())
    }

    pub fn mark_source_success(&self, source: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO source_sync (source, last_success, last_attempt, last_error)
             VALUES (?1, datetime('now'), datetime('now'), NULL)
             ON CONFLICT(source) DO UPDATE SET
                last_success = excluded.last_success,
                last_attempt = excluded.last_attempt,
                last_error = NULL",
            [source],
        )?;
        Ok(())
    }

    pub fn mark_source_failure(&self, source: &str, error: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO source_sync (source, last_success, last_attempt, last_error)
             VALUES (?1, NULL, datetime('now'), ?2)
             ON CONFLICT(source) DO UPDATE SET
                last_attempt = excluded.last_attempt,
                last_error = excluded.last_error",
            (source, error),
        )?;
        Ok(())
    }

    pub fn source_sync_states(&self) -> Result<Vec<SourceSyncState>> {
        let mut stmt = self.conn.prepare(
            "SELECT source, last_success, last_attempt, last_error
             FROM source_sync ORDER BY source",
        )?;
        let rows = stmt.query_map([], |row| {
            let parse = |value: Option<String>| {
                value.and_then(|value| {
                    NaiveDateTime::parse_from_str(&value, "%Y-%m-%d %H:%M:%S")
                        .ok()
                        .map(|value| value.and_utc())
                })
            };
            Ok(SourceSyncState {
                source: row.get(0)?,
                last_success: parse(row.get(1)?),
                last_attempt: parse(row.get(2)?),
                last_error: row.get(3)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    #[allow(dead_code)]
    pub fn clear(&self) -> Result<()> {
        self.conn.execute_batch(
            "DELETE FROM indexed_sessions;
             DELETE FROM source_sync;",
        )?;
        Ok(())
    }

    pub fn count(&self, source: Option<&str>) -> Result<usize> {
        let count: usize = if let Some(s) = source {
            self.conn.query_row(
                "SELECT COUNT(*) FROM indexed_sessions WHERE source = ?1",
                [s],
                |row| row.get(0),
            )?
        } else {
            self.conn
                .query_row("SELECT COUNT(*) FROM indexed_sessions", [], |row| {
                    row.get(0)
                })?
        };
        Ok(count)
    }
}
