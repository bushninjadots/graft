use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

/// Bump together with a new file in `src/schema/` and a branch in `migrate`.
pub const SCHEMA_VERSION: i64 = 5;

pub struct Db(pub Mutex<Connection>);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub description: String,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self, String> {
        let conn = Connection::open(path).map_err(|e| format!("cannot open database: {e}"))?;
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")
            .map_err(|e| format!("cannot configure database: {e}"))?;
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(|e| format!("cannot set busy timeout: {e}"))?;
        migrate(&conn)?;
        Ok(Self(Mutex::new(conn)))
    }

    fn conn(&self) -> MutexGuard<'_, Connection> {
        self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, String> {
        let conn = self.conn();
        conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| format!("cannot read setting '{key}': {e}"))
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), String> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![key, value, now()],
        )
        .map(|_| ())
        .map_err(|e| format!("cannot write setting '{key}': {e}"))
    }

    pub fn delete_setting(&self, key: &str) -> Result<(), String> {
        let conn = self.conn();
        conn.execute("DELETE FROM settings WHERE key = ?1", params![key])
            .map(|_| ())
            .map_err(|e| format!("cannot delete setting '{key}': {e}"))
    }

    pub fn list_projects(&self) -> Result<Vec<Project>, String> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare(
                "SELECT id, name, path, description, created_at, updated_at
                 FROM projects ORDER BY updated_at DESC, name COLLATE NOCASE ASC",
            )
            .map_err(|e| format!("cannot query projects: {e}"))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(Project {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    path: row.get(2)?,
                    description: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })
            .map_err(|e| format!("cannot read projects: {e}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("cannot decode projects: {e}"))
    }

    pub fn create_project(
        &self,
        name: &str,
        path: &str,
        description: &str,
        at: i64,
    ) -> Result<Project, String> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO projects (name, path, description, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?4)",
            params![name, path, description, at],
        )
        .map_err(|e| format!("cannot create project: {e}"))?;
        let id = conn.last_insert_rowid();
        Ok(Project {
            id,
            name: name.to_string(),
            path: path.to_string(),
            description: description.to_string(),
            created_at: at,
            updated_at: at,
        })
    }

    pub fn delete_project(&self, id: i64) -> Result<(), String> {
        let conn = self.conn();
        let removed = conn
            .execute("DELETE FROM projects WHERE id = ?1", params![id])
            .map_err(|e| format!("cannot delete project: {e}"))?;
        if removed == 0 {
            return Err("project not found".into());
        }
        Ok(())
    }
}

fn migrate(conn: &Connection) -> Result<(), String> {
    let current: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|e| format!("cannot read schema version: {e}"))?;

    if current >= SCHEMA_VERSION {
        return Ok(());
    }

    if current < 1 {
        conn.execute_batch(include_str!("schema/001_init.sql"))
            .map_err(|e| format!("migration 001 failed: {e}"))?;
    }

    if current < 2 {
        conn.execute_batch(include_str!("schema/002_watchtower.sql"))
            .map_err(|e| format!("migration 002 failed: {e}"))?;
    }

    if current < 3 {
        conn.execute_batch(include_str!("schema/003_library.sql"))
            .map_err(|e| format!("migration 003 failed: {e}"))?;
    }

    if current < 4 {
        conn.execute_batch(include_str!("schema/004_discovery.sql"))
            .map_err(|e| format!("migration 004 failed: {e}"))?;
    }

    if current < 5 {
        conn.execute_batch(include_str!("schema/005_extract.sql"))
            .map_err(|e| format!("migration 005 failed: {e}"))?;
    }

    conn.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION};"))
        .map_err(|e| format!("cannot record schema version: {e}"))?;
    Ok(())
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(label: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("graft-{label}-{}-{nanos}.db", std::process::id()))
    }

    fn cleanup(path: &Path) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(format!("{}-wal", path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", path.display()));
    }

    #[test]
    fn migration_is_applied_and_idempotent() {
        let path = temp_path("migrate");
        {
            let db = Db::open(&path).expect("first open");
            let version: i64 = db
                .conn()
                .pragma_query_value(None, "user_version", |row| row.get(0))
                .expect("read version");
            assert_eq!(version, SCHEMA_VERSION);
        }
        {
            let db = Db::open(&path).expect("second open");
            let version: i64 = db
                .conn()
                .pragma_query_value(None, "user_version", |row| row.get(0))
                .expect("read version");
            assert_eq!(version, SCHEMA_VERSION);
        }
        cleanup(&path);
    }

    #[test]
    fn settings_roundtrip_and_overwrite() {
        let path = temp_path("settings");
        let db = Db::open(&path).expect("open");

        assert_eq!(db.get_setting("theme").expect("read empty"), None);
        db.set_setting("theme", "graft").expect("write");
        assert_eq!(db.get_setting("theme").expect("read").as_deref(), Some("graft"));
        db.set_setting("theme", "void").expect("overwrite");
        assert_eq!(db.get_setting("theme").expect("read").as_deref(), Some("void"));

        db.delete_setting("theme").expect("delete");
        assert_eq!(db.get_setting("theme").expect("read"), None);

        cleanup(&path);
    }

    #[test]
    fn project_lifecycle() {
        let path = temp_path("projects");
        let db = Db::open(&path).expect("open");

        let first = db.create_project("MOOI", "/tmp/mooi", "shell", 100).expect("create");
        let second = db.create_project("Tethyr", "", "", 200).expect("create");

        let listed = db.list_projects().expect("list");
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].id, second.id, "newest project sorts first");
        assert_eq!(listed[1].name, "MOOI");

        db.delete_project(first.id).expect("delete");
        assert_eq!(db.list_projects().expect("list").len(), 1);
        assert!(db.delete_project(first.id).is_err(), "second delete must fail");

        cleanup(&path);
    }
}
