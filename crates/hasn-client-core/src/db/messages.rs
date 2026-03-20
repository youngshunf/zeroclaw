use crate::db::Database;
use crate::model::{HasnMessage, SendStatus};
use rusqlite::Result as SqlResult;

impl Database {
    /// 插入消息 (发送时或收到时)
    pub fn insert_message(&self, msg: &HasnMessage) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO messages
             (id, local_id, conversation_id, from_id, from_star_id, from_type,
              content, content_type, metadata, reply_to, status, send_status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            rusqlite::params![
                if msg.id > 0 { Some(msg.id) } else { None },
                msg.local_id,
                msg.conversation_id,
                msg.from_id,
                msg.from_star_id,
                msg.from_type,
                msg.content,
                msg.content_type,
                msg.metadata.as_ref().map(|v| v.to_string()),
                msg.reply_to,
                msg.status,
                msg.send_status.as_str(),
                msg.created_at,
            ],
        )?;
        Ok(())
    }

    /// 更新消息: 发送成功后用服务端数据更新本地记录
    pub fn update_message_after_send(
        &self,
        local_id: &str,
        server_id: i64,
        conversation_id: &str,
        created_at: Option<&str>,
    ) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE messages SET id = ?1, conversation_id = ?2, send_status = 'sent', created_at = COALESCE(?3, created_at)
             WHERE local_id = ?4",
            rusqlite::params![server_id, conversation_id, created_at, local_id],
        )?;
        Ok(())
    }

    /// 标记消息发送失败
    pub fn mark_message_failed(&self, local_id: &str) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE messages SET send_status = 'failed' WHERE local_id = ?1",
            [local_id],
        )?;
        Ok(())
    }

    /// upsert 消息 (同步时: 服务端ID已知)
    pub fn upsert_message(&self, msg: &HasnMessage) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        // 先尝试按 server_id 查找
        if msg.id > 0 {
            let exists: bool = conn.query_row(
                "SELECT COUNT(*) > 0 FROM messages WHERE id = ?1",
                [msg.id],
                |row| row.get(0),
            )?;
            if exists {
                return Ok(()); // 已存在, 不重复写入
            }
        }
        drop(conn);
        self.insert_message(msg)
    }

    /// 获取会话的消息列表 (游标分页, 按时间倒序)
    pub fn get_messages(
        &self,
        conversation_id: &str,
        before_id: Option<i64>,
        limit: i32,
    ) -> SqlResult<Vec<HasnMessage>> {
        let conn = self.conn.lock().unwrap();

        let mut messages = Vec::new();

        if let Some(bid) = before_id {
            let mut stmt = conn.prepare(
                "SELECT id, local_id, conversation_id, from_id, from_star_id, from_type,
                        content, content_type, metadata, reply_to, status, send_status, created_at
                 FROM messages
                 WHERE conversation_id = ?1 AND (id < ?2 OR id IS NULL)
                 ORDER BY created_at DESC
                 LIMIT ?3",
            )?;
            let rows = stmt.query_map(rusqlite::params![conversation_id, bid, limit], |row| {
                Ok(Self::row_to_message(row))
            })?;
            for row in rows {
                messages.push(row?);
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, local_id, conversation_id, from_id, from_star_id, from_type,
                        content, content_type, metadata, reply_to, status, send_status, created_at
                 FROM messages
                 WHERE conversation_id = ?1
                 ORDER BY created_at DESC
                 LIMIT ?2",
            )?;
            let rows = stmt.query_map(rusqlite::params![conversation_id, limit], |row| {
                Ok(Self::row_to_message(row))
            })?;
            for row in rows {
                messages.push(row?);
            }
        }

        // 翻转为正序 (时间升序显示)
        messages.reverse();
        Ok(messages)
    }

    /// 获取发送失败的消息
    pub fn get_failed_messages(&self) -> SqlResult<Vec<HasnMessage>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, local_id, conversation_id, from_id, from_star_id, from_type,
                    content, content_type, metadata, reply_to, status, send_status, created_at
             FROM messages WHERE send_status = 'failed'
             ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([], |row| Ok(Self::row_to_message(row)))?;
        let mut messages = Vec::new();
        for row in rows {
            messages.push(row?);
        }
        Ok(messages)
    }

    fn row_to_message(row: &rusqlite::Row) -> HasnMessage {
        let metadata_str: Option<String> = row.get(8).unwrap_or(None);
        let send_status_str: String = row.get(11).unwrap_or_else(|_| "sending".to_string());

        HasnMessage {
            id: row.get(0).unwrap_or(0),
            local_id: row.get(1).unwrap_or_default(),
            conversation_id: row.get(2).unwrap_or_default(),
            from_id: row.get(3).unwrap_or_default(),
            from_star_id: row.get(4).unwrap_or(None),
            from_type: row.get(5).unwrap_or(1),
            content: row.get(6).unwrap_or_default(),
            content_type: row.get(7).unwrap_or(1),
            metadata: metadata_str.and_then(|s| serde_json::from_str(&s).ok()),
            reply_to: row.get(9).unwrap_or(None),
            status: row.get(10).unwrap_or(1),
            send_status: SendStatus::from_str(&send_status_str),
            created_at: row.get(12).unwrap_or(None),
        }
    }
}
