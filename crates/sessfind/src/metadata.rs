//! User-controlled metadata: tags on sessions and user-defined projects.
//! Stored in `data_dir()/metadata.db`, separate from the tantivy index and the
//! index-state DB so it survives re-indexing.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension};
use sessfind_common::{TagCount, UserProject};

pub struct MetadataStore {
    conn: Connection,
}

impl MetadataStore {
    pub fn open(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS tags (
                session_id TEXT NOT NULL,
                tag        TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (session_id, tag)
            );
            CREATE INDEX IF NOT EXISTS idx_tags_tag ON tags(tag);

            CREATE TABLE IF NOT EXISTS user_projects (
                id          INTEGER PRIMARY KEY,
                name        TEXT NOT NULL UNIQUE,
                root_dir    TEXT NOT NULL,
                description TEXT,
                created_at  TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS user_project_dirs (
                project_id INTEGER NOT NULL REFERENCES user_projects(id) ON DELETE CASCADE,
                dir        TEXT NOT NULL,
                PRIMARY KEY (project_id, dir)
            );
            CREATE TABLE IF NOT EXISTS user_project_sessions (
                project_id INTEGER NOT NULL REFERENCES user_projects(id) ON DELETE CASCADE,
                session_id TEXT NOT NULL,
                PRIMARY KEY (project_id, session_id)
            );",
        )?;
        Ok(Self { conn })
    }

    // ── Tags ──

