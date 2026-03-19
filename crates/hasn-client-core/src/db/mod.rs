pub mod schema;
pub mod messages;
pub mod conversations;
pub mod contacts;

use rusqlite::{Connection, Result as SqlResult};
use std::sync::Mutex;
use crate::model::AuthState;

/// 本地数据库
pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    /// 打开/创建数据库
    pub fn new(path: &str) -> SqlResult<Self> {
        let conn = Connection::open(path)?;

        // 性能优化
        conn.execute_batch("
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA foreign_keys = ON;
        ")?;

        let db = Self { conn: Mutex::new(conn) };
        db.init_tables()?;
        Ok(db)
    }

    /// 创建所有表
    fn init_tables(&self) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(schema::CREATE_ALL)?;
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
