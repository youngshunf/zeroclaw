//! HASN 云端-本地增量同步器
//!
//! 同步方向:
//!   Server → Local (下行同步): 联系人、会话列表
//!   Local → Server (上行同步): 消息发送通过 hasn.message.send 实时上行
//!
//! 对齐设计文档第八节

use zeroclaw_gateway::AppState;
use crate::hasn_chat_db::{ContactRecord, HasnChatDb};
use crate::hasn_router::MessageRouter;
use hasn_client_core::api::HasnApiClient;
use std::sync::Arc;

/// 云端同步器
pub struct HasnSyncer {
    app_state: Arc<AppState>,
}

impl HasnSyncer {
    pub fn new(app_state: Arc<AppState>) -> Self {
        Self { app_state }
    }

    /// 执行全量同步（启动时/重连时调用）
    pub async fn initial_sync(&self, hasn_id: &str) {
        tracing::info!("[HasnSync] Starting initial sync for {}", hasn_id);

        // Resolve local chat DB
        let router = MessageRouter::new_for_api(self.app_state.clone());
        let chat_db = match router.resolve_user_chat_db(hasn_id).await {
            Ok(db) => db,
            Err(e) => {
                tracing::error!("[HasnSync] Failed to resolve chat db: {}", e);
                return;
            }
        };

        // Get the cloud API client
        let api_client = match self.create_api_client().await {
            Some(c) => c,
            None => {
                tracing::warn!("[HasnSync] No API client available, skipping sync");
                return;
            }
        };

        // 1. Sync contacts
        if let Err(e) = self.sync_contacts(&api_client, &chat_db).await {
            tracing::error!("[HasnSync] Contact sync failed: {}", e);
        }

        // 2. Sync conversations
        if let Err(e) = self.sync_conversations(&api_client, &chat_db).await {
            tracing::error!("[HasnSync] Conversation sync failed: {}", e);
        }

        tracing::info!("[HasnSync] Initial sync complete for {}", hasn_id);
    }

    /// 同步联系人 (cloud → local)
    async fn sync_contacts(
        &self,
        api_client: &HasnApiClient,
        chat_db: &HasnChatDb,
    ) -> anyhow::Result<()> {
        tracing::debug!("[HasnSync] Syncing contacts...");

        let contacts = api_client.list_contacts("social").await?;

        for c in &contacts {
            let record = ContactRecord {
                hasn_id: c.peer_hasn_id.clone(),
                nickname: c.nickname.clone().or_else(|| Some(c.peer_name.clone())),
                avatar_url: c.peer_avatar_url.clone(),
                contact_type: c.peer_type.clone(),
                relation_type: c.relation_type.clone(),
                trust_level: c.trust_level,
                status: c.status.clone(),
                created_at: c
                    .connected_at
                    .clone()
                    .unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()),
            };
            chat_db.upsert_contact(&record).await?;
        }

        chat_db
            .set_sync_state(
                "contacts_last_sync",
                &chrono::Utc::now().to_rfc3339(),
            )
            .await?;

        tracing::info!(
            "[HasnSync] Synced {} contacts to local DB",
            contacts.len()
        );
        Ok(())
    }

    /// 同步会话列表 (cloud → local)
    async fn sync_conversations(
        &self,
        api_client: &HasnApiClient,
        chat_db: &HasnChatDb,
    ) -> anyhow::Result<()> {
        tracing::debug!("[HasnSync] Syncing conversations...");

        let conversations = api_client.list_conversations(100, 0).await?;

        for conv in &conversations {
            let peer_id = conv.peer_hasn_id.clone().unwrap_or_default();
            if peer_id.is_empty() {
                continue;
            }
            chat_db
                .upsert_session(&conv.id, &conv.conv_type, &peer_id)
                .await?;
        }

        chat_db
            .set_sync_state(
                "conversations_last_sync",
                &chrono::Utc::now().to_rfc3339(),
            )
            .await?;

        tracing::info!(
            "[HasnSync] Synced {} conversations to local DB",
            conversations.len()
        );
        Ok(())
    }

    /// 从配置创建 HASN API 客户端
    async fn create_api_client(&self) -> Option<HasnApiClient> {
        let config = self.app_state.config.lock().clone();

        // Use hasn_base_url or fall back to api_base_url
        let base_url = config
            .huanxing
            .hasn_base_url
            .as_deref()
            .or(config.huanxing.api_base_url.as_deref())?;

        if base_url.is_empty() {
            return None;
        }

        let client = HasnApiClient::new(base_url);

        // Try to locate a user's access_token from TenantDb for API auth
        let config_dir = config
            .config_path
            .parent()
            .unwrap_or(&config.workspace_dir)
            .to_path_buf();
        let db_path = config.huanxing.resolve_db_path(&config_dir);
        if let Ok(tenant_db) = crate::db::TenantDb::open(&db_path) {
            if let Ok(Some(tenant_dir)) = tenant_db.get_first_tenant_dir().await {
                // Find a user that has this tenant_dir
                if let Ok(Some(tenant)) = tenant_db.get_user(&tenant_dir).await {
                    if let Some(ref token) = tenant.access_token {
                        client.set_hasn_token(token).await;
                    }
                }
            }
        }

        Some(client)
    }
}

/// 启动后台定期同步任务
pub fn spawn_periodic_sync(app_state: Arc<AppState>, hasn_id: String) {
    tokio::spawn(async move {
        let syncer = HasnSyncer::new(app_state);

        // 初始同步 (延迟 3 秒让连接稳定)
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        syncer.initial_sync(&hasn_id).await;

        // 每 5 分钟增量同步
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
        loop {
            interval.tick().await;
            tracing::debug!("[HasnSync] Running periodic sync...");
            syncer.initial_sync(&hasn_id).await;
        }
    });
}
