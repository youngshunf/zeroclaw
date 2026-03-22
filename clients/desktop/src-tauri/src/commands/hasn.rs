//! HASN IM 命令 — 消息/会话/联系人
//!
//! 接入 hasn-client-core，实现完整的 HASN 社交通信功能。
//! 对齐 29/30 设计文档的客户端连接架构。

use std::collections::HashSet;
use std::sync::Arc;

use hasn_client_core::{
    AgentReport, HasnApiClient, HasnWsClient, SyncEngine,
    WsCommand, WsEvent, WsMessagePayload,
};
use hasn_client_core::ws::WsStatus;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::RwLock;

/// HASN 客户端状态（对齐 29 文档 §6.2 HasnClientState）
pub struct HasnClientState {
    /// API 客户端
    pub api: Arc<HasnApiClient>,
    /// WebSocket 客户端
    pub ws: Arc<HasnWsClient>,
    /// 同步引擎
    pub sync_engine: Arc<SyncEngine>,
    /// 本地数据库
    pub db: Arc<hasn_client_core::Database>,
    /// 用户 hasn_id
    pub user_hasn_id: RwLock<Option<String>>,
    /// 客户端 ID
    pub client_id: RwLock<Option<String>>,
    /// Client JWT
    pub client_jwt: RwLock<Option<String>>,
    /// 当前客户端管理的 Agent hasn_id 集合
    pub local_agents: RwLock<HashSet<String>>,
}

impl HasnClientState {
    pub fn new(api_base_url: &str, db_path: &str) -> Result<Self, String> {
        let api = Arc::new(HasnApiClient::new(api_base_url));
        let db = Arc::new(
            hasn_client_core::Database::new(db_path).map_err(|e: rusqlite::Error| e.to_string())?,
        );
        let sync_engine = Arc::new(SyncEngine::new(api.clone(), db.clone()));
        let ws = Arc::new(HasnWsClient::new());

        Ok(Self {
            api,
            ws,
            sync_engine,
            db,
            user_hasn_id: RwLock::new(None),
            client_id: RwLock::new(None),
            client_jwt: RwLock::new(None),
            local_agents: RwLock::new(HashSet::new()),
        })
    }
}

// ─── 响应类型 ───

#[derive(Debug, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub peer_id: String,
    pub peer_name: String,
    pub peer_type: String,
    pub last_message: Option<String>,
    pub last_message_at: Option<String>,
    pub unread_count: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    pub id: i64,
    pub local_id: String,
    pub conversation_id: String,
    pub from_id: String,
    pub from_type: i32,
    pub content: String,
    pub content_type: i32,
    pub status: i32,
    pub send_status: String,
    pub created_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Contact {
    pub hasn_id: String,
    pub star_id: String,
    pub name: String,
    pub peer_type: String,
    pub relation_type: String,
    pub trust_level: i32,
    pub status: String,
}

// ─── HASN 连接管理 ───

/// 初始化 HASN 连接（登录后调用）
#[tauri::command]
pub async fn hasn_connect(
    platform_token: String,
    hasn_id: String,
    _star_id: String,
    state: State<'_, Arc<HasnClientState>>,
    app: AppHandle,
) -> Result<serde_json::Value, String> {
    // 1. 设置 API token
    state.api.set_platform_token(&platform_token).await;
    state.api.set_hasn_token(&platform_token).await;
    state.sync_engine.set_current_user(&hasn_id).await;
    *state.user_hasn_id.write().await = Some(hasn_id.clone());

    // 2. 注册客户端（如果还没有 client_id）
    let client_id = {
        let existing = state.client_id.read().await.clone();
        if let Some(cid) = existing {
            cid
        } else {
            let resp = state
                .api
                .register_client("desktop", Some("唤星桌面端"))
                .await
                .map_err(|e| format!("注册客户端失败: {}", e))?;
            *state.client_id.write().await = Some(resp.client_id.clone());
            resp.client_id
        }
    };

    // 3. 获取 Client JWT
    let token_resp = state
        .api
        .get_client_token(&client_id)
        .await
        .map_err(|e| format!("获取 Client JWT 失败: {}", e))?;
    *state.client_jwt.write().await = Some(token_resp.client_jwt.clone());

    // 4. 全量同步
    if let Err(e) = state.sync_engine.full_sync().await {
        tracing::warn!("全量同步失败（非致命）: {}", e);
    }

    // 5. 建立 WebSocket 连接
    let ws_url = state.api.ws_client_url(&token_resp.client_jwt);
    let app_handle = app.clone();
    let sync = state.sync_engine.clone();

    state
        .ws
        .connect(&ws_url, move |event| {
            handle_ws_event(event, &app_handle, &sync);
        })
        .await
        .map_err(|e| format!("WebSocket 连接失败: {}", e))?;

    // 6. 上报本地 Agent（获取本地 Agent 列表）
    let agents = state
        .api
        .list_my_agents()
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|a| a.agent_type == "local")
        .map(|a| AgentReport {
            hasn_id: a.hasn_id.clone(),
        })
        .collect::<Vec<_>>();

    if !agents.is_empty() {
        let cmd = WsCommand::ReportAgents { agents };
        let _ = state.ws.send_command(&cmd).await;
    }

    Ok(serde_json::json!({
        "connected": true,
        "client_id": client_id,
        "hasn_id": hasn_id,
    }))
}

