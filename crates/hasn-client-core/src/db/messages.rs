use crate::db::Database;
use crate::model::{HasnMessageRecord, SendStatus};
use rusqlite::Result as SqlResult;

impl Database {
    /// 插入消息 (发送时或收到时)
    pub fn insert_message(&self, msg: &HasnMessageRecord) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO messages
             (id, conversation_id, from_hasn_id, from_owner_id, from_entity_type,
              to_hasn_id, to_owner_id, content_type, body, status, send_status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            rusqlite::params![
                msg.id,
                msg.conversation_id,
                msg.from_hasn_id,
                msg.from_owner_id,
                msg.from_entity_type,
                msg.to_hasn_id,
                msg.to_owner_id,
                msg.content_type,
                msg.body,
                msg.status,
                msg.send_status.as_str(),
                msg.created_at,
            ],
        )?;
        Ok(())
    }

    /// 更新消息: 发送成功后确认状态
    pub fn update_message_after_send(
        &self,
        msg_id: &str,
        conversation_id: &str,
        server_received_at: Option<&str>,
    ) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE messages SET conversation_id = ?1, send_status = 'sent',
             created_at = COALESCE(?2, created_at)
             WHERE id = ?3",
            rusqlite::params![conversation_id, server_received_at, msg_id],
        )?;
        Ok(())
    }

    /// 标记消息发送失败
    pub fn mark_message_failed(&self, msg_id: &str) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE messages SET send_status = 'failed' WHERE id = ?1",
            [msg_id],
        )?;
        Ok(())
    }

    /// upsert 消息 (同步时: 服务端消息已有 ID)
    pub fn upsert_message(&self, msg: &HasnMessageRecord) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        let exists: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM messages WHERE id = ?1",
            [&msg.id],
            |row| row.get(0),
        )?;
        if exists {
            return Ok(()); // 已存在, 不重复写入
        }
        drop(conn);
        self.insert_message(msg)
    }

    /// 获取会话的消息列表 (游标分页, 按时间倒序)
    pub fn get_messages(
        &self,
        conversation_id: &str,
        before_id: Option<&str>,
        limit: i32,
    ) -> SqlResult<Vec<HasnMessageRecord>> {
        let conn = self.conn.lock().unwrap();

        let mut messages = Vec::new();

        if let Some(bid) = before_id {
            let mut stmt = conn.prepare(
                "SELECT id, conversation_id, from_hasn_id, from_owner_id, from_entity_type,
                        to_hasn_id, to_owner_id, content_type, body, status, send_status, created_at
                 FROM messages
                 WHERE conversation_id = ?1 AND created_at < (SELECT created_at FROM messages WHERE id = ?2)
                 ORDER BY created_at DESC
                 LIMIT ?3",
            )?;
            let rows = stmt.query_map(rusqlite::params![conversation_id, bid, limit], |row| {
                Ok(Self::row_to_record(row))
            })?;
            for row in rows {
                messages.push(row?);
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, conversation_id, from_hasn_id, from_owner_id, from_entity_type,
                        to_hasn_id, to_owner_id, content_type, body, status, send_status, created_at
                 FROM messages
                 WHERE conversation_id = ?1
                 ORDER BY created_at DESC
                 LIMIT ?2",
            )?;
            let rows = stmt.query_map(rusqlite::params![conversation_id, limit], |row| {
                Ok(Self::row_to_record(row))
            })?;
            for row in rows {
                messages.push(row?);
            }
        }

        // 翻转为正序 (时间升序显示)
        messages.reverse();
        Ok(messages)
    }

    /// 获取发送失败的消息 (用于重发)
    pub fn get_failed_messages(&self) -> SqlResult<Vec<HasnMessageRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, conversation_id, from_hasn_id, from_owner_id, from_entity_type,
                    to_hasn_id, to_owner_id, content_type, body, status, send_status, created_at
             FROM messages WHERE send_status = 'failed'
             ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([], |row| Ok(Self::row_to_record(row)))?;
        let mut messages = Vec::new();
        for row in rows {
            messages.push(row?);
        }
        Ok(messages)
    }

    fn row_to_record(row: &rusqlite::Row) -> HasnMessageRecord {
        let send_status_str: String = row.get(10).unwrap_or_else(|_| "sending".to_string());

        HasnMessageRecord {
            id: row.get(0).unwrap_or_default(),
            conversation_id: row.get(1).unwrap_or_default(),
            from_hasn_id: row.get(2).unwrap_or_default(),
            from_owner_id: row.get(3).unwrap_or_default(),
            from_entity_type: row.get(4).unwrap_or_else(|_| "human".to_string()),
            to_hasn_id: row.get(5).unwrap_or_default(),
            to_owner_id: row.get(6).unwrap_or_default(),
            content_type: row.get(7).unwrap_or_else(|_| "text".to_string()),
            body: row.get(8).unwrap_or_default(),
            status: row.get(9).unwrap_or_else(|_| "sent".to_string()),
            send_status: SendStatus::from_str(&send_status_str),
            created_at: row.get(11).unwrap_or_default(),
        }
    }
}
