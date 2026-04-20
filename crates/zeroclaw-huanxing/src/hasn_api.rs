//! HASN HTTP/WS 端点 — 供前端通过 Sidecar 收发 HASN 消息
//!
//! 端点列表:
//! - POST   /api/v1/hasn/connect     连接 HASN 中央节点
//! - POST   /api/v1/hasn/disconnect  断开连接
//! - GET    /api/v1/hasn/status      获取连接状态
//! - POST   /api/v1/hasn/send        发送消息
//! - POST   /api/v1/hasn/node/owners 绑定 Owner
//! - POST   /api/v1/hasn/node/owners/{owner_id}/renew 续期 Owner
//! - DELETE /api/v1/hasn/node/owners/{owner_id} 解绑 Owner
//! - GET    /api/v1/hasn/node/owners 查询已绑定 Owner
//! - POST   /api/v1/hasn/node/agents  上线 Agent
//! - DELETE /api/v1/hasn/node/agents/{agent_id} 下线 Agent
//! - WS     /ws/hasn-events          HASN 事件实时推送

use axum::{
    Json,
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;
use tracing::{error, info};

use zeroclaw_gateway::AppState;

// Phase 05-05 Task 4 — 11 个控制平面端点全部薄转发到 hasn-node 全局 connector。
// legacy `hasn_connector` 现仅供 `hasn_sync.rs` 等非 WS 热路径（Pull 联系人）使用，
// 本文件不再依赖其任何 symbol。

// ─── Request/Response 类型 ───

#[derive(Debug, Deserialize)]
pub struct ConnectRequest {
    /// HASN WS URL（可选，默认从 config 读取）
    pub url: Option<String>,
    /// JWT token 或 API Key（可选，支持动态传入）
    pub token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SendRequest {
    pub to: String,
    pub content: serde_json::Value,
    #[serde(default)]
    pub from_id: Option<String>,
    #[serde(default)]
    pub local_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AgentReportItem {
    pub hasn_id: String,
    #[serde(default)]
    pub owner_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OwnerProofItem {
    #[serde(rename = "type")]
    pub proof_type: String,
    pub credential: String,
}

#[derive(Debug, Deserialize)]
pub struct AddOwnerRequest {
    pub owner_id: String,
    pub owner_proof: OwnerProofItem,
}

#[derive(Debug, Deserialize)]
pub struct AddAgentRequest {
    pub agent_id: String,
    pub owner_id: String,
}

// ─── 端点实现 ───

/// POST /api/v1/hasn/connect
///
/// Phase 05-05 Task 2 — 薄转发到 hasn-node 全局 connector。
/// 桌面端合同保持 Phase 05-02 基线：响应 JSON 字段不带 `data` 外壳；
/// 200 `{status:"connected"}` / 503 `{status:"failed", error}` / 504 `{status:"timeout", error}` /
/// 400 `{error}`。
pub async fn hasn_connect(
    State(state): State<AppState>,
    Json(req): Json<ConnectRequest>,
) -> impl IntoResponse {
    let config = state.config.lock().clone();
    let hasn_config = &config.huanxing.hasn;

    // 构建 WS URL
    let base_url = req
        .url
        .or_else(|| hasn_config.central_url.clone())
        .unwrap_or_else(|| {
            format!(
                "{}/api/v1/hasn/ws/node",
                config
                    .huanxing
                    .hasn_url()
                    .replace("https://", "wss://")
                    .replace("http://", "ws://")
            )
        });

    // v2.1 简化认证：用 Bearer/OwnerKey + X-Node-Id
    let token = if let Some(t) = &req.token {
        t.clone()
    } else if let Some(api_key) = &hasn_config.api_key {
        if api_key.trim().is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "缺少认证凭据 (token 或 api_key)"})),
            )
                .into_response();
        }
        api_key.clone()
    } else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "缺少认证凭据 (token 或 api_key)"})),
        )
            .into_response();
    };

    let mut auth_headers = if token.starts_with("hasn_ok_") {
        vec![("Authorization".to_string(), format!("OwnerKey {}", token))]
    } else if token.starts_with("hasn_nk_") {
        // 向后兼容旧版 NodeKey（过渡期）
        vec![("Authorization".to_string(), format!("NodeKey {}", token))]
    } else {
        vec![("Authorization".to_string(), format!("Bearer {}", token))]
    };

    // 附加 X-Node-Id
    let fp_node_id = crate::device_fingerprint::get_global_fingerprint()
        .map(|fp| fp.node_id.clone())
        .unwrap_or_default();
    if !fp_node_id.is_empty() {
        auth_headers.push(("X-Node-Id".to_string(), fp_node_id));
    }

    // 附加 X-Node-Name（OS 版本标识）
    let os_version = {
        let arch = std::env::consts::ARCH;
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("sw_vers")
                .arg("-productVersion")
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|v| format!("macOS {} ({})", v.trim(), arch))
                .unwrap_or_else(|| format!("macOS ({})", arch))
        }
        #[cfg(target_os = "linux")]
        {
            format!("Linux ({})", arch)
        }
        #[cfg(target_os = "windows")]
        {
            format!("Windows ({})", arch)
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        {
            format!("{} ({})", std::env::consts::OS, arch)
        }
    };
    auth_headers.push(("X-Node-Name".to_string(), os_version));

    let url = format!("{}?protocol=hasn/2.0", base_url);

    let Some(connector) = hasn_node::connector::global_connector_opt() else {
        error!("[HASN API] hasn-node 全局 connector 未初始化");
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "status": "failed",
                "error": "hasn-node 全局 connector 未初始化",
            })),
        )
            .into_response();
    };

    // ⚠️ 不能在 HTTP 处理器里跑 connect_with_retry：
    //   默认 max_retries=10 + 指数退避 (1s→30s)，最坏阻塞 ~3 分钟，
    //   Vite 代理/浏览器的 socket 空闲时间会先超时返回 408。
    //   这里只做一次握手，带 15s 硬超时，重试交给前端 5 分钟心跳兜底。
    const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

    match tokio::time::timeout(CONNECT_TIMEOUT, connector.connect(&url, auth_headers)).await {
        Ok(Ok(())) => {
            info!("[HASN API] 连接成功");
            (
                StatusCode::OK,
                Json(serde_json::json!({"status": "connected"})),
            )
                .into_response()
        }
        Ok(Err(e)) => {
            error!("[HASN API] 连接失败: {}", e);
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "status": "failed",
                    "error": format!("连接失败: {e}"),
                })),
            )
                .into_response()
        }
        Err(_) => {
            error!(
                "[HASN API] 连接超时（{}s），中央节点未在期限内响应",
                CONNECT_TIMEOUT.as_secs()
            );
            (
                StatusCode::GATEWAY_TIMEOUT,
                Json(serde_json::json!({
                    "status": "timeout",
                    "error": format!("HASN 中央节点 {}s 内未握手完成", CONNECT_TIMEOUT.as_secs()),
                })),
            )
                .into_response()
        }
    }
}