/// 断开 HASN 连接
#[tauri::command]
pub async fn hasn_disconnect(state: State<'_, Arc<HasnClientState>>) -> Result<(), String> {
    state.ws.disconnect().await;
    *state.user_hasn_id.write().await = None;
    *state.client_jwt.write().await = None;
    state.local_agents.write().await.clear();
    Ok(())
}

/// 获取 HASN 连接状态
#[tauri::command]
pub async fn hasn_status(state: State<'_, Arc<HasnClientState>>) -> Result<String, String> {
    let status = state.ws.status().await;
    Ok(match status {
        WsStatus::Connected => "connected".to_string(),
        WsStatus::Connecting => "connecting".to_string(),
        WsStatus::Disconnected => "disconnected".to_string(),
        WsStatus::Reconnecting { attempt } => format!("reconnecting:{}", attempt),
    })
}

// ─── 会话 ───

#[tauri::command]
pub async fn get_conversations(
    state: State<'_, Arc<HasnClientState>>,
) -> Result<Vec<Conversation>, String> {
    let convs = state
        .db
        .list_conversations()
        .map_err(|e| format!("读取会话失败: {}", e))?;

    Ok(convs
        .into_iter()
        .map(|c| Conversation {
            id: c.id.clone(),
            peer_id: c.peer_hasn_id.unwrap_or_default(),
            peer_name: c.peer_name.unwrap_or_default(),
            peer_type: c.peer_type.unwrap_or_else(|| "human".to_string()),
            last_message: c.last_message_preview,
            last_message_at: c.last_message_at,
            unread_count: c.unread_count,
        })
        .collect())
}

// ─── 消息 ───

#[tauri::command]
pub async fn get_messages(
    conversation_id: String,
    before_id: Option<i64>,
    limit: Option<i32>,
    state: State<'_, Arc<HasnClientState>>,
) -> Result<Vec<Message>, String> {
    let limit = limit.unwrap_or(50);

    // 先从本地 DB 读
    let local_msgs = state
        .db
        .get_messages(&conversation_id, before_id, limit)
        .map_err(|e| format!("读取消息失败: {}", e))?;

    // 如果本地不够，从服务端拉取
    if local_msgs.len() < limit as usize {
        if let Ok(remote_msgs) = state
            .api
            .get_messages(&conversation_id, before_id, limit)
            .await
        {
            for msg in &remote_msgs {
                let _ = state.db.upsert_message(msg);
            }
        }
    }

    // 重新从本地读取（包含刚拉取的）
    let msgs = state
        .db
        .get_messages(&conversation_id, before_id, limit)
        .map_err(|e| format!("读取消息失败: {}", e))?;

    Ok(msgs
        .into_iter()
        .map(|m| Message {
            id: m.id,
            local_id: m.local_id,
            conversation_id: m.conversation_id,
            from_id: m.from_id,
            from_type: m.from_type,
            content: m.content,
            content_type: m.content_type,
            status: m.status,
            send_status: m.send_status.as_str().to_string(),
            created_at: m.created_at,
        })
        .collect())
}

