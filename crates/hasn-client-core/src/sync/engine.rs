use std::sync::Arc;
use tracing::{error, info, warn};

use crate::api::HasnApiClient;
use crate::db::Database;
use crate::error::HasnError;
use crate::model::*;

/// 消息同步引擎: 本地优先 + 增量同步
pub struct SyncEngine {
    api: Arc<HasnApiClient>,
    db: Arc<Database>,
    /// 当前登录用户 hasn_id
    hasn_id: tokio::sync::RwLock<Option<String>>,
}

impl SyncEngine {
    pub fn new(api: Arc<HasnApiClient>, db: Arc<Database>) -> Self {
        Self {
            api,
            db,
            hasn_id: tokio::sync::RwLock::new(None),
        }
    }

    /// 设置当前用户
    pub async fn set_current_user(&self, hasn_id: &str) {
        *self.hasn_id.write().await = Some(hasn_id.to_string());
    }

    pub async fn current_hasn_id(&self) -> Option<String> {
        self.hasn_id.read().await.clone()
    }

    /// 全量同步 (启动时调用)
    pub async fn full_sync(&self) -> Result<(), HasnError> {
        info!("[Sync] 开始全量同步");

        // 1. 同步会话列表
        match self.api.list_conversations(100, 0).await {
            Ok(convs) => {
                for conv in &convs {
                    if let Err(e) = self.db.upsert_conversation(conv) {
                        warn!("[Sync] 写入会话失败: {}", e);
                    }
                }
                info!("[Sync] 同步了 {} 个会话", convs.len());
            }
            Err(e) => warn!("[Sync] 获取会话列表失败: {}", e),
        }

        // 2. 同步未读数
        match self.api.get_unread_counts().await {
            Ok(unreads) => {
                for (conv_id, count) in &unreads {
                    let _ = self.db.update_unread_count(conv_id, *count);
                }
                info!("[Sync] 同步了 {} 个会话的未读数", unreads.len());
            }
            Err(e) => warn!("[Sync] 获取未读数失败: {}", e),
        }

        // 3. 同步联系人
        match self.api.list_contacts("social").await {
            Ok(contacts) => {
                for contact in &contacts {
                    let _ = self.db.upsert_contact(contact);
                }
                info!("[Sync] 同步了 {} 个联系人", contacts.len());
            }
            Err(e) => warn!("[Sync] 获取联系人失败: {}", e),
        }

        info!("[Sync] 全量同步完成");
        Ok(())
    }

    /// 发送消息 (本地优先)
    pub async fn send_message(
        &self,
        to_star_id: &str,
        content: &str,
        content_type: i32,
    ) -> Result<HasnMessage, HasnError> {
        let hasn_id = self
            .current_hasn_id()
            .await
            .ok_or_else(|| HasnError::Auth("未登录".to_string()))?;

        // 1. 创建本地消息 (状态=sending)
        let local_msg = HasnMessage::new_outgoing("pending", &hasn_id, content, content_type);
        let local_id = local_msg.local_id.clone();

        self.db
            .insert_message(&local_msg)
            .map_err(|e| HasnError::Db(e.to_string()))?;

        // 2. 调 API 发送
        match self
            .api
            .send_message(to_star_id, content, content_type)
            .await
        {
            Ok(resp) => {
                // 3a. 成功: 用服务端数据更新本地记录
                self.db
                    .update_message_after_send(
                        &local_id,
                        resp.id,
                        &resp.conversation_id,
                        resp.created_at.as_deref(),
                    )
                    .map_err(|e| HasnError::Db(e.to_string()))?;

                // 更新会话
                let preview = if content.len() > 200 {
                    &content[..200]
                } else {
                    content
                };
                let _ = self.db.update_conversation_last_message(
                    &resp.conversation_id,
                    preview,
                    resp.created_at.as_deref().unwrap_or(""),
                );

                let mut result = local_msg;
                result.id = resp.id;
                result.conversation_id = resp.conversation_id;
                result.send_status = SendStatus::Sent;
                result.created_at = resp.created_at;
                Ok(result)
            }
            Err(e) => {
                // 3b. 失败: 标记
                self.db
                    .mark_message_failed(&local_id)
                    .map_err(|e| HasnError::Db(e.to_string()))?;

                let mut result = local_msg;
                result.send_status = SendStatus::Failed;
                error!("[Sync] 发送消息失败: {}", e);
                Err(e)
            }
        }
    }

    /// 处理WS收到的新消息
    pub fn handle_incoming_message(
        &self,
        payload: WsMessagePayload,
    ) -> Result<HasnMessage, HasnError> {
        let msg = payload.into_hasn_message();

        // 写入本地DB
        self.db
            .upsert_message(&msg)
            .map_err(|e| HasnError::Db(e.to_string()))?;

        // 更新会话
        let preview = if msg.content.len() > 200 {
            &msg.content[..200]
        } else {
            &msg.content
        };
        let _ = self.db.update_conversation_last_message(
            &msg.conversation_id,
            preview,
            msg.created_at.as_deref().unwrap_or(""),
        );

        // 增加未读
        let _ = self.db.increment_unread(&msg.conversation_id);

        // 更新同步游标
        if msg.id > 0 {
            let _ = self.db.update_sync_cursor(&msg.conversation_id, msg.id);
        }

        Ok(msg)
    }

    /// 处理 ACK 回执
    pub fn handle_ack(
        &self,
        msg_id: i64,
        conversation_id: &str,
        local_id: Option<&str>,
    ) -> Result<(), HasnError> {
        if let Some(lid) = local_id {
            self.db
                .update_message_after_send(lid, msg_id, conversation_id, None)
                .map_err(|e| HasnError::Db(e.to_string()))?;
        }
        Ok(())
    }
}
