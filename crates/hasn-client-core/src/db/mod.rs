pub mod contacts;
pub mod conversations;
pub mod messages;
pub mod schema;

use crate::model::AuthState;
use rusqlite::{Connection, Result as SqlResult};
use std::sync::Mutex;

/// 本地数据库
pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    /// 打开/创建数据库
    pub fn new(path: &str) -> SqlResult<Self> {
        let conn = Connection::open(path)?;

        // 性能优化
        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA foreign_keys = ON;
        ",
        )?;

        let db = Self {
            conn: Mutex::new(conn),
        };
        db.init_tables()?;
        Ok(db)
    }

    /// 创建所有表
    fn init_tables(&self) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(schema::CREATE_ALL)?;
        ensure_contacts_owner_id_column(&conn)?;
        Ok(())
    }

    /// 保存认证状态
    pub fn save_auth_state(&self, auth: &AuthState) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        let json = serde_json::to_string(auth).unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO auth_state (key, value) VALUES ('current', ?1)",
            [&json],
        )?;
        Ok(())
    }

    /// 读取认证状态
    pub fn load_auth_state(&self) -> SqlResult<Option<AuthState>> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT value FROM auth_state WHERE key = 'current'",
            [],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(json) => Ok(serde_json::from_str(&json).ok()),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// 清除认证状态
    pub fn clear_auth_state(&self) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM auth_state WHERE key = 'current'", [])?;
        Ok(())
    }
}

/// US-006：旧版本数据库的 `contacts` 表可能缺 `owner_id` 列（早于本迁移建表）；
/// 幂等地 `ALTER TABLE contacts ADD COLUMN owner_id` 兜底，不影响全新库。
fn ensure_contacts_owner_id_column(conn: &Connection) -> SqlResult<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(contacts)")?;
    let mut rows = stmt.query([])?;
    let mut has_owner_id = false;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == "owner_id" {
            has_owner_id = true;
            break;
        }
    }
    drop(rows);
    drop(stmt);

    if !has_owner_id {
        conn.execute(
            "ALTER TABLE contacts ADD COLUMN owner_id TEXT NOT NULL DEFAULT ''",
            [],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_path(name: &str) -> String {
        let dir = std::env::temp_dir().join(format!(
            "hasn_client_core_test_{}_{}_{}.sqlite",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        dir.to_string_lossy().to_string()
    }

    #[test]
    fn fresh_database_has_owner_id_column() {
        let path = tmp_path("fresh");
        let db = Database::new(&path).expect("open fresh db");
        let conn = db.conn.lock().unwrap();
        let mut stmt = conn.prepare("PRAGMA table_info(contacts)").unwrap();
        let mut rows = stmt.query([]).unwrap();
        let mut names = Vec::new();
        while let Some(row) = rows.next().unwrap() {
            names.push(row.get::<_, String>(1).unwrap());
        }
        drop(rows);
        drop(stmt);
        drop(conn);
        let _ = std::fs::remove_file(&path);
        assert!(names.iter().any(|n| n == "owner_id"), "columns: {names:?}");
    }

    #[test]
    fn legacy_database_gains_owner_id_via_alter_table() {
        let path = tmp_path("legacy");

        // Seed legacy-style contacts table (no owner_id).
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS contacts (
                    id              INTEGER PRIMARY KEY,
                    peer_hasn_id    TEXT NOT NULL,
                    peer_star_id    TEXT NOT NULL,
                    peer_name       TEXT NOT NULL,
                    peer_type       TEXT NOT NULL DEFAULT 'human',
                    peer_avatar_url TEXT,
                    peer_owner_id   TEXT,
                    relation_type   TEXT DEFAULT 'social',
                    trust_level     INTEGER DEFAULT 1,
                    nickname        TEXT,
                    tags            TEXT,
                    status          TEXT DEFAULT 'pending',
                    connected_at    TEXT,
                    updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
                    UNIQUE(peer_hasn_id, relation_type)
                );",
            )
            .unwrap();
        }

        // Re-open via Database::new — migration must add owner_id idempotently.
        let db = Database::new(&path).expect("open legacy db");
        let conn = db.conn.lock().unwrap();
        let mut stmt = conn.prepare("PRAGMA table_info(contacts)").unwrap();
        let mut rows = stmt.query([]).unwrap();
        let mut names = Vec::new();
        while let Some(row) = rows.next().unwrap() {
            names.push(row.get::<_, String>(1).unwrap());
        }
        drop(rows);
        drop(stmt);
        drop(conn);

        // Re-running migration on already-migrated DB must stay idempotent.
        let _db2 = Database::new(&path).expect("idempotent re-open");

        let _ = std::fs::remove_file(&path);
        assert!(names.iter().any(|n| n == "owner_id"), "columns: {names:?}");
    }
}
