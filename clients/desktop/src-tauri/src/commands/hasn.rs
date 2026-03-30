//! HASN IM 命令 — 消息/会话/联系人
//!
//! 接入 hasn-client-core，实现完整的 HASN 社交通信功能。
//! 连接生命周期由 Tauri 管理，前端只读状态。
//! 对齐 29/30 设计文档的客户端连接架构。

use std::collections::HashSet;
use std::sync::Arc;
use std::path::PathBuf;

use hasn_client_core::{
    AgentReport, HasnApiClient, HasnWsClient, SyncEngine, WsCommand,
};
use hasn_client_core::ws::WsStatus;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::RwLock;

use crate::commands::models::{HasnClientInfo, Conversation, Message, Contact};
use crate::utils::device::build_device_info;
use crate::services::hasn_ws_router::handle_ws_event;

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
    /// Sidecar 端口（用于 HASN 消息转发到本地 Agent）
    pub sidecar_port: RwLock<Option<u16>>,
    /// 全局 HTTP 客户端（共享连接池优化）
    pub http_client: reqwest::Client,
}

impl HasnClientState {
    pub fn new(api_base_url: &str, db_path: &str) -> Result<Self, String> {
        let api = Arc::new(HasnApiClient::new(api_base_url));
        let db = Arc::new(
            hasn_client_core::Database::new(db_path).map_err(|e: rusqlite::Error| e.to_string())?,
        );
        let sync_engine = Arc::new(SyncEngine::new(api.clone(), db.clone()));
        let ws = Arc::new(HasnWsClient::new());

        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|e| format!("创建 HTTP 客户端池失败: {e}"))?;

        Ok(Self {
            api,
            ws,
            sync_engine,
            db,
            user_hasn_id: RwLock::new(None),
            client_id: RwLock::new(None),
            client_jwt: RwLock::new(None),
            local_agents: RwLock::new(HashSet::new()),
            sidecar_port: RwLock::new(None),
            http_client,
        })
    }
}

// ─── client.json 持久化（只存标识符，不存 token）───

fn client_info_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".huanxing")
        .join("hasn")
        .join("client.json")
}

