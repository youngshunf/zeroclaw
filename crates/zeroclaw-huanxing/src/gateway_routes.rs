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

use axum::routing::{get, post};
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
        // ── HASN Agent 同步调用端点（桌面端 Runtime 层桥接，保留） ──────
        .route(
            "/api/v1/agent/hasn-invoke",
            post(crate::hasn_bridge::invoke::hasn_invoke),
        )
        // ── M3: 19 条 /api/v1/hasn/** 路由已迁移到 hasn-node ───────────
        // 前端 HASN_NODE_BASE = http://127.0.0.1:42618/api/v1/hasn 直连 hasn-node，
        // 不再经 Tauri gateway。WS /ws/hasn-events 同样迁到 hasn-node。
        // 旧 handler 文件（hasn_api.rs / hasn_chat_api.rs）待本 commit 后删除。
        // ── WebSocket 端点 ──────────────────────────────────────────
        // Phase 05-04d 起，`huanxing` feature 通过
        // `zeroclaw-gateway/external_chat_ws` 让上游编译期不注册默认 /ws/chat，
        // 由下面这一行的 hx_ws 多租户版本接管同一路径 —— 桌面端 WS URL
        // 不用改，tenant 自动从 WS query/headers 解析，api_key 从租户级
        // config.toml 自动注入到 reliability.api_keys。
        .route("/ws/chat", get(crate::hx_ws::handle_ws_chat))
}