// ─── Phase 05-05 Task 4 — 11 端点薄转发 helpers ───
//
// 所有端点共用一个结构：
//   1. 用 `hasn_node::connector::global_connector_opt()` 拿全局 connector
//   2. None 时 → 503 `{"error": "HASN 未初始化"}`
//   3. Ok → 保持 Phase 05-02 桌面端合同（响应 JSON 不带 `data` 外壳）
//   4. Err → 500 `{"error": "..."}`

fn service_unavailable_json() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(serde_json::json!({"error": "HASN 未初始化"})),
    )
}

/// POST /api/v1/hasn/disconnect
pub async fn hasn_disconnect() -> impl IntoResponse {
    let Some(connector) = hasn_node::connector::global_connector_opt() else {
        return service_unavailable_json().into_response();
    };
    connector.disconnect().await;
    (
        StatusCode::OK,
        Json(serde_json::json!({"status": "disconnected"})),
    )
        .into_response()
}

/// GET /api/v1/hasn/status
///
/// 响应 JSON 形状严格保持 Phase 05-02 基线（桌面端合同）：
/// `{connected, node_id, device_fingerprint, node_name, device_platform}`。
/// 内部从 hasn-node 的 `ConnectionSnapshot` 取 `connected` / `node_id`；
/// device_* 三字段继续从 legacy device_fingerprint 全局获取。
pub async fn hasn_status() -> impl IntoResponse {
    let (connected, node_id) = match hasn_node::connector::global_connector_opt() {
        Some(connector) => {
            let snap = connector.status_snapshot().await;
            (snap.connected, snap.node_id)
        }
        None => (false, None),
    };

    let fp = crate::device_fingerprint::get_global_fingerprint();

    Json(serde_json::json!({
        "connected": connected,
        "node_id": node_id,
        "device_fingerprint": fp.map(|f| f.fingerprint.as_str()),
        "node_name": fp.map(|f| f.node_name.as_str()),
        "device_platform": fp.map(|f| f.device_platform.as_str()),
    }))
    .into_response()
}

