use rusqlite::Result as SqlResult;
use crate::db::Database;
use crate::model::HasnConversation;

impl Database {
    /// 插入或更新会话
    pub fn upsert_conversation(&self, conv: &HasnConversation) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO conversations
             (id, conv_type, peer_hasn_id, peer_star_id, peer_name, peer_type,
              peer_avatar_url, last_message_at, last_message_preview,
              message_count, unread_count, status, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, datetime('now'))
             ON CONFLICT(id) DO UPDATE SET
              peer_name = COALESCE(excluded.peer_name, peer_name),
              peer_avatar_url = COALESCE(excluded.peer_avatar_url, peer_avatar_url),
              last_message_at = COALESCE(excluded.last_message_at, last_message_at),
              last_message_preview = COALESCE(excluded.last_message_preview, last_message_preview),
              message_count = excluded.message_count,
              status = excluded.status,
              updated_at = datetime('now')",
            rusqlite::params![
                conv.id,
                conv.conv_type,
                conv.peer_hasn_id,
                conv.peer_star_id,
                conv.peer_name,
                conv.peer_type,
                conv.peer_avatar_url,
                conv.last_message_at,
                conv.last_message_preview,
                conv.message_count,
                conv.unread_count,
                conv.status,
            ],
        )?;
        Ok(())
    }

    /// 获取所有会话 (按最后消息时间倒序)
    pub fn list_conversations(&self) -> SqlResult<Vec<HasnConversation>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, conv_type, peer_hasn_id, peer_star_id, peer_name, peer_type,
                    peer_avatar_url, last_message_at, last_message_preview,
                    message_count, unread_count, status
             FROM conversations
             WHERE status = 'active'
             ORDER BY last_message_at DESC"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(HasnConversation {
                id: row.get(0)?,
                conv_type: row.get(1)?,
                peer_hasn_id: row.get(2)?,
                peer_star_id: row.get(3)?,
                peer_name: row.get(4)?,
                peer_type: row.get(5)?,
                peer_avatar_url: row.get(6)?,
                last_message_at: row.get(7)?,
                last_message_preview: row.get(8)?,
                message_count: row.get(9)?,
                unread_count: row.get(10)?,
                status: row.get(11)?,
            })
        })?;

        let mut convs = Vec::new();
        for row in rows {
            convs.push(row?);
        }
        Ok(convs)
    }

    /// 更新会话最后消息
    pub fn update_conversation_last_message(
        &self,
        conversation_id: &str,
        preview: &str,
        timestamp: &str,
    ) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE conversations
             SET last_message_preview = ?1,
                 last_message_at = ?2,
                 message_count = message_count + 1,
                 updated_at = datetime('now')
             WHERE id = ?3",
            rusqlite::params![preview, timestamp, conversation_id],
        )?;
        Ok(())
    }

    /// 增加未读数
    pub fn increment_unread(&self, conversation_id: &str) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE conversations SET unread_count = unread_count + 1 WHERE id = ?1",
            [conversation_id],
        )?;
        Ok(())
    }

    /// 清除未读数
    pub fn clear_unread(&self, conversation_id: &str) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE conversations SET unread_count = 0 WHERE id = ?1",
            [conversation_id],
        )?;
        Ok(())
    }

    /// 更新未读数 (同步时)
    pub fn update_unread_count(&self, conversation_id: &str, count: i32) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE conversations SET unread_count = ?1 WHERE id = ?2",
            rusqlite::params![count, conversation_id],
        )?;
        Ok(())
    }

    /// 获取同步游标
    pub fn get_sync_cursor(&self, conversation_id: &str) -> SqlResult<Option<i64>> {
        let conn = self.conn.lock().unwrap();
        match conn.query_row(
            "SELECT last_synced_id FROM sync_cursors WHERE conversation_id = ?1",
            [conversation_id],
            |row| row.get::<_, i64>(0),
        ) {
            Ok(id) => Ok(Some(id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// 更新同步游标
    pub fn update_sync_cursor(&self, conversation_id: &str, last_id: i64) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO sync_cursors (conversation_id, last_synced_id, synced_at)
             VALUES (?1, ?2, datetime('now'))
             ON CONFLICT(conversation_id) DO UPDATE SET
              last_synced_id = MAX(excluded.last_synced_id, last_synced_id),
              synced_at = datetime('now')",
            rusqlite::params![conversation_id, last_id],
        )?;
        Ok(())
    }
}
