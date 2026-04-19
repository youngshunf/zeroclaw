//! HASN 消息路由器
//!
//! 职责:
//! 1. 从 hasn_connector 接收所有入站消息
//! 2. 解析 to_id 确定目标
//! 3. 将消息写入本地 hasn_chat.db
//! 4. 根据 to_id 类型分发: Human → UI 推送; Agent → Agent 运行时

use zeroclaw_gateway::AppState;
use crate::db::TenantDb;
use crate::hasn_chat_db::{ChatMessageRecord, HasnChatDb};
use crate::hasn_agent_bridge::HasnAgentBridge;
use crate::hasn_connector::HasnAgentSession;
use anyhow::Result;
use hasn_client_core::model::WsMessagePayload;
use hasn_client_core::ws::HasnWsClient;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct MessageRouter {
    app_state: Arc<AppState>,
    ws: Option<Arc<HasnWsClient>>,
    sessions: Option<Arc<RwLock<HashMap<String, Arc<HasnAgentSession>>>>>,
}

impl MessageRouter {
    /// Full constructor for use from hasn_connector (WS context available)
    pub fn new(
        app_state: Arc<AppState>,
        ws: Arc<HasnWsClient>,
        sessions: Arc<RwLock<HashMap<String, Arc<HasnAgentSession>>>>,
    ) -> Self {
        Self {
            app_state,
            ws: Some(ws),
            sessions: Some(sessions),
        }
    }

    /// Lightweight constructor for REST API handlers (no WS)
    pub fn new_for_api(app_state: Arc<AppState>) -> Self {
        Self {
            app_state,
            ws: None,
            sessions: None,
        }
    }

    /// Resolve the per-user `hasn_chat.db` path from a hasn_id.
    ///
    /// Works for both h_xxx (human) and a_xxx (agent) hasn_ids.
    /// For agents, `TenantDb::find_by_hasn_id` resolves through the agents JOIN users query.
    pub async fn resolve_user_chat_db(&self, hasn_id: &str) -> Result<HasnChatDb> {
        let config = self.app_state.config.lock().clone();
        let config_dir = config
            .config_path
            .parent()
            .unwrap_or(&config.workspace_dir)
            .to_path_buf();
        let global_db_path = config.huanxing.resolve_db_path(&config_dir);

        let db = TenantDb::open(&global_db_path)?;

        let tenant_dir = if hasn_id.starts_with("h_") {
            db.get_tenant_dir_by_user_hasn_id(hasn_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Unknown human hasn_id: {}", hasn_id))?
        } else {
            match db.find_by_hasn_id(hasn_id).await? {
                Some(tenant) => tenant
                    .tenant_dir
                    .ok_or_else(|| anyhow::anyhow!("Tenant has no tenant_dir for hasn_id={}", hasn_id))?,
                None => return Err(anyhow::anyhow!("Unknown agent hasn_id: {}", hasn_id)),
            }
        };

        let tenant_root = config
            .huanxing
            .resolve_tenant_root(&config_dir, Some(&tenant_dir));
        let chat_db_path = tenant_root.join("data").join("hasn_chat.db");

        HasnChatDb::open(&chat_db_path)
    }

    /// Dispatch an incoming HASN message (WsMessagePayload from hasn_connector)
    pub async fn dispatch(&self, message: WsMessagePayload) -> Result<()> {
        let target_id = message.to_id.clone().unwrap_or_default();
        if target_id.is_empty() {
            return Err(anyhow::anyhow!("Message has no to_id"));
        }

        // 1. Resolve chat DB by target hasn_id
        let chat_db = self.resolve_user_chat_db(&target_id).await?;

        // 2. Convert content_type int to string label
        let content_type_str = match message.content_type {
            1 => "text",
            2 => "image",
            3 => "file",
            4 => "voice",
            5 => "card",
            6 => "tool_call",
            _ => "text",
        };

        // 3. Generate a stable message_id string
        let msg_id_str = match &message.id {
            serde_json::Value::Number(n) => format!("msg_{}", n),
            serde_json::Value::String(s) => s.clone(),
            _ => format!("msg_{}", uuid::Uuid::new_v4()),
        };

        // 4. Save into persistent IM db
        let record = ChatMessageRecord {
            id: 0,
            message_id: msg_id_str,
            conversation_id: message.conversation_id.clone(),
            sender_id: message.from_id.clone(),
            receiver_id: target_id.clone(),
            content_type: content_type_str.to_string(),
            content: serde_json::to_string(&message.content).unwrap_or_default(),
            status: "delivered".to_string(),
            is_outgoing: message.self_sent.unwrap_or(false),
            created_at: message
                .created_time
                .clone()
                .unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()),
        };

        chat_db.insert_message(&record).await?;

        // 5. Ensure session exists
        let session_type = if message.conversation_id.starts_with("g_") {
            "group"
        } else {
            "p2p"
        };
        chat_db
            .upsert_session(&message.conversation_id, session_type, &message.from_id)
            .await?;

        // 6. Route according to agent/human rules
        if target_id.starts_with("a_") {
            tracing::info!(
                "[MessageRouter] Routing message to Agent {}",
                target_id
            );
            if let (Some(ws), Some(sessions)) = (&self.ws, &self.sessions) {
                let bridge = HasnAgentBridge::new(
                    self.app_state.config.lock().clone(),
                    self.app_state.session_backend.clone(),
                    chat_db,
                    sessions.clone(),
                );
                bridge
                    .inject_and_stream(&target_id, message, ws.clone())
                    .await;
            } else {
                tracing::error!(
                    "[MessageRouter] Cannot inject to agent — missing WS/Sessions context"
                );
            }
        } else {
            // Human target — frontend picks it up via /ws/hasn-events broadcast
            tracing::info!(
                "[MessageRouter] Routing message to Human {}",
                target_id
            );
        }

        Ok(())
    }
}
