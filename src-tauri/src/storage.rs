use rusqlite::Connection;
use std::path::Path;

pub struct Store {
    pub conn: Connection,
}

const MIGRATION_V1: &str = "
CREATE TABLE IF NOT EXISTS clipboard_items (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    kind        TEXT    NOT NULL,
    content     TEXT,
    image_path  TEXT,
    thumb_path  TEXT,
    source_app  TEXT,
    pinned      INTEGER NOT NULL DEFAULT 0,
    created_at  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_clip_created ON clipboard_items(created_at DESC);
CREATE TABLE IF NOT EXISTS app_usage (
    path         TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    launch_count INTEGER NOT NULL DEFAULT 0,
    last_used    INTEGER
);
CREATE TABLE IF NOT EXISTS file_usage (
    path       TEXT PRIMARY KEY,
    open_count INTEGER NOT NULL DEFAULT 0,
    last_used  INTEGER
);
PRAGMA user_version = 1;
";

impl Store {
    pub fn open(db_path: &Path) -> anyhow::Result<Store> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        let store = Store { conn };
        store.run_migrations()?;
        Ok(store)
    }

    fn run_migrations(&self) -> anyhow::Result<()> {
        self.conn.execute_batch(MIGRATION_V1)?;
        Ok(())
    }

    pub fn table_names(&self) -> anyhow::Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?;
        let names = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<String>, _>>()?;
        Ok(names)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn migrations_create_expected_tables() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("stash.db");
        let store = Store::open(&db).unwrap();
        let tables = store.table_names().unwrap();
        assert!(tables.contains(&"clipboard_items".to_string()));
        assert!(tables.contains(&"app_usage".to_string()));
        assert!(tables.contains(&"file_usage".to_string()));
    }

    #[test]
    fn open_creates_missing_parent_dirs() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("nested/sub/stash.db");
        assert!(Store::open(&db).is_ok());
        assert!(db.exists());
    }
}