    pub fn add_tag(&self, session_id: &str, tag: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO tags (session_id, tag) VALUES (?1, ?2)",
            (session_id, tag),
        )?;
        Ok(())
    }

    pub fn remove_tag(&self, session_id: &str, tag: &str) -> Result<bool> {
        let n = self.conn.execute(
            "DELETE FROM tags WHERE session_id = ?1 AND tag = ?2",
            (session_id, tag),
        )?;
        Ok(n > 0)
    }

    // Single-session lookup, used by the extension's session preview (PR4).
    #[allow(dead_code)]
    pub fn tags_for_session(&self, session_id: &str) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT tag FROM tags WHERE session_id = ?1 ORDER BY tag")?;
        let tags = stmt
            .query_map([session_id], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(tags)
    }

    /// Tags for many sessions at once, keyed by session_id (batch decoration).
    pub fn tags_for_sessions(
        &self,
        session_ids: &[String],
    ) -> Result<HashMap<String, Vec<String>>> {
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        if session_ids.is_empty() {
            return Ok(map);
        }
        let placeholders = std::iter::repeat_n("?", session_ids.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT session_id, tag FROM tags WHERE session_id IN ({placeholders}) ORDER BY tag"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let params = rusqlite::params_from_iter(session_ids.iter());
        let rows = stmt.query_map(params, |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (sid, tag) = row?;
            map.entry(sid).or_default().push(tag);
        }
        Ok(map)
    }

    pub fn list_tags(&self) -> Result<Vec<TagCount>> {
        let mut stmt = self
            .conn
            .prepare("SELECT tag, COUNT(*) FROM tags GROUP BY tag ORDER BY COUNT(*) DESC, tag")?;
        let tags = stmt
            .query_map([], |row| {
                Ok(TagCount {
                    tag: row.get(0)?,
                    session_count: row.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(tags)
    }

    /// Session IDs carrying the given tag.
    pub fn sessions_with_tag(&self, tag: &str) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT session_id FROM tags WHERE tag = ?1")?;
        let ids = stmt
            .query_map([tag], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(ids)
    }

    // ── User projects ──

    pub fn create_project(&self, name: &str, root_dir: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO user_projects (name, root_dir) VALUES (?1, ?2)",
            (name, root_dir),
        )?;
        Ok(())
    }

    pub fn delete_project(&self, name: &str) -> Result<bool> {
        let n = self
            .conn
            .execute("DELETE FROM user_projects WHERE name = ?1", [name])?;
        Ok(n > 0)
    }

    fn project_id(&self, name: &str) -> Result<Option<i64>> {
        let id = self
            .conn
            .query_row(
                "SELECT id FROM user_projects WHERE name = ?1",
                [name],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        Ok(id)
    }

    pub fn add_dir(&self, name: &str, dir: &str) -> Result<()> {
        let id = self
            .project_id(name)?
            .ok_or_else(|| anyhow::anyhow!("No user project named '{name}'"))?;
        self.conn.execute(
            "INSERT OR IGNORE INTO user_project_dirs (project_id, dir) VALUES (?1, ?2)",
            (id, dir),
        )?;
        Ok(())
    }

    pub fn remove_dir(&self, name: &str, dir: &str) -> Result<bool> {
        let id = self
            .project_id(name)?
            .ok_or_else(|| anyhow::anyhow!("No user project named '{name}'"))?;
        let n = self.conn.execute(
            "DELETE FROM user_project_dirs WHERE project_id = ?1 AND dir = ?2",
            (id, dir),
        )?;
        Ok(n > 0)
    }

    pub fn pin_session(&self, name: &str, session_id: &str) -> Result<()> {
        let id = self
            .project_id(name)?
            .ok_or_else(|| anyhow::anyhow!("No user project named '{name}'"))?;
        self.conn.execute(
            "INSERT OR IGNORE INTO user_project_sessions (project_id, session_id) VALUES (?1, ?2)",
            (id, session_id),
        )?;
        Ok(())
    }

    pub fn unpin_session(&self, name: &str, session_id: &str) -> Result<bool> {
        let id = self
            .project_id(name)?
            .ok_or_else(|| anyhow::anyhow!("No user project named '{name}'"))?;
        let n = self.conn.execute(
            "DELETE FROM user_project_sessions WHERE project_id = ?1 AND session_id = ?2",
            (id, session_id),
        )?;
        Ok(n > 0)
    }

    pub fn get_project(&self, name: &str) -> Result<Option<UserProject>> {
        let row = self
            .conn
            .query_row(
                "SELECT id, name, root_dir, description, created_at FROM user_projects WHERE name = ?1",
                [name],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
            .optional()?;

        let Some((id, name, root_dir, description, created_at)) = row else {
            return Ok(None);
        };

        Ok(Some(UserProject {
            name,
            root_dir,
            dirs: self.project_dirs(id)?,
            pinned_sessions: self.project_pinned(id)?,
            description,
            created_at: parse_sqlite_datetime(&created_at),
        }))
    }

    pub fn list_projects(&self) -> Result<Vec<UserProject>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, root_dir, description, created_at FROM user_projects ORDER BY name",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut projects = Vec::with_capacity(rows.len());
        for (id, name, root_dir, description, created_at) in rows {
            projects.push(UserProject {
                name,
                root_dir,
                dirs: self.project_dirs(id)?,
                pinned_sessions: self.project_pinned(id)?,
                description,
                created_at: parse_sqlite_datetime(&created_at),
            });
        }
        Ok(projects)
    }

    fn project_dirs(&self, project_id: i64) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT dir FROM user_project_dirs WHERE project_id = ?1 ORDER BY dir")?;
        let dirs = stmt
            .query_map([project_id], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(dirs)
    }

    fn project_pinned(&self, project_id: i64) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT session_id FROM user_project_sessions WHERE project_id = ?1 ORDER BY session_id",
        )?;
        let ids = stmt
            .query_map([project_id], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(ids)
    }
}

fn parse_sqlite_datetime(s: &str) -> DateTime<Utc> {
    // SQLite `datetime('now')` yields "YYYY-MM-DD HH:MM:SS" in UTC.
    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
        .map(|dt| dt.and_utc())
        .unwrap_or_else(|_| Utc::now())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn store() -> (TempDir, MetadataStore) {
        let dir = TempDir::new().unwrap();
        let store = MetadataStore::open(&dir.path().join("metadata.db")).unwrap();
        (dir, store)
    }

    #[test]
    fn tag_add_list_remove() {
        let (_d, s) = store();
        s.add_tag("sess1", "work").unwrap();
        s.add_tag("sess1", "rust").unwrap();
        s.add_tag("sess2", "work").unwrap();
        // Idempotent
        s.add_tag("sess1", "work").unwrap();

        assert_eq!(s.tags_for_session("sess1").unwrap(), vec!["rust", "work"]);

        let counts = s.list_tags().unwrap();
        let work = counts.iter().find(|c| c.tag == "work").unwrap();
        assert_eq!(work.session_count, 2);

        assert!(s.remove_tag("sess1", "work").unwrap());
        assert!(!s.remove_tag("sess1", "work").unwrap());
        assert_eq!(s.tags_for_session("sess1").unwrap(), vec!["rust"]);
    }

    #[test]
    fn tags_for_sessions_batch() {
        let (_d, s) = store();
        s.add_tag("a", "x").unwrap();
        s.add_tag("a", "y").unwrap();
        s.add_tag("b", "z").unwrap();

        let map = s
            .tags_for_sessions(&["a".into(), "b".into(), "c".into()])
            .unwrap();
        assert_eq!(
            map.get("a").unwrap(),
            &vec!["x".to_string(), "y".to_string()]
        );
        assert_eq!(map.get("b").unwrap(), &vec!["z".to_string()]);
        assert!(!map.contains_key("c"));
    }

    #[test]
    fn sessions_with_tag_lookup() {
        let (_d, s) = store();
        s.add_tag("a", "work").unwrap();
        s.add_tag("b", "work").unwrap();
        s.add_tag("c", "play").unwrap();
        let mut ids = s.sessions_with_tag("work").unwrap();
        ids.sort();
        assert_eq!(ids, vec!["a", "b"]);
    }

    #[test]
    fn project_crud() {
        let (_d, s) = store();
        s.create_project("myproj", "/home/me/proj").unwrap();
        s.add_dir("myproj", "/home/me/extra").unwrap();
        s.add_dir("myproj", "/home/me/extra").unwrap(); // idempotent
        s.pin_session("myproj", "sess-xyz").unwrap();

        let p = s.get_project("myproj").unwrap().unwrap();
        assert_eq!(p.name, "myproj");
        assert_eq!(p.root_dir, "/home/me/proj");
        assert_eq!(p.dirs, vec!["/home/me/extra"]);
        assert_eq!(p.pinned_sessions, vec!["sess-xyz"]);
        assert!(p.description.is_none());

        assert!(s.remove_dir("myproj", "/home/me/extra").unwrap());
        assert!(s.unpin_session("myproj", "sess-xyz").unwrap());
        let p = s.get_project("myproj").unwrap().unwrap();
        assert!(p.dirs.is_empty());
        assert!(p.pinned_sessions.is_empty());
    }

    #[test]
    fn project_delete_cascades() {
        let (_d, s) = store();
        s.create_project("p", "/root").unwrap();
        s.add_dir("p", "/d").unwrap();
        s.pin_session("p", "sess").unwrap();
        assert!(s.delete_project("p").unwrap());
        assert!(s.get_project("p").unwrap().is_none());
        assert!(!s.delete_project("p").unwrap());
        // Recreate with the same name works — cascade cleared child rows.
        s.create_project("p", "/root2").unwrap();
        let p = s.get_project("p").unwrap().unwrap();
        assert!(p.dirs.is_empty());
        assert!(p.pinned_sessions.is_empty());
    }

    #[test]
    fn create_duplicate_project_fails() {
        let (_d, s) = store();
        s.create_project("dup", "/a").unwrap();
        assert!(s.create_project("dup", "/b").is_err());
    }

    #[test]
    fn add_dir_to_missing_project_fails() {
        let (_d, s) = store();
        assert!(s.add_dir("nope", "/d").is_err());
    }

    #[test]
    fn reopen_persists() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("metadata.db");
        {
            let s = MetadataStore::open(&path).unwrap();
            s.add_tag("sess", "keep").unwrap();
            s.create_project("proj", "/root").unwrap();
        }
        let s = MetadataStore::open(&path).unwrap();
        assert_eq!(s.tags_for_session("sess").unwrap(), vec!["keep"]);
        assert!(s.get_project("proj").unwrap().is_some());
    }
}
