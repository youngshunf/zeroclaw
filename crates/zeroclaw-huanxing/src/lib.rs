//! 唤星多租户 SaaS 扩展 crate。
//!
//! 本 crate 遵循 RFC D1 workspace 拆分范式，把所有唤星业务逻辑从根
//! `zeroclawlabs` crate 里剥离出来：
//!
//! - `TenantRouter` / `TenantContext` / `TenantDb` —— 多租户路由层
//! - `MultiTenantResolver` —— `MessageContextResolver` trait 的唤星实现
//! - `api_*` —— Agent / Session / UserConfig REST API
//! - `hasn_*` —— HASN 协议集成（WebSocket + 本地 IM + 消息路由）
//! - 各 `*_tools` / `hx_*` —— 唤星专属工具
//! - `channels/` —— 唤星专属渠道扩展（napcat、wechat_pad、weixin）
//!
//! 挂载到根 crate 的路径：
//! 1. `features.huanxing = ["dep:zeroclaw-huanxing", ...]`
//! 2. 根 `src/lib.rs` 加 `#[cfg(feature = "huanxing")] pub use zeroclaw_huanxing;`
//! 3. `src/daemon/mod.rs` 启动时调用 `zeroclaw_huanxing::bootstrap::init_tenant_systems()`
//! 4. `src/gateway/mod.rs` merge `zeroclaw_huanxing::api::router()`

#![warn(clippy::all)]

#![allow(
    clippy::module_name_repetitions,
    clippy::too_many_lines,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::must_use_candidate,
    clippy::uninlined_format_args,
    clippy::needless_pass_by_value,
    clippy::single_match_else,
    clippy::map_unwrap_or,
    clippy::items_after_statements,
    clippy::similar_names,
    clippy::redundant_closure_for_method_calls
)]

pub mod agent_bridge;
pub mod api_agents;
pub mod api_client;
pub mod api_sessions;
pub mod api_user_config;
pub mod bootstrap;
pub mod channel_registry;
pub mod channels;
pub mod config;
pub mod context_resolver;

/// Re-export shim for `zeroclaw_macros::Configurable` derive macro, which
/// expands to `crate::security::SecurityPolicy` paths on fields marked
/// `#[secret]`. Mirrors the shim that zeroclaw-config itself maintains.
pub mod security {
    pub use zeroclaw_config::security::*;
}
pub mod db;
pub mod device_fingerprint;
pub mod gateway_routes;
pub mod doc_tools;
pub mod hasn_agent_bridge;
pub mod hasn_api;
pub mod hasn_chat_api;
pub mod hasn_chat_db;
pub mod hasn_connector;
pub mod hasn_invoke;
pub mod hasn_router;
pub mod hasn_sync;
pub mod hasn_tools;
pub mod hub_sync;
pub mod hx_image_gen;
pub mod hx_web_search;
pub mod hx_ws;
pub mod knowledge_cross;
pub mod migrate;
pub mod multi_tenant_resolver;
pub mod permissions;
pub mod register;
pub mod registry;
pub mod router;
pub mod secret_tools;
pub mod skill_market_tools;
pub mod sop_api;
pub mod sync;
pub mod templates;
pub mod tenant;
pub mod tenant_heartbeat;
pub mod tools;
pub mod tts_dashscope;
pub mod voice;
pub mod voice_hook;
pub mod ws_observer;

pub use api_client::ApiClient;
pub use config::HuanXingConfig;
pub use db::TenantDb;
pub use multi_tenant_resolver::MultiTenantResolver;
pub use router::TenantRouter;
#[allow(unused_imports)]
pub use tenant::TenantContext;