pub fn load_client_info() -> Option<HasnClientInfo> {
    let path = client_info_path();
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

fn save_client_info(info: &HasnClientInfo) {
    let path = client_info_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    if let Ok(json) = serde_json::to_string_pretty(info) {
        if let Err(e) = std::fs::write(&path, json) {
            tracing::warn!("[HASN] 保存 client.json 失败: {e}");
        }
    }
}

fn delete_client_info() {
    let path = client_info_path();
    std::fs::remove_file(&path).ok();
}

/// 读取当前 HASN 客户端 ID（供前端绑定 Agent 时使用）
#[tauri::command]
pub async fn hasn_get_client_id(
    state: State<'_, Arc<HasnClientState>>,
) -> Result<Option<String>, String> {
    // 优先从运行时 state 读
    let cid = state.client_id.read().await.clone();
    if cid.is_some() {
        return Ok(cid);
    }
    // 回退到 client.json
    Ok(load_client_info().map(|info| info.client_id))
}

// ─── HASN 连接核心逻辑 ───

/// 连接核心逻辑（被 hasn_connect IPC 和 auto_connect 共用）
pub async fn do_hasn_connect(
    state: &Arc<HasnClientState>,
    app: &AppHandle,
    platform_token: &str,
    hasn_id: &str,
    star_id: &str,
) -> Result<String, String> {
    // 1. 设置 API token
    state.api.set_platform_token(platform_token).await;
    state.api.set_hasn_token(platform_token).await;
    state.sync_engine.set_current_user(hasn_id).await;
    *state.user_hasn_id.write().await = Some(hasn_id.to_string());

    // 2. 复用已有 client_id（内存 → 磁盘 → 新注册）
    let client_id = {
        // 2a: 内存中已有（同次运行多次连接）
        let existing = state.client_id.read().await.clone();
        if let Some(cid) = existing {
            cid
        }
        // 2b: 磁盘 client.json 中有（重启恢复）
        else if let Some(info) = load_client_info() {
            if !info.client_id.is_empty() {
                tracing::info!("[HASN] 从 client.json 恢复 client_id: {}", info.client_id);
                *state.client_id.write().await = Some(info.client_id.clone());
                info.client_id
            } else {
                // client_id 为空，重新注册
                let resp = state
                    .api
                    .register_client("desktop", Some("唤星桌面端"), Some(build_device_info()))
                    .await
                    .map_err(|e| format!("注册客户端失败: {}", e))?;
                *state.client_id.write().await = Some(resp.client_id.clone());
                resp.client_id
            }
        }
        // 2c: 首次注册
        else {
            let resp = state
                .api
                .register_client("desktop", Some("唤星桌面端"), Some(build_device_info()))
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
    let hasn_state = state.clone();

    state
        .ws
        .connect(&ws_url, move |event| {
            handle_ws_event(event, &app_handle, &hasn_state);
        })
        .await
        .map_err(|e| format!("WebSocket 连接失败: {}", e))?;

    // 6. 上报本地 Agent
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

    // 7. 持久化客户端标识（无 token）
    save_client_info(&HasnClientInfo {
        hasn_id: hasn_id.to_string(),
        star_id: star_id.to_string(),
        client_id: client_id.clone(),
        name: String::new(),
    });

    eprintln!("[HASN] 连接成功: hasn_id={}, client_id={}", hasn_id, client_id);
    Ok(client_id)
}

// ─── HASN 连接管理 (IPC Commands) ───

/// 初始化 HASN 连接（前端登录后调用）
#[tauri::command]
pub async fn hasn_connect(
    platform_token: String,
    hasn_id: String,
    star_id: String,
    state: State<'_, Arc<HasnClientState>>,
    app: AppHandle,
) -> Result<serde_json::Value, String> {
    let client_id = do_hasn_connect(
        state.inner(), &app, &platform_token, &hasn_id, &star_id,
    ).await?;

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
    delete_client_info();
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

// ─── 自动连接（setup hook 调用）───

/// App 启动时自动连接 HASN
pub async fn hasn_auto_connect(state: Arc<HasnClientState>, app: AppHandle) {
    let info = match load_client_info() {
        Some(info) => info,
        None => {
            eprintln!("[HASN] 无 client.json，跳过自动连接");
            return;
        }
    };

    eprintln!("[HASN] 发现 client.json, hasn_id={}, 请求前端提供 token...", info.hasn_id);

    *state.client_id.write().await = Some(info.client_id.clone());

    let _ = app.emit("hasn:request-token", serde_json::json!({
        "hasn_id": info.hasn_id,
        "star_id": info.star_id,
    }));

    eprintln!("[HASN] 等待前端提供 token（将通过 hasn_provide_token 命令）");
}

/// 前端响应 hasn:request-token 事件，提供 platform_token
#[tauri::command]
pub async fn hasn_provide_token(
    platform_token: String,
    state: State<'_, Arc<HasnClientState>>,
    app: AppHandle,
) -> Result<serde_json::Value, String> {
    let info = load_client_info()
        .ok_or("client.json 不存在")?;

    eprintln!("[HASN] 收到前端 token，开始自动连接...");

    let client_id = do_hasn_connect(
        state.inner(), &app, &platform_token, &info.hasn_id, &info.star_id,
    ).await?;

    Ok(serde_json::json!({
        "connected": true,
        "client_id": client_id,
        "hasn_id": info.hasn_id,
    }))
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

    let local_msgs = state
        .db
        .get_messages(&conversation_id, before_id, limit)
        .map_err(|e| format!("读取消息失败: {}", e))?;

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
    reply_to_id: Option<i64>,
    state: State<'_, Arc<HasnClientState>>,
) -> Result<Message, String> {
    let hasn_id = state
        .user_hasn_id
        .read()
        .await
        .clone()
        .ok_or("未连接 HASN")?;

    let is_local_agent = state.local_agents.read().await.contains(&to);

    if is_local_agent {
        return Err("本地 Agent 对话待实现".into());
    }

    let local_msg =
        hasn_client_core::model::message::HasnMessage::new_outgoing("pending", &hasn_id, &content, 1, reply_to_id);
    let local_id_copy = local_msg.local_id.clone();
    let _ = state.db.insert_message(&local_msg);

    let content_json = serde_json::json!({"text": &content});
    let cmd = WsCommand::Send {
        from_id: Some(hasn_id.clone()),
        to: to.clone(),
        content: content_json,
        content_type: Some(1),
        msg_type: Some("message".to_string()),
        local_id: Some(local_id_copy.clone()),
        reply_to_id,
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
    let _ = state.db.clear_unread(&conversation_id);
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

    let contacts = state
        .api
        .list_contacts(rt)
        .await
        .map_err(|e| format!("获取联系人失败: {}", e))?;

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

// ─── Sidecar 端口设置 ───

#[tauri::command]
pub async fn set_hasn_sidecar_port(
    port: u16,
    state: State<'_, Arc<HasnClientState>>,
) -> Result<(), String> {
    *state.sidecar_port.write().await = Some(port);
    tracing::info!("[HASN] Sidecar 端口设置为 {}", port);
    Ok(())
}

// ─── 托盘状态管理 ───

#[tauri::command]
pub async fn update_tray_badge(
    count: u32,
    app: AppHandle,
) -> Result<(), String> {
    if let Some(tray) = app.tray_by_id("main") {
        if count > 0 {
            #[cfg(target_os = "macos")]
            let _ = tray.set_title(Some(format!(" {}", count)));
            
            let _ = tray.set_tooltip(Some(format!("唤星 AI ({}条未读消息)", count)));
        } else {
            #[cfg(target_os = "macos")]
            let _ = tray.set_title(None::<String>);
            
            let _ = tray.set_tooltip(Some("唤星 AI"));
        }
    }
    Ok(())
}