/// POST /api/v1/hasn/send
///
/// 桌面端合同：成功响应 **必须** 是 `{"status":"sent"}`（不带 `data` 外壳）。
pub async fn hasn_send(Json(req): Json<SendRequest>) -> impl IntoResponse {
    let Some(connector) = hasn_node::connector::global_connector_opt() else {
        return service_unavailable_json().into_response();
    };

    match connector
        .send_message(&req.to, req.content, req.from_id, req.local_id)
        .await
    {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"status": "sent"}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("{e}")})),
        )
            .into_response(),
    }
}

pub async fn hasn_add_owner(Json(req): Json<AddOwnerRequest>) -> impl IntoResponse {
    let Some(connector) = hasn_node::connector::global_connector_opt() else {
        return service_unavailable_json().into_response();
    };
    match connector
        .add_owner(
            &req.owner_id,
            &req.owner_proof.proof_type,
            &req.owner_proof.credential,
        )
        .await
    {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "owner_binding_requested"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("{e}")})),
        )
            .into_response(),
    }
}

pub async fn hasn_add_agent(Json(req): Json<AddAgentRequest>) -> impl IntoResponse {
    let Some(connector) = hasn_node::connector::global_connector_opt() else {
        return service_unavailable_json().into_response();
    };
    match connector
        .add_agent_presence(&req.agent_id, &req.owner_id)
        .await
    {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "agent_add_requested"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("{e}")})),
        )
            .into_response(),
    }
}

pub async fn hasn_remove_agent(
    axum::extract::Path(agent_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    let Some(connector) = hasn_node::connector::global_connector_opt() else {
        return service_unavailable_json().into_response();
    };
    match connector.remove_agent_presence(&agent_id).await {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "agent_remove_requested"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("{e}")})),
        )
            .into_response(),
    }
}

pub async fn hasn_renew_owner(
    axum::extract::Path(owner_id): axum::extract::Path<String>,
    Json(req): Json<OwnerProofItem>,
) -> impl IntoResponse {
    let Some(connector) = hasn_node::connector::global_connector_opt() else {
        return service_unavailable_json().into_response();
    };
    match connector
        .renew_owner(&owner_id, &req.proof_type, &req.credential)
        .await
    {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "owner_renew_requested"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("{e}")})),
        )
            .into_response(),
    }
}

pub async fn hasn_remove_owner(
    axum::extract::Path(owner_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    let Some(connector) = hasn_node::connector::global_connector_opt() else {
        return service_unavailable_json().into_response();
    };
    match connector.remove_owner(&owner_id).await {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "owner_remove_requested"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("{e}")})),
        )
            .into_response(),
    }
}

pub async fn hasn_list_owners() -> impl IntoResponse {
    let Some(connector) = hasn_node::connector::global_connector_opt() else {
        return service_unavailable_json().into_response();
    };
    match connector.list_owners().await {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "owners_list_requested"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("{e}")})),
        )
            .into_response(),
    }
}

/// WS /ws/hasn-events — HASN 事件实时推送
pub async fn hasn_events_ws(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_hasn_events_ws)
}

async fn handle_hasn_events_ws(mut socket: WebSocket) {
    // Phase 05-05 Task 4 — 事件源改走 hasn-node connector。
    // 未初始化时向客户端发一条 text 错误后关闭；桌面端侧逻辑保持兼容。
    let Some(connector) = hasn_node::connector::global_connector_opt() else {
        let err = serde_json::json!({
            "type": "error",
            "error": "HASN 未初始化",
        });
        let _ = socket
            .send(Message::Text(err.to_string().into()))
            .await;
        return;
    };
    let mut rx = connector.subscribe();

    info!("[HASN Events WS] 新订阅者已连接 (via hasn-node)");

    loop {
        tokio::select! {
            event = rx.recv() => {
                match event {
                    Ok(hasn_event) => {
                        if let Ok(json) = serde_json::to_string(&hasn_event) {
                            if socket.send(Message::Text(json.into())).await.is_err() {
                                break; // 客户端断开
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("[HASN Events WS] 落后 {} 条事件", n);
                    }
                    Err(_) => break, // 广播通道关闭
                }
            }

            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }

    info!("[HASN Events WS] 订阅者断开");
}
