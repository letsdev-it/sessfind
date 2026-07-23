//! User-controlled metadata: names and tags on sessions and project directories.
//! Stored in `data_dir()/metadata.db`, separate from the tantivy index and the
//! index-state DB so it survives re-indexing.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use rusqlite::Connection;

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

            CREATE TABLE IF NOT EXISTS session_names (
                session_id TEXT PRIMARY KEY,
                name       TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS project_tags (
                project_dir TEXT NOT NULL,
                tag         TEXT NOT NULL,
                created_at  TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (project_dir, tag)
            );
            CREATE INDEX IF NOT EXISTS idx_project_tags_tag ON project_tags(tag);
            CREATE TABLE IF NOT EXISTS project_descriptions (
                project_dir  TEXT PRIMARY KEY,
                description  TEXT NOT NULL,
                tool         TEXT NOT NULL,
                generated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )?;
        Ok(Self { conn })
    }

    // ── Tags ──

    /// Materialize pre-source-qualified metadata under every matching source
    /// key before a mutation. Legacy native IDs are ambiguous when two tools
    /// reuse an ID, so each matching session inherits the old value once.
    pub fn migrate_legacy_session(
        &self,
        legacy_session_id: &str,
        session_keys: &[String],
    ) -> Result<()> {
        if session_keys.is_empty() {
            return Ok(());
        }
        let tx = self.conn.unchecked_transaction()?;
        for key in session_keys {
            tx.execute(
                "INSERT OR IGNORE INTO tags (session_id, tag)
                 SELECT ?1, tag FROM tags WHERE session_id = ?2",
                (key, legacy_session_id),
            )?;
            tx.execute(
                "INSERT OR IGNORE INTO session_names (session_id, name)
                 SELECT ?1, name FROM session_names WHERE session_id = ?2",
                (key, legacy_session_id),
            )?;
        }
        tx.execute(
            "DELETE FROM tags WHERE session_id = ?1",
            [legacy_session_id],
        )?;
        tx.execute(
            "DELETE FROM session_names WHERE session_id = ?1",
            [legacy_session_id],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn add_tag(&self, session_key: &str, tag: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO tags (session_id, tag) VALUES (?1, ?2)",
            (session_key, tag),
        )?;
        Ok(())
    }

    pub fn remove_tag(
        &self,
        session_key: &str,
        legacy_session_id: &str,
        tag: &str,
    ) -> Result<bool> {
        let n = self.conn.execute(
            "DELETE FROM tags
             WHERE session_id IN (?1, ?2) AND tag = ?3",
            (session_key, legacy_session_id, tag),
        )?;
        Ok(n > 0)
    }

    pub fn tags_for_session(
        &self,
        session_key: &str,
        legacy_session_id: &str,
    ) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT tag FROM tags
                 WHERE session_id IN (?1, ?2) ORDER BY tag",
        )?;
        let tags = stmt
            .query_map((session_key, legacy_session_id), |row| {
                row.get::<_, String>(0)
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(tags)
    }

    /// Tags for many sessions at once, keyed by source-qualified session key.
    /// Legacy native-id rows created by the pre-qualified schema are applied as
    /// a fallback so existing user metadata is preserved.
    pub fn tags_for_sessions(
        &self,
        sessions: &[(String, String)],
    ) -> Result<HashMap<String, Vec<String>>> {
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        if sessions.is_empty() {
            return Ok(map);
        }
        let lookup_ids: Vec<&str> = sessions
            .iter()
            .flat_map(|(key, id)| [key.as_str(), id.as_str()])
            .collect();
        let placeholders = std::iter::repeat_n("?", lookup_ids.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT session_id, tag FROM tags WHERE session_id IN ({placeholders}) ORDER BY tag"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let params = rusqlite::params_from_iter(lookup_ids);
        let rows = stmt.query_map(params, |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let keys_by_native: HashMap<&str, Vec<&str>> =
            sessions.iter().fold(HashMap::new(), |mut acc, (key, id)| {
                acc.entry(id.as_str()).or_default().push(key.as_str());
                acc
            });
        for row in rows {
            let (stored_id, tag) = row?;
            if sessions.iter().any(|(key, _)| key == &stored_id) {
                let tags = map.entry(stored_id).or_default();
                if !tags.contains(&tag) {
                    tags.push(tag);
                }
            } else if let Some(keys) = keys_by_native.get(stored_id.as_str()) {
                for key in keys {
                    let tags = map.entry((*key).to_string()).or_default();
                    if !tags.contains(&tag) {
                        tags.push(tag.clone());
                    }
                }
            }
        }
        Ok(map)
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

    // ── Session names (user rename overrides) ──

    pub fn set_session_name(&self, session_key: &str, name: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO session_names (session_id, name) VALUES (?1, ?2)",
            (session_key, name),
        )?;
        Ok(())
    }

    pub fn clear_session_name(&self, session_key: &str, legacy_session_id: &str) -> Result<bool> {
        let n = self.conn.execute(
            "DELETE FROM session_names WHERE session_id IN (?1, ?2)",
            (session_key, legacy_session_id),
        )?;
        Ok(n > 0)
    }

    /// Custom names for many sessions at once, keyed by source-qualified key.
    pub fn names_for_sessions(
        &self,
        sessions: &[(String, String)],
    ) -> Result<HashMap<String, String>> {
        let mut map = HashMap::new();
        if sessions.is_empty() {
            return Ok(map);
        }
        let lookup_ids: Vec<&str> = sessions
            .iter()
            .flat_map(|(key, id)| [key.as_str(), id.as_str()])
            .collect();
        let placeholders = std::iter::repeat_n("?", lookup_ids.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT session_id, name FROM session_names WHERE session_id IN ({placeholders})"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(lookup_ids), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let keys_by_native: HashMap<&str, Vec<&str>> =
            sessions.iter().fold(HashMap::new(), |mut acc, (key, id)| {
                acc.entry(id.as_str()).or_default().push(key.as_str());
                acc
            });
        let mut legacy = Vec::new();
        for row in rows {
            let (stored_id, name) = row?;
            if sessions.iter().any(|(key, _)| key == &stored_id) {
                map.insert(stored_id, name);
            } else {
                legacy.push((stored_id, name));
            }
        }
        for (native_id, name) in legacy {
            if let Some(keys) = keys_by_native.get(native_id.as_str()) {
                for key in keys {
                    map.entry((*key).to_string())
                        .or_insert_with(|| name.clone());
                }
            }
        }
        Ok(map)
    }

    // ── Project (directory) descriptions (LLM summaries) ──

    pub fn set_project_description(
        &self,
        project_dir: &str,
        description: &str,
        tool: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO project_descriptions (project_dir, description, tool)
             VALUES (?1, ?2, ?3)",
            (project_dir, description, tool),
        )?;
        Ok(())
    }

    /// All stored project descriptions, keyed by directory.
    pub fn project_descriptions_map(&self) -> Result<HashMap<String, String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT project_dir, description FROM project_descriptions")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut map = HashMap::new();
        for row in rows {
            let (dir, desc) = row?;
            map.insert(dir, desc);
        }
        Ok(map)
    }

    // ── Project (directory) tags ──

    pub fn add_project_tag(&self, project_dir: &str, tag: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO project_tags (project_dir, tag) VALUES (?1, ?2)",
            (project_dir, tag),
        )?;
        Ok(())
    }

    pub fn remove_project_tag(&self, project_dir: &str, tag: &str) -> Result<bool> {
        let n = self.conn.execute(
            "DELETE FROM project_tags WHERE project_dir = ?1 AND tag = ?2",
            (project_dir, tag),
        )?;
        Ok(n > 0)
    }

    /// All project-dir tags, keyed by directory.
    pub fn project_tags_map(&self) -> Result<HashMap<String, Vec<String>>> {
        let mut stmt = self
            .conn
            .prepare("SELECT project_dir, tag FROM project_tags ORDER BY tag")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        for row in rows {
            let (dir, tag) = row?;
            map.entry(dir).or_default().push(tag);
        }
        Ok(map)
    }
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

        assert_eq!(
            s.tags_for_session("sess1", "sess1").unwrap(),
            vec!["rust", "work"]
        );

        assert!(s.remove_tag("sess1", "sess1", "work").unwrap());
        assert!(!s.remove_tag("sess1", "sess1", "work").unwrap());
        assert_eq!(s.tags_for_session("sess1", "sess1").unwrap(), vec!["rust"]);
    }

    #[test]
    fn tags_for_sessions_batch() {
        let (_d, s) = store();
        s.add_tag("a", "x").unwrap();
        s.add_tag("a", "y").unwrap();
        s.add_tag("b", "z").unwrap();

        let map = s
            .tags_for_sessions(&[
                ("claude:a".into(), "a".into()),
                ("claude:b".into(), "b".into()),
                ("claude:c".into(), "c".into()),
            ])
            .unwrap();
        assert_eq!(
            map.get("claude:a").unwrap(),
            &vec!["x".to_string(), "y".to_string()]
        );
        assert_eq!(map.get("claude:b").unwrap(), &vec!["z".to_string()]);
        assert!(!map.contains_key("claude:c"));
    }

    #[test]
    fn legacy_metadata_is_copied_to_each_source_before_mutation() {
        let (_d, s) = store();
        s.add_tag("same", "legacy").unwrap();
        s.set_session_name("same", "Old name").unwrap();
        s.migrate_legacy_session("same", &["claude:same".into(), "codex:same".into()])
            .unwrap();

        for key in ["claude:same", "codex:same"] {
            assert_eq!(s.tags_for_session(key, "same").unwrap(), vec!["legacy"]);
            assert_eq!(
                s.names_for_sessions(&[(key.into(), "same".into())])
                    .unwrap()
                    .get(key)
                    .map(String::as_str),
                Some("Old name")
            );
        }
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
    fn session_name_set_clear() {
        let (_d, s) = store();
        s.set_session_name("s1", "My great session").unwrap();
        s.set_session_name("s1", "Renamed").unwrap(); // overwrite
        s.set_session_name("s2", "Other").unwrap();

        let names = s
            .names_for_sessions(&[
                ("claude:s1".into(), "s1".into()),
                ("claude:s2".into(), "s2".into()),
                ("claude:s3".into(), "s3".into()),
            ])
            .unwrap();
        assert_eq!(names.get("claude:s1").map(String::as_str), Some("Renamed"));
        assert_eq!(names.get("claude:s2").map(String::as_str), Some("Other"));
        assert!(!names.contains_key("claude:s3"));

        assert!(s.clear_session_name("claude:s1", "s1").unwrap());
        assert!(!s.clear_session_name("claude:s1", "s1").unwrap());
        let names = s
            .names_for_sessions(&[("claude:s1".into(), "s1".into())])
            .unwrap();
        assert!(names.is_empty());
    }

    #[test]
    fn project_tags_add_remove_map() {
        let (_d, s) = store();
        s.add_project_tag("/p1", "work").unwrap();
        s.add_project_tag("/p1", "rust").unwrap();
        s.add_project_tag("/p1", "work").unwrap(); // idempotent
        s.add_project_tag("/p2", "work").unwrap();

        let map = s.project_tags_map().unwrap();
        assert_eq!(
            map.get("/p1").unwrap(),
            &vec!["rust".to_string(), "work".to_string()]
        );
        assert_eq!(map.get("/p2").unwrap(), &vec!["work".to_string()]);

        assert!(s.remove_project_tag("/p1", "rust").unwrap());
        assert!(!s.remove_project_tag("/p1", "rust").unwrap());
        let map = s.project_tags_map().unwrap();
        assert_eq!(map.get("/p1").unwrap(), &vec!["work".to_string()]);
    }

    #[test]
    fn reopen_persists() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("metadata.db");
        {
            let s = MetadataStore::open(&path).unwrap();
            s.add_tag("sess", "keep").unwrap();
            s.set_session_name("sess", "Named").unwrap();
            s.add_project_tag("/root", "hub").unwrap();
        }
        let s = MetadataStore::open(&path).unwrap();
        assert_eq!(
            s.tags_for_session("claude:sess", "sess").unwrap(),
            vec!["keep"]
        );
        assert_eq!(
            s.names_for_sessions(&[("claude:sess".into(), "sess".into())])
                .unwrap()
                .get("claude:sess")
                .map(String::as_str),
            Some("Named")
        );
        assert!(s.project_tags_map().unwrap().contains_key("/root"));
    }
}
