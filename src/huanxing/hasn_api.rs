//! HASN HTTP/WS 端点 — 供前端通过 Sidecar 收发 HASN 消息
//!
//! 端点列表:
//! - POST   /api/v1/hasn/connect     连接 HASN 中央节点
//! - POST   /api/v1/hasn/disconnect  断开连接
//! - GET    /api/v1/hasn/status      获取连接状态
//! - POST   /api/v1/hasn/send        发送消息
//! - POST   /api/v1/hasn/report      上报 Agent 列表
//! - WS     /ws/hasn-events          HASN 事件实时推送

use std::sync::Arc;

use axum::{
    extract::{State, ws::{Message, WebSocket, WebSocketUpgrade}},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use tracing::{error, info};

use crate::gateway::AppState;
use crate::huanxing::hasn_connector;
use hasn_client_core::model::AgentReport;

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
pub struct ReportAgentsRequest {
    pub agents: Vec<AgentReportItem>,
}

#[derive(Debug, Deserialize)]
pub struct AgentReportItem {
    pub hasn_id: String,
    #[serde(default)]
    pub owner_id: Option<String>,
}

// ─── 端点实现 ───

/// POST /api/v1/hasn/connect
pub async fn hasn_connect(
    State(state): State<AppState>,
    Json(req): Json<ConnectRequest>,
) -> impl IntoResponse {
    let config = state.config.lock().clone();
    let hasn_config = &config.huanxing.hasn;

    // 构建 WS URL
    let base_url = req.url
        .or_else(|| hasn_config.central_url.clone())
        .unwrap_or_else(|| {
            format!("{}/api/v1/hasn/ws/node", config.huanxing.hasn_url()
                .replace("https://", "wss://")
                .replace("http://", "ws://"))
        });

    // 认证参数
    let auth_param = if let Some(token) = &req.token {
        if token.starts_with("hasn_ak_") {
            format!("?api_key={}", token)
        } else {
            format!("?token={}", token)
        }
    } else if let Some(api_key) = &hasn_config.api_key {
        format!("?api_key={}", api_key)
    } else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "缺少认证凭据 (token 或 api_key)"})),
        ).into_response();
    };

    let url = format!("{}{}", base_url, auth_param);

    let connector = hasn_connector::global_connector();
    let max_retries = hasn_config.max_retries;

    match connector.connect_with_retry(&url, max_retries, Arc::new(state)).await {
        Ok(()) => {
            info!("[HASN API] 连接成功");
            (StatusCode::OK, Json(serde_json::json!({"status": "connected"}))).into_response()
        }
        Err(e) => {
            error!("[HASN API] 连接失败: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("连接失败: {e}")})),
            ).into_response()
        }
    }
}

/// POST /api/v1/hasn/disconnect
pub async fn hasn_disconnect() -> impl IntoResponse {
    let connector = hasn_connector::global_connector();
    connector.disconnect().await;
    (StatusCode::OK, Json(serde_json::json!({"status": "disconnected"})))
}

/// GET /api/v1/hasn/status
pub async fn hasn_status() -> impl IntoResponse {
    let connector = hasn_connector::global_connector();
    let connected = connector.is_connected().await;
    let node_id = connector.get_node_id().await;

    Json(serde_json::json!({
        "connected": connected,
        "node_id": node_id,
    }))
}

/// POST /api/v1/hasn/send
pub async fn hasn_send(
    Json(req): Json<SendRequest>,
) -> impl IntoResponse {
    let connector = hasn_connector::global_connector();

    match connector.send_message(&req.to, req.content, req.from_id, req.local_id).await {
        Ok(()) => {
            (StatusCode::OK, Json(serde_json::json!({"status": "sent"}))).into_response()
        }
        Err(e) => {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("{e}")})),
            ).into_response()
        }
    }
}

/// POST /api/v1/hasn/report
pub async fn hasn_report_agents(
    Json(req): Json<ReportAgentsRequest>,
) -> impl IntoResponse {
    let connector = hasn_connector::global_connector();

    let agents: Vec<AgentReport> = req.agents.into_iter().map(|a| AgentReport {
        hasn_id: a.hasn_id,
        owner_id: a.owner_id,
    }).collect();

    match connector.report_agents(agents).await {
        Ok(()) => {
            (StatusCode::OK, Json(serde_json::json!({"status": "reported"}))).into_response()
        }
        Err(e) => {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("{e}")})),
            ).into_response()
        }
    }
}

/// WS /ws/hasn-events — HASN 事件实时推送
pub async fn hasn_events_ws(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_hasn_events_ws)
}

async fn handle_hasn_events_ws(mut socket: WebSocket) {
    let connector = hasn_connector::global_connector();
    let mut rx = connector.subscribe();

    info!("[HASN Events WS] 新订阅者已连接");

    loop {
        tokio::select! {
            // 从 HASN 事件广播接收
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

            // 客户端发来的消息（暂时只处理 Close）
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {} // 忽略其他消息
                }
            }
        }
    }

    info!("[HASN Events WS] 订阅者断开");
}
