//! HASN 本地聊天数据库 (Per-Tenant 隔离)
//!
//! 存储路径: `{config_dir}/users/{tenant_dir}/data/hasn_chat.db`
//! 职责: IM 聊天记录、联系人、会话列表（与 Agent sessions.db 严格分离）

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════════
// Data Records
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessageRecord {
    pub id: i64,
    pub message_id: String,
    pub conversation_id: String,
    pub sender_id: String,
    pub receiver_id: String,
    pub content_type: String,
    pub content: String,
    pub status: String,
    pub is_outgoing: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactRecord {
    pub hasn_id: String,
    pub nickname: Option<String>,
    pub avatar_url: Option<String>,
    pub contact_type: String, // "human" | "agent"
    pub relation_type: String, // "social" | "commerce" | "service" | "professional"
    pub trust_level: i32,
    pub status: String, // "pending" | "connected" | "blocked"
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub conversation_id: String,
    pub session_type: String, // "p2p" | "group"
    pub peer_id: String,
    pub title: Option<String>,
    pub last_message_id: Option<String>,
    pub last_message_preview: Option<String>,
    pub unread_count: i32,
    pub updated_at: String,
}

// ═══════════════════════════════════════════════════════════════════
// Database Handle
// ═══════════════════════════════════════════════════════════════════

/// HASN Chat Database Handle (Per-Tenant)
#[derive(Clone)]
pub struct HasnChatDb {
    pub db_path: PathBuf,
    conn: Arc<Mutex<rusqlite::Connection>>,
}

impl HasnChatDb {
    /// Open or create the local chat database
    pub fn open(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = rusqlite::Connection::open(db_path).context("Failed to open hasn_chat database")?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")?;

        let db = Self {
            db_path: db_path.to_path_buf(),
            conn: Arc::new(Mutex::new(conn)),
        };
        db.ensure_schema()?;
        Ok(db)
    }

    fn ensure_schema(&self) -> Result<()> {
        let conn = self.conn.try_lock().map_err(|_| anyhow::anyhow!("DB lock failed"))?;
        
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                message_id TEXT NOT NULL UNIQUE,
                conversation_id TEXT NOT NULL,
                sender_id TEXT NOT NULL,
                receiver_id TEXT NOT NULL,
                content_type TEXT NOT NULL DEFAULT 'text',
                content TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'sent',
                is_outgoing BOOLEAN NOT NULL DEFAULT 0,
                created_at DATETIME DEFAULT (datetime('now', 'localtime'))
            );
            CREATE INDEX IF NOT EXISTS idx_messages_conv ON messages(conversation_id);
            CREATE INDEX IF NOT EXISTS idx_messages_msgid ON messages(message_id);
            CREATE INDEX IF NOT EXISTS idx_messages_created ON messages(conversation_id, created_at);

            CREATE TABLE IF NOT EXISTS contacts (
                hasn_id TEXT PRIMARY KEY,
                nickname TEXT,
                avatar_url TEXT,
                contact_type TEXT NOT NULL DEFAULT 'human',
                relation_type TEXT NOT NULL DEFAULT 'social',
                trust_level INTEGER NOT NULL DEFAULT 2,
                status TEXT NOT NULL DEFAULT 'connected',
                created_at DATETIME DEFAULT (datetime('now', 'localtime')),
                updated_at DATETIME DEFAULT (datetime('now', 'localtime'))
            );

            CREATE TABLE IF NOT EXISTS sessions (
                conversation_id TEXT PRIMARY KEY,
                session_type TEXT NOT NULL DEFAULT 'p2p',
                peer_id TEXT NOT NULL,
                title TEXT,
                last_message_id TEXT,
                last_message_preview TEXT,
                unread_count INTEGER DEFAULT 0,
                created_at DATETIME DEFAULT (datetime('now', 'localtime')),
                updated_at DATETIME DEFAULT (datetime('now', 'localtime')),
                FOREIGN KEY(last_message_id) REFERENCES messages(message_id)
            );
            CREATE INDEX IF NOT EXISTS idx_sessions_updated ON sessions(updated_at);

            CREATE TABLE IF NOT EXISTS sync_state (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at DATETIME DEFAULT (datetime('now', 'localtime'))
            );
            "
        )?;

        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════
    // Message Methods
    // ═══════════════════════════════════════════════════════════════

    pub async fn insert_message(&self, msg: &ChatMessageRecord) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO messages (
                message_id, conversation_id, sender_id, receiver_id, 
                content_type, content, status, is_outgoing, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(message_id) DO UPDATE SET 
                status = excluded.status",
            rusqlite::params![
                msg.message_id,
                msg.conversation_id,
                msg.sender_id,
                msg.receiver_id,
                msg.content_type,
                msg.content,
                msg.status,
                msg.is_outgoing,
                msg.created_at,
            ],
        )?;

        // Derive preview text
        let preview = derive_preview(&msg.content_type, &msg.content);

        // Update the session's last message + preview
        conn.execute(
            "UPDATE sessions 
             SET last_message_id = ?1, last_message_preview = ?2, updated_at = ?3
             WHERE conversation_id = ?4",
            rusqlite::params![msg.message_id, preview, msg.created_at, msg.conversation_id],
        )?;

        // Increment unread for non-outgoing messages
        if !msg.is_outgoing {
            conn.execute(
                "UPDATE sessions SET unread_count = unread_count + 1 WHERE conversation_id = ?1",
                rusqlite::params![msg.conversation_id],
            )?;
        }

        Ok(())
    }

    pub async fn update_message_status(&self, message_id: &str, status: &str) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "UPDATE messages SET status = ?1 WHERE message_id = ?2",
            rusqlite::params![status, message_id],
        )?;
        Ok(())
    }

    pub async fn mark_recalled(&self, message_id: &str) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "UPDATE messages SET status = 'recalled', content = '\"[消息已撤回]\"' WHERE message_id = ?1",
            rusqlite::params![message_id],
        )?;
        Ok(())
    }

    pub async fn get_messages(&self, conversation_id: &str, limit: u32, offset: u32) -> Result<Vec<ChatMessageRecord>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare_cached(
            "SELECT id, message_id, conversation_id, sender_id, receiver_id, 
                    content_type, content, status, is_outgoing, created_at 
             FROM messages WHERE conversation_id = ?1 
             ORDER BY created_at DESC LIMIT ?2 OFFSET ?3"
        )?;

        let rows = stmt.query_map(rusqlite::params![conversation_id, limit, offset], |row| {
            Ok(ChatMessageRecord {
                id: row.get(0)?,
                message_id: row.get(1)?,
                conversation_id: row.get(2)?,
                sender_id: row.get(3)?,
                receiver_id: row.get(4)?,
                content_type: row.get(5)?,
                content: row.get(6)?,
                status: row.get(7)?,
                is_outgoing: row.get(8)?,
                created_at: row.get(9)?,
            })
        })?;

        let mut messages = Vec::new();
        for r in rows {
            messages.push(r?);
        }
        messages.reverse(); // Return in chronological order
        Ok(messages)
    }

    // ═══════════════════════════════════════════════════════════════
    // Session Methods
    // ═══════════════════════════════════════════════════════════════

    pub async fn upsert_session(&self, conversation_id: &str, session_type: &str, peer_id: &str) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO sessions (conversation_id, session_type, peer_id)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(conversation_id) DO NOTHING",
            rusqlite::params![conversation_id, session_type, peer_id],
        )?;
        Ok(())
    }

    pub async fn mark_read(&self, conversation_id: &str) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "UPDATE sessions SET unread_count = 0 WHERE conversation_id = ?1",
            rusqlite::params![conversation_id],
        )?;
        Ok(())
    }

    pub async fn get_sessions(&self) -> Result<Vec<SessionRecord>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare_cached(
            "SELECT conversation_id, session_type, peer_id, title, 
                    last_message_id, last_message_preview, unread_count, updated_at 
             FROM sessions ORDER BY updated_at DESC"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(SessionRecord {
                conversation_id: row.get(0)?,
                session_type: row.get(1)?,
                peer_id: row.get(2)?,
                title: row.get(3)?,
                last_message_id: row.get(4)?,
                last_message_preview: row.get(5)?,
                unread_count: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })?;

        let mut sessions = Vec::new();
        for r in rows {
            sessions.push(r?);
        }
        Ok(sessions)
    }

    // ═══════════════════════════════════════════════════════════════
    // Contact / Trust Methods
    // ═══════════════════════════════════════════════════════════════

    pub async fn get_contact(&self, hasn_id: &str) -> Result<Option<ContactRecord>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare_cached(
            "SELECT hasn_id, nickname, avatar_url, contact_type, relation_type, 
                    trust_level, status, created_at 
             FROM contacts WHERE hasn_id = ?1 LIMIT 1"
        )?;

        match stmt.query_row(rusqlite::params![hasn_id], |row| {
            Ok(ContactRecord {
                hasn_id: row.get(0)?,
                nickname: row.get(1)?,
                avatar_url: row.get(2)?,
                contact_type: row.get(3)?,
                relation_type: row.get(4)?,
                trust_level: row.get(5)?,
                status: row.get(6)?,
                created_at: row.get(7)?,
            })
        }) {
            Ok(c) => Ok(Some(c)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn get_all_contacts(&self) -> Result<Vec<ContactRecord>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare_cached(
            "SELECT hasn_id, nickname, avatar_url, contact_type, relation_type,
                    trust_level, status, created_at
             FROM contacts WHERE status = 'connected' ORDER BY nickname"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(ContactRecord {
                hasn_id: row.get(0)?,
                nickname: row.get(1)?,
                avatar_url: row.get(2)?,
                contact_type: row.get(3)?,
                relation_type: row.get(4)?,
                trust_level: row.get(5)?,
                status: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?;

        let mut contacts = Vec::new();
        for r in rows {
            contacts.push(r?);
        }
        Ok(contacts)
    }

    pub async fn upsert_contact(&self, contact: &ContactRecord) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO contacts (hasn_id, nickname, avatar_url, contact_type, relation_type, trust_level, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(hasn_id) DO UPDATE SET 
                nickname = excluded.nickname,
                avatar_url = excluded.avatar_url,
                relation_type = excluded.relation_type,
                trust_level = excluded.trust_level,
                status = excluded.status,
                updated_at = datetime('now', 'localtime')",
            rusqlite::params![
                contact.hasn_id,
                contact.nickname,
                contact.avatar_url,
                contact.contact_type,
                contact.relation_type,
                contact.trust_level,
                contact.status,
            ],
        )?;
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════
    // Sync State Methods
    // ═══════════════════════════════════════════════════════════════

    pub async fn get_sync_state(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().await;
        match conn.query_row(
            "SELECT value FROM sync_state WHERE key = ?1",
            rusqlite::params![key],
            |row| row.get::<_, String>(0),
        ) {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn set_sync_state(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO sync_state (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now', 'localtime')",
            rusqlite::params![key, value],
        )?;
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════
    // Phase 05-05 Task 5b — peer_id 历史数据修正
    // ═══════════════════════════════════════════════════════════════

    /// 幂等迁移：修正同 owner Agent 会话 `sessions.peer_id` 从 `h_*` 变为
    /// 对端 Agent 的 `a_*`。
    ///
    /// 触发条件：当前 `sessions.peer_id` 是 `h_*`（写错的 Owner 自己）且
    /// 相同 `conversation_id` 的 `messages` 表里存在 `receiver_id LIKE 'a_%'`
    /// （说明对话一方是 Agent，peer 应当是这个 Agent）。
    ///
    /// 幂等性由 `sync_state(key="phase_05_05_peer_id_migration")` 保证：
    /// 成功一次后标记 "done"，后续调用早退返 0。
    pub async fn run_migration_phase_05_05_peer_id(&self) -> Result<u64> {
        const KEY: &str = "phase_05_05_peer_id_migration";
        if self.get_sync_state(KEY).await?.is_some() {
            return Ok(0);
        }

        let updated: u64 = {
            let conn = self.conn.lock().await;
            conn.execute(
                "UPDATE sessions SET peer_id = (
                    SELECT receiver_id FROM messages
                    WHERE messages.conversation_id = sessions.conversation_id
                      AND receiver_id LIKE 'a_%'
                    LIMIT 1
                 )
                 WHERE peer_id LIKE 'h_%'
                   AND EXISTS (
                       SELECT 1 FROM messages
                       WHERE messages.conversation_id = sessions.conversation_id
                         AND receiver_id LIKE 'a_%'
                   )",
                [],
            )? as u64
        };

        self.set_sync_state(KEY, "done").await?;
        Ok(updated)
    }
}

// ═══════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════

/// Derive a short preview string from content for session list display
fn derive_preview(content_type: &str, content: &str) -> String {
    match content_type {
        "text" | "1" => {
            // Try to extract "text" field from JSON
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(content) {
                if let Some(t) = v.get("text").and_then(|t| t.as_str()) {
                    let truncated: String = t.chars().take(100).collect();
                    return truncated;
                }
            }
            let truncated: String = content.chars().take(100).collect();
            truncated
        }
        "image" | "2" => "[图片]".to_string(),
        "file" | "3" => "[文件]".to_string(),
        "voice" | "4" => "[语音]".to_string(),
        "card" | "5" => "[卡片]".to_string(),
        "tool_call" | "6" => "[工具调用]".to_string(),
        _ => "[消息]".to_string(),
    }
}