#[tauri::command]
pub async fn send_message(
    to: String,
    content: String,
    state: State<'_, Arc<HasnClientState>>,
) -> Result<Message, String> {
    let hasn_id = state
        .user_hasn_id
        .read()
        .await
        .clone()
        .ok_or("未连接 HASN")?;

    // 检查是否为本地 Agent 对话
    let is_local_agent = state.local_agents.read().await.contains(&to);

    if is_local_agent {
        // 本地对话：直接 IPC 调 Sidecar（TODO: Phase 2.3 实现）
        return Err("本地 Agent 对话待实现".into());
    }

    // 本地优先：先写入本地 DB
    let local_msg =
        hasn_client_core::model::message::HasnMessage::new_outgoing("pending", &hasn_id, &content, 1);
    let local_id_copy = local_msg.local_id.clone();
    let _ = state.db.insert_message(&local_msg);

    // 远程路由：通过 WS 发送
    let content_json = serde_json::json!({"text": &content});
    let cmd = WsCommand::Send {
        from_id: Some(hasn_id.clone()),
        to: to.clone(),
        content: content_json,
        content_type: Some(1),
        msg_type: Some("message".to_string()),
        local_id: Some(local_id_copy.clone()),
    };

    state
        .ws
        .send_command(&cmd)
        .await
        .map_err(|e| format!("发送失败: {}", e))?;

    Ok(Message {
        id: 0,
        local_id: local_id_copy,
        conversation_id: "pending".to_string(),
        from_id: hasn_id,
        from_type: 1,
        content,
        content_type: 1,
        status: 1,
        send_status: "sending".to_string(),
        created_at: Some(chrono::Utc::now().to_rfc3339()),
    })
}

#[tauri::command]
pub async fn mark_conversation_read(
    conversation_id: String,
    last_msg_id: Option<i64>,
    state: State<'_, Arc<HasnClientState>>,
) -> Result<(), String> {
    let msg_id = last_msg_id.unwrap_or(0);

    // 本地更新
    let _ = state.db.clear_unread(&conversation_id);

    // 通过 WS 通知服务端
    let cmd = WsCommand::Read {
        conversation_id,
        last_msg_id: msg_id,
    };
    let _ = state.ws.send_command(&cmd).await;

    Ok(())
}

// ─── 联系人 ───

#[tauri::command]
pub async fn get_contacts(
    relation_type: Option<String>,
    state: State<'_, Arc<HasnClientState>>,
) -> Result<Vec<Contact>, String> {
    let rt = relation_type.as_deref().unwrap_or("social");

    // 从 API 拉取最新
    let contacts = state
        .api
        .list_contacts(rt)
        .await
        .map_err(|e| format!("获取联系人失败: {}", e))?;

    // 写入本地 DB
    for c in &contacts {
        let _ = state.db.upsert_contact(c);
    }

    Ok(contacts
        .into_iter()
        .map(|c| Contact {
            hasn_id: c.peer_hasn_id,
            star_id: c.peer_star_id,
            name: c.peer_name,
            peer_type: c.peer_type,
            relation_type: c.relation_type,
            trust_level: c.trust_level,
            status: c.status,
        })
        .collect())
}

#[tauri::command]
pub async fn send_friend_request(
    star_id: String,
    message: Option<String>,
    state: State<'_, Arc<HasnClientState>>,
) -> Result<(), String> {
    state
        .api
        .send_friend_request(&star_id, &message.unwrap_or_default())
        .await
        .map_err(|e| format!("发送好友请求失败: {}", e))
}

#[tauri::command]
pub async fn get_friend_requests(
    state: State<'_, Arc<HasnClientState>>,
) -> Result<Vec<serde_json::Value>, String> {
    let requests = state
        .api
        .list_pending_requests()
        .await
        .map_err(|e| format!("获取好友请求失败: {}", e))?;

    Ok(requests
        .into_iter()
        .map(|r| serde_json::to_value(r).unwrap_or_default())
        .collect())
}

