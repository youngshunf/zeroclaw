use rusqlite::Result as SqlResult;
use crate::db::Database;
use crate::model::HasnContact;

impl Database {
    /// 插入或更新联系人
    pub fn upsert_contact(&self, contact: &HasnContact) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        let tags_json = contact.tags.as_ref().map(|t| serde_json::to_string(t).unwrap_or_default());
        conn.execute(
            "INSERT INTO contacts
             (id, peer_hasn_id, peer_star_id, peer_name, peer_type,
              peer_avatar_url, relation_type, trust_level, nickname,
              tags, status, connected_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, datetime('now'))
             ON CONFLICT(peer_hasn_id, relation_type) DO UPDATE SET
              peer_star_id = excluded.peer_star_id,
              peer_name = excluded.peer_name,
              peer_avatar_url = COALESCE(excluded.peer_avatar_url, peer_avatar_url),
              trust_level = excluded.trust_level,
              nickname = COALESCE(excluded.nickname, nickname),
              tags = COALESCE(excluded.tags, tags),
              status = excluded.status,
              updated_at = datetime('now')",
            rusqlite::params![
                contact.id,
                contact.peer_hasn_id,
                contact.peer_star_id,
                contact.peer_name,
                contact.peer_type,
                contact.peer_avatar_url,
                contact.relation_type,
                contact.trust_level,
                contact.nickname,
                tags_json,
                contact.status,
                contact.connected_at,
            ],
        )?;
        Ok(())
    }

    /// 获取联系人列表
    pub fn list_contacts(&self, relation_type: &str) -> SqlResult<Vec<HasnContact>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, peer_hasn_id, peer_star_id, peer_name, peer_type,
                    peer_avatar_url, relation_type, trust_level, nickname,
                    tags, status, connected_at
             FROM contacts
             WHERE relation_type = ?1 AND status = 'connected'
             ORDER BY peer_name ASC"
        )?;

        let rows = stmt.query_map([relation_type], |row| {
            let tags_str: Option<String> = row.get(9)?;
            let tags: Option<Vec<String>> = tags_str
                .and_then(|s| serde_json::from_str(&s).ok());
            Ok(HasnContact {
                id: row.get(0)?,
                peer_hasn_id: row.get(1)?,
                peer_star_id: row.get(2)?,
                peer_name: row.get(3)?,
                peer_type: row.get(4)?,
                peer_avatar_url: row.get(5)?,
                relation_type: row.get(6)?,
                trust_level: row.get(7)?,
                nickname: row.get(8)?,
                tags,
                status: row.get(10)?,
                connected_at: row.get(11)?,
            })
        })?;

        let mut contacts = Vec::new();
        for row in rows {
            contacts.push(row?);
        }
        Ok(contacts)
    }
}
