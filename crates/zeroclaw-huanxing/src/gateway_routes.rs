//! 唤星 gateway 路由汇总。
//!
//! 将所有唤星扩展的 REST 端点收拢在一个 `huanxing_routes()` 函数里，
//! 供根 crate 在启动 gateway 前通过 `zeroclaw_gateway::register_router_extender`
//! 注册进 axum Router。
//!
//! 路由清单（Phase 5 从 huanxing-clean 的 src/gateway/mod.rs 迁移而来）：
//! - 【Agent 管理】    /api/agents/*           api_agents::agent_routes
//! - 【用户级配置】    /api/user_config/*      api_user_config::user_config_routes
//! - 【Session REST】  /api/huanxing/sessions/*  api_sessions::session_routes（Phase 05-04b 起从 /api/sessions 迁出，避免与上游 `handle_api_sessions_*` merge panic）
//! - 【SOP 工作流】    /api/sop/*              sop_api::sop_routes
//! - 【Hub 同步】      /api/hub_sync/*         hub_sync::hub_routes
//! - 【HASN Agent 同步调用】 /api/v1/agent/hasn-invoke
//! - 【HASN 节点连接】 /api/v1/hasn/{connect,disconnect,status,send}
//! - 【HASN 本地 IM】  /api/v1/hasn/chat/*     hasn_chat_api::*
//! - 【HASN 节点管理】 /api/v1/hasn/node/owners/*, /node/agents/*
//! - 【HASN 事件 WS】  /ws/hasn-events
//! - 【Agent WS】      /ws/chat                hx_ws::handle_ws_chat

use axum::routing::{delete, get, post};
use axum::Router;
use zeroclaw_gateway::AppState;

/// 构建包含所有唤星扩展 REST / WS 端点的 axum Router。
///
/// 调用方需在 gateway 启动前通过
/// `zeroclaw_gateway::register_router_extender` 注册此 Router，
/// zeroclaw-gateway 的 `run_gateway` 会在构建 inner router 时 `.merge()` 进去。
pub fn huanxing_routes() -> Router<AppState> {
    Router::new()
        // ── Agent 管理 / Session / 用户配置 / SOP / Hub ──────────────
        .merge(crate::api_agents::agent_routes())
        .merge(crate::api_user_config::user_config_routes())
        .merge(crate::api_sessions::session_routes())
        .merge(crate::sop_api::sop_routes())
        .merge(crate::hub_sync::hub_routes())
        // ── HASN Agent 同步调用端点（桌面端 Sidecar 用） ──────────────
        .route(
            "/api/v1/agent/hasn-invoke",
            post(crate::hasn_invoke::hasn_invoke),
        )
        // ── HASN 节点连接管理 API ────────────────────────────────────
        .route("/api/v1/hasn/connect", post(crate::hasn_api::hasn_connect))
        .route(
            "/api/v1/hasn/disconnect",
            post(crate::hasn_api::hasn_disconnect),
        )
        .route("/api/v1/hasn/status", get(crate::hasn_api::hasn_status))
        .route("/api/v1/hasn/send", post(crate::hasn_api::hasn_send))
        // ── HASN 本地 IM (chat_db) REST API ──────────────────────────
        .route(
            "/api/v1/hasn/chat/sessions",
            get(crate::hasn_chat_api::hasn_chat_get_sessions),
        )
        .route(
            "/api/v1/hasn/chat/messages",
            get(crate::hasn_chat_api::hasn_chat_get_messages),
        )
        .route(
            "/api/v1/hasn/chat/read",
            post(crate::hasn_chat_api::hasn_chat_mark_read),
        )
        .route(
            "/api/v1/hasn/chat/contacts",
            get(crate::hasn_chat_api::hasn_chat_get_contacts),
        )
        .route(
            "/api/v1/hasn/chat/contacts/detail",
            get(crate::hasn_chat_api::hasn_chat_get_contact),
        )
        .route(
            "/api/v1/hasn/chat/sync/status",
            get(crate::hasn_chat_api::hasn_chat_sync_status),
        )
        // ── HASN 节点 Owner / Agent 管理 ─────────────────────────────
        .route(
            "/api/v1/hasn/node/owners",
            post(crate::hasn_api::hasn_add_owner).get(crate::hasn_api::hasn_list_owners),
        )
        .route(
            "/api/v1/hasn/node/owners/{owner_id}",
            delete(crate::hasn_api::hasn_remove_owner),
        )
        .route(
            "/api/v1/hasn/node/owners/{owner_id}/renew",
            post(crate::hasn_api::hasn_renew_owner),
        )
        .route(
            "/api/v1/hasn/node/agents",
            post(crate::hasn_api::hasn_add_agent),
        )
        .route(
            "/api/v1/hasn/node/agents/{agent_id}",
            delete(crate::hasn_api::hasn_remove_agent),
        )
        // ── WebSocket 端点 ──────────────────────────────────────────
        // 注：`/ws/chat` 历史上在 huanxing feature 下替换为 hx_ws 的多会话
        // 版本，但新的 router extension 模式无法 route 替换（axum merge 会
        // panic），目前保留上游 ws::handle_ws_chat，hx_ws 仅供 HASN 内部
        // invoke 使用。桌面端 HASN 消息流通过 /ws/hasn-events + REST API。
        .route("/ws/hasn-events", get(crate::hasn_api::hasn_events_ws))
}