#[tauri::command]
pub async fn respond_friend_request(
    request_id: i64,
    accept: bool,
    state: State<'_, Arc<HasnClientState>>,
) -> Result<(), String> {
    let action = if accept { "accept" } else { "reject" };
    state
        .api
        .respond_friend_request(request_id, action)
        .await
        .map_err(|e| format!("回应好友请求失败: {}", e))
}

// ─── Agent 管理 ───

#[tauri::command]
pub async fn get_my_agents(
    state: State<'_, Arc<HasnClientState>>,
) -> Result<Vec<serde_json::Value>, String> {
    let agents = state
        .api
        .list_my_agents()
        .await
        .map_err(|e| format!("获取 Agent 列表失败: {}", e))?;

    Ok(agents
        .into_iter()
        .map(|a| serde_json::to_value(a).unwrap_or_default())
        .collect())
}

// ─── WebSocket 事件处理（本地路由，对齐 29 文档 §6.1）───

fn handle_ws_event(event: WsEvent, app: &AppHandle, sync: &Arc<SyncEngine>) {
    match event {
        WsEvent::Connected {
            user_hasn_id,
            client_id,
            ..
        } => {
            tracing::info!("[HASN] 已连接: {} ({})", user_hasn_id, client_id);
            let _ = app.emit("hasn:connected", serde_json::json!({
                "user_hasn_id": user_hasn_id,
                "client_id": client_id,
            }));
        }

        WsEvent::ReportAgentsAck { accepted, failed } => {
            tracing::info!(
                "[HASN] Agent 上报: accepted={}, failed={}",
                accepted.len(),
                failed.len()
            );
            let _ = app.emit("hasn:agents_reported", serde_json::json!({
                "accepted": accepted,
                "failed": failed,
            }));
        }

        WsEvent::Message { to_id, message } => {
            if let Ok(msg) = sync.handle_incoming_message(message) {
                let _ = app.emit("hasn:message", serde_json::json!({
                    "id": msg.id,
                    "local_id": msg.local_id,
                    "conversation_id": msg.conversation_id,
                    "from_id": msg.from_id,
                    "from_type": msg.from_type,
                    "content": msg.content,
                    "content_type": msg.content_type,
                    "status": msg.status,
                    "created_at": msg.created_at,
                    "to_id": to_id,
                }));
            }
        }

        WsEvent::OfflineMessages { messages } => {
            tracing::info!("[HASN] 收到 {} 条离线消息", messages.len());
            for raw in messages {
                if let Ok(payload) = serde_json::from_value::<WsMessagePayload>(raw) {
                    if let Ok(msg) = sync.handle_incoming_message(payload) {
                        let _ = app.emit("hasn:message", serde_json::json!({
                            "id": msg.id,
                            "conversation_id": msg.conversation_id,
                            "from_id": msg.from_id,
                            "content": msg.content,
                            "offline": true,
                        }));
                    }
                }
            }
        }

        WsEvent::Ack {
            msg_id,
            conversation_id,
            local_id,
            status,
        } => {
            let _ = sync.handle_ack(msg_id, &conversation_id, local_id.as_deref());
            let _ = app.emit("hasn:ack", serde_json::json!({
                "msg_id": msg_id,
                "conversation_id": conversation_id,
                "local_id": local_id,
                "status": status,
            }));
        }

        WsEvent::Typing {
            from_id,
            conversation_id,
        } => {
            let _ = app.emit("hasn:typing", serde_json::json!({
                "from_id": from_id,
                "conversation_id": conversation_id,
            }));
        }

        WsEvent::Presence { hasn_id, status } => {
            let _ = app.emit("hasn:presence", serde_json::json!({
                "hasn_id": hasn_id,
                "status": status,
            }));
        }

        WsEvent::MessageRecalled {
            msg_id,
            conversation_id,
            recalled_by,
        } => {
            let _ = app.emit("hasn:message_recalled", serde_json::json!({
                "msg_id": msg_id,
                "conversation_id": conversation_id,
                "recalled_by": recalled_by,
            }));
        }

        WsEvent::Pong { .. } => {}

        WsEvent::Error { code, message } => {
            tracing::error!("[HASN] 服务端错误: code={} msg={}", code, message);
            let _ = app.emit("hasn:error", serde_json::json!({
                "code": code,
                "message": message,
            }));
        }

        _ => {}
    }
}
