//! HASN Agent 同步调用 HTTP 端点
//!
//! 为 Tauri 桌面端提供 HTTP API，将 HASN 收到的社交消息转发给本地 Agent 处理。
//!
//! ```text
//! POST /api/v1/agent/hasn-invoke
//!
//! Request:
//! {
//!   "hasn_id": "a_001",
//!   "session_id": "conv_xxx",
//!   "from_id": "h_yyy",
//!   "message": "帮我查天气"
//! }
//!
//! Response:
//! {
//!   "reply": "今天北京晴转多云..."
//! }
//! ```

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use tracing::{error, info};

use crate::gateway::AppState;
use crate::huanxing::agent_bridge;

/// HASN invoke 请求体
#[derive(Debug, Deserialize)]
pub struct HasnInvokeRequest {
    /// Agent 的 hasn_id（如 "a_001"），用于查找对应工作区
    pub hasn_id: String,
    /// HASN conversation_id，映射为 Sidecar session_id
    pub session_id: String,
    /// 发送者 hasn_id（如 "h_yyy"），用于 Agent 上下文
    pub from_id: String,
    /// 消息正文
    pub message: String,
}

/// POST /api/v1/agent/hasn-invoke
///
/// 将 HASN 消息转发给本地 Agent 同步处理。
/// 通过 hasn_id 解析工作区，使用 conversation_id 作为 session_id 保持上下文。
pub async fn hasn_invoke(
    State(state): State<AppState>,
    Json(req): Json<HasnInvokeRequest>,
) -> impl IntoResponse {
    let config = state.config.lock().clone();

    if !config.huanxing.enabled {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "唤星功能未启用"})),
        )
            .into_response();
    }

    let agents_dir = config
        .huanxing
        .resolve_agents_dir(config.config_path.parent().unwrap_or(&config.workspace_dir));

    // 通过 hasn_id 解析工作区路径
    let bridge = agent_bridge::global_bridge();
    let workspace = match bridge
        .resolve_workspace_by_hasn_id(&agents_dir, &req.hasn_id)
        .await
    {
        Some(ws) => ws,
        None => {
            error!(hasn_id = %req.hasn_id, "HASN invoke: 找不到 Agent 工作区");
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": format!("Agent '{}' 工作区不存在", req.hasn_id)
                })),
            )
                .into_response();
        }
    };

    info!(
        hasn_id = %req.hasn_id,
        session_id = %req.session_id,
        from_id = %req.from_id,
        workspace = %workspace.display(),
        "HASN invoke: 开始处理"
    );

    // 调用 Agent
    match bridge
        .invoke(&state, &workspace, &req.session_id, &req.message)
        .await
    {
        Ok(result) => {
            info!(
                hasn_id = %req.hasn_id,
                session_id = %req.session_id,
                reply_len = result.reply.len(),
                "HASN invoke: 处理完成"
            );
            (StatusCode::OK, Json(serde_json::json!(result))).into_response()
        }
        Err(e) => {
            error!(
                hasn_id = %req.hasn_id,
                session_id = %req.session_id,
                error = %e,
                "HASN invoke: Agent 处理失败"
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": format!("Agent 处理失败: {e}")
                })),
            )
                .into_response()
        }
    }
}
