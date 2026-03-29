use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

use crate::models::Session;

pub struct IndexState {
    conn: Connection,
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

    #[allow(dead_code)]
    pub fn clear(&self) -> Result<()> {
        self.conn.execute("DELETE FROM indexed_sessions", [])?;
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
