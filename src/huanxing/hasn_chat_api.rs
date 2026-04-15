//! HASN 本地聊天 REST API
//!
//! 对齐设计文档第六节: /api/v1/hasn/chat/* 路径
//! 所有数据来源于 hasn_chat.db（本地隔离数据库），不直连云端。

use crate::gateway::AppState;
use crate::huanxing::hasn_router::MessageRouter;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use std::sync::Arc;

// ═══════════════════════════════════════════════════════════════════
// Query Params
// ═══════════════════════════════════════════════════════════════════

#[derive(Deserialize)]
pub struct HasnIdQuery {
    pub hasn_id: String,
}

#[derive(Deserialize)]
pub struct GetMessagesQuery {
    pub hasn_id: String,
    pub conversation_id: String,
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}

#[derive(Deserialize)]
pub struct MarkReadQuery {
    pub hasn_id: String,
    pub conversation_id: String,
}

#[derive(Deserialize)]
pub struct GetContactQuery {
    pub hasn_id: String,
    pub peer_id: String,
}

#[derive(Deserialize)]
pub struct SyncQuery {
    pub hasn_id: String,
}

fn default_limit() -> u32 {
    50
}

// ═══════════════════════════════════════════════════════════════════
// 会话 API
// ═══════════════════════════════════════════════════════════════════

/// GET /api/v1/hasn/chat/sessions — 获取会话列表
pub async fn hasn_chat_get_sessions(
    State(state): State<AppState>,
    Query(query): Query<HasnIdQuery>,
) -> impl IntoResponse {
    let router = MessageRouter::new_for_api(Arc::new(state));
    match router.resolve_user_chat_db(&query.hasn_id).await {
        Ok(db) => match db.get_sessions().await {
            Ok(sessions) => (
                StatusCode::OK,
                Json(serde_json::json!({ "sessions": sessions })),
            )
                .into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response(),
        },
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// GET /api/v1/hasn/chat/messages — 获取消息历史
pub async fn hasn_chat_get_messages(
    State(state): State<AppState>,
    Query(query): Query<GetMessagesQuery>,
) -> impl IntoResponse {
    let router = MessageRouter::new_for_api(Arc::new(state));
    match router.resolve_user_chat_db(&query.hasn_id).await {
        Ok(db) => match db
            .get_messages(&query.conversation_id, query.limit, query.offset)
            .await
        {
            Ok(messages) => (
                StatusCode::OK,
                Json(serde_json::json!({ "messages": messages })),
            )
                .into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response(),
        },
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// POST /api/v1/hasn/chat/read — 标记会话已读
pub async fn hasn_chat_mark_read(
    State(state): State<AppState>,
    Query(query): Query<MarkReadQuery>,
) -> impl IntoResponse {
    let router = MessageRouter::new_for_api(Arc::new(state));
    match router.resolve_user_chat_db(&query.hasn_id).await {
        Ok(db) => match db.mark_read(&query.conversation_id).await {
            Ok(_) => (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response(),
        },
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// GET /api/v1/hasn/chat/contacts — 获取联系人列表
pub async fn hasn_chat_get_contacts(
    State(state): State<AppState>,
    Query(query): Query<HasnIdQuery>,
) -> impl IntoResponse {
    let router = MessageRouter::new_for_api(Arc::new(state));
    match router.resolve_user_chat_db(&query.hasn_id).await {
        Ok(db) => match db.get_all_contacts().await {
            Ok(contacts) => (
                StatusCode::OK,
                Json(serde_json::json!({ "contacts": contacts })),
            )
                .into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response(),
        },
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// GET /api/v1/hasn/chat/contacts/detail — 获取单个联系人详情
pub async fn hasn_chat_get_contact(
    State(state): State<AppState>,
    Query(query): Query<GetContactQuery>,
) -> impl IntoResponse {
    let router = MessageRouter::new_for_api(Arc::new(state));
    match router.resolve_user_chat_db(&query.hasn_id).await {
        Ok(db) => match db.get_contact(&query.peer_id).await {
            Ok(Some(contact)) => (
                StatusCode::OK,
                Json(serde_json::json!({ "contact": contact })),
            )
                .into_response(),
            Ok(None) => (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "联系人不存在" })),
            )
                .into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response(),
        },
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// GET /api/v1/hasn/chat/sync/status — 查询同步状态
pub async fn hasn_chat_sync_status(
    State(state): State<AppState>,
    Query(query): Query<HasnIdQuery>,
) -> impl IntoResponse {
    let router = MessageRouter::new_for_api(Arc::new(state));
    match router.resolve_user_chat_db(&query.hasn_id).await {
        Ok(db) => {
            let contacts_sync = db
                .get_sync_state("contacts_last_sync")
                .await
                .ok()
                .flatten();
            let conversations_sync = db
                .get_sync_state("conversations_last_sync")
                .await
                .ok()
                .flatten();
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "contacts_last_sync": contacts_sync,
                    "conversations_last_sync": conversations_sync,
                })),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
