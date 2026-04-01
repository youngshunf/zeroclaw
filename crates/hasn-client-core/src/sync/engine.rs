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

    /// 发送消息 (本地优先, v4.0 Envelope 模式)
    pub async fn send_message(
        &self,
        to_hasn_id: &str,
        to_owner_id: &str,
        content: &str,
        _content_type: &str,
    ) -> Result<HasnEnvelope, HasnError> {
        let hasn_id = self
            .current_hasn_id()
            .await
            .ok_or_else(|| HasnError::Auth("未登录".to_string()))?;

        // 1. 构造本地 Envelope (状态=sending)
        let envelope = HasnEnvelope::new_text(
            EntityRef {
                hasn_id: hasn_id.clone(),
                entity_type: EntityType::Human,
                owner_id: hasn_id.clone(),
            },
            EntityRef {
                hasn_id: to_hasn_id.to_string(),
                entity_type: EntityType::Human, // 外部调用者可以后续覆盖
                owner_id: to_owner_id.to_string(),
            },
            "pending",
            content,
        );

        let msg_id = envelope.id.clone();

        // 2. 写入本地 DB
        let mut record = HasnMessageRecord::from_envelope(&envelope);
        record.send_status = SendStatus::Sending;
        self.db
            .insert_message(&record)
            .map_err(|e| HasnError::Db(e.to_string()))?;

        // 3. 调 API 发送
        // TODO: API 层需要升级为接受 HasnEnvelope 参数
        match self
            .api
            .send_message(to_hasn_id, content, 1) // 临时兼容旧 API
            .await
        {
            Ok(resp) => {
                // 3a. 成功: 更新本地记录
                self.db
                    .update_message_after_send(
                        &msg_id,
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

                let mut result = envelope;
                result.context.conversation_id = resp.conversation_id;
                Ok(result)
            }
            Err(e) => {
                // 3b. 失败: 标记
                self.db
                    .mark_message_failed(&msg_id)
                    .map_err(|e| HasnError::Db(e.to_string()))?;

                error!("[Sync] 发送消息失败: {}", e);
                Err(e)
            }
        }
    }

    /// 处理 WS 收到的新消息 (桥接传输层 → v4.0 Envelope)
    pub fn handle_incoming_message(
        &self,
        payload: WsMessagePayload,
    ) -> Result<HasnEnvelope, HasnError> {
        let envelope = payload.into_envelope();

        // 写入本地 DB
        let record = HasnMessageRecord::from_envelope(&envelope);
        self.db
            .upsert_message(&record)
            .map_err(|e| HasnError::Db(e.to_string()))?;

        // 更新会话
        let preview_text = envelope.text_content();
        let preview = if preview_text.len() > 200 {
            &preview_text[..200]
        } else {
            &preview_text
        };
        let _ = self.db.update_conversation_last_message(
            &envelope.context.conversation_id,
            preview,
            &envelope.metadata.created_at,
        );

        // 增加未读
        let _ = self.db.increment_unread(&envelope.context.conversation_id);

        Ok(envelope)
    }

    /// 处理 ACK 回执
    pub fn handle_ack(
        &self,
        msg_id: &str,
        conversation_id: &str,
    ) -> Result<(), HasnError> {
        self.db
            .update_message_after_send(msg_id, conversation_id, None)
            .map_err(|e| HasnError::Db(e.to_string()))?;
        Ok(())
    }
}
