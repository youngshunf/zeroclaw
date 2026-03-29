//! HASN IM 命令 — 消息/会话/联系人
//!
//! 接入 hasn-client-core，实现完整的 HASN 社交通信功能。
//! 连接生命周期由 Tauri 管理，前端只读状态。
//! 对齐 29/30 设计文档的客户端连接架构。

use std::collections::HashSet;
use std::sync::Arc;
use std::path::PathBuf;

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
    /// Sidecar 端口（用于 HASN 消息转发到本地 Agent）
    pub sidecar_port: RwLock<Option<u16>>,
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
            sidecar_port: RwLock::new(None),
        })
    }
}

// ─── client.json 持久化（只存标识符，不存 token）───

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HasnClientInfo {
    pub hasn_id: String,
    pub star_id: String,
    pub client_id: String,
    pub name: String,
}

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

/// 获取稳定的设备指纹（跨重启不变）
///
/// macOS: 通过 ioreg 获取 IOPlatformUUID
/// Linux: 读 /etc/machine-id
/// 兜底: hostname 的哈希
fn get_device_fingerprint() -> String {
    // macOS: ioreg -rd1 -c IOPlatformExpertDevice | grep IOPlatformUUID
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("ioreg")
            .args(["-rd1", "-c", "IOPlatformExpertDevice"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.contains("IOPlatformUUID") {
                    if let Some(uuid) = line.split('"').nth(3) {
                        return uuid.to_string();
                    }
                }
            }
        }
    }

    // Linux: /etc/machine-id
    #[cfg(target_os = "linux")]
    {
        if let Ok(mid) = std::fs::read_to_string("/etc/machine-id") {
            let trimmed = mid.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }

    // Fallback: hostname hash
    let hostname = hostname::get()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    hostname.hash(&mut hasher);
    format!("h_{:016x}", hasher.finish())
}

/// 构造 device_info JSON（含设备指纹）
fn build_device_info() -> serde_json::Value {
    let fingerprint = get_device_fingerprint();
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let hostname = hostname::get()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    serde_json::json!({
        "device_fingerprint": fingerprint,
        "os": os,
        "arch": arch,
        "hostname": hostname,
    })
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
/// 1. 读取 client.json 获取 hasn_id
/// 2. 发 event 向前端请求 platform_token
/// 3. 前端响应后执行 do_hasn_connect
pub async fn hasn_auto_connect(state: Arc<HasnClientState>, app: AppHandle) {
    // 检查是否有保存的客户端信息
    let info = match load_client_info() {
        Some(info) => info,
        None => {
            eprintln!("[HASN] 无 client.json，跳过自动连接");
            return;
        }
    };

    eprintln!("[HASN] 发现 client.json, hasn_id={}, 请求前端提供 token...", info.hasn_id);

    // 预设 client_id（避免重复注册）
    *state.client_id.write().await = Some(info.client_id.clone());

    // 发事件给前端请求 platform_token
    let _ = app.emit("hasn:request-token", serde_json::json!({
        "hasn_id": info.hasn_id,
        "star_id": info.star_id,
    }));

    // 监听前端响应（通过全局 OnceChannel）
    // 前端会调用 hasn_provide_token 命令传回 token
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
    reply_to_id: Option<i64>,
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
        hasn_client_core::model::message::HasnMessage::new_outgoing("pending", &hasn_id, &content, 1, reply_to_id);
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

// ─── HASN → 本地 Agent 调用 ───

/// 通过 HTTP 调用本地 Sidecar 的 hasn-invoke 端点
async fn invoke_sidecar_agent(
    port: u16,
    hasn_id: &str,
    session_id: &str,
    from_id: &str,
    message: &str,
) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))?;

    let resp = client
        .post(format!("http://127.0.0.1:{port}/api/v1/agent/hasn-invoke"))
        .json(&serde_json::json!({
            "hasn_id": hasn_id,
            "session_id": session_id,
            "from_id": from_id,
            "message": message,
        }))
        .send()
        .await
        .map_err(|e| format!("Sidecar 调用失败: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Sidecar 返回 {status}: {body}"));
    }

    let result: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("解析 Sidecar 响应失败: {e}"))?;

    result
        .get("reply")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "Sidecar 响应缺少 reply 字段".to_string())
}

// ─── WebSocket 事件处理（本地路由，对齐 29 文档 §6.1）───

fn handle_ws_event(event: WsEvent, app: &AppHandle, state: &Arc<HasnClientState>) {
    let sync = &state.sync_engine;
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
            // 1. 先写入本地 DB
            let msg = match sync.handle_incoming_message(message) {
                Ok(m) => m,
                Err(e) => {
                    tracing::error!("[HASN] 处理入站消息失败: {e}");
                    return;
                }
            };

            // 2. 通知前端显示
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
                "reply_to_id": msg.reply_to,
            }));

            // 3. 发送系统通知（消息来自别人时）
            {
                let content_preview = if msg.content.len() > 50 {
                    format!("{}...", &msg.content[..50])
                } else {
                    msg.content.clone()
                };
                let _ = app.emit("hasn:notification", serde_json::json!({
                    "title": msg.from_id,
                    "body": content_preview,
                }));
            }

            // 3. 路由判断：to_id 是否是本地 Agent？
            if let Some(ref target_id) = to_id {
                let state = state.clone();
                let target = target_id.clone();
                let from = msg.from_id.clone();
                let conv_id = msg.conversation_id.clone();
                let content = msg.content.clone();
                let ws = state.ws.clone();

                tokio::spawn(async move {
                    let is_local = state.local_agents.read().await.contains(&target);
                    if !is_local {
                        return; // 不是本地 Agent，仅显示，不转发
                    }

                    let port = match *state.sidecar_port.read().await {
                        Some(p) => p,
                        None => {
                            tracing::warn!("[HASN] 本地 Agent 消息但 sidecar_port 未设置");
                            return;
                        }
                    };

                    // 发送 TYPING 状态
                    let typing_cmd = WsCommand::Typing {
                        conversation_id: conv_id.clone(),
                        to_id: from.clone(),
                    };
                    let _ = ws.send_command(&typing_cmd).await;

                    // 调用 Sidecar Agent
                    tracing::info!(
                        "[HASN] 转发消息到本地 Agent: {} -> {} (port={})",
                        from, target, port
                    );
                    match invoke_sidecar_agent(
                        port, &target, &conv_id, &from, &content,
                    ).await {
                        Ok(reply) => {
                            // 通过 HASN WS 回复
                            let reply_json = serde_json::json!({"text": &reply});
                            let send_cmd = WsCommand::Send {
                                from_id: Some(target.clone()),
                                to: from.clone(),
                                content: reply_json,
                                content_type: Some(1),
                                msg_type: Some("message".to_string()),
                                local_id: Some(format!("agent_{}", chrono::Utc::now().timestamp_millis())),
                                reply_to_id: None,
                            };
                            if let Err(e) = ws.send_command(&send_cmd).await {
                                tracing::error!("[HASN] Agent 回复发送失败: {e}");
                            } else {
                                tracing::info!("[HASN] Agent 回复已发送 ({} 字符)", reply.len());
                            }
                        }
                        Err(e) => {
                            tracing::error!("[HASN] Sidecar 调用失败: {e}");
                        }
                    }
                });
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
