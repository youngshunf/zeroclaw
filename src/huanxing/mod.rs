//! HuanXing multi-tenant routing layer.
//!
//! Routes inbound channel messages to per-tenant agent contexts based on
//! sender identity. The [`TenantRouter`] resolves `(channel, sender_id)` to
//! a [`TenantContext`] that carries per-user system prompt, tools, model,
//! and workspace configuration.
//!
//! # Architecture
//!
//! ```text
//! Channel.listen()
//!       │
//!       ▼
//! ChannelMessage { sender, channel, content }
//!       │
//!       ▼
//! ┌─────────────┐
//! │TenantRouter  │  (channel:sender) → TenantContext
//! └──────┬──────┘
//!        │
//!   ┌────┼────┐
//!   ▼    ▼    ▼
//! Guardian  User1  User2   ← per-tenant system_prompt / tools / model
//! ```
//!
//! Channels (NapCat WS, Feishu App, QQ) are **shared** — one connection
//! serves all tenants. Only the agent context (prompt, memory, workspace)
//! is per-tenant.

pub mod agent_bridge;
pub mod channel_registry;
pub mod channels;
pub mod api_agents;
pub mod api_sessions;
pub mod api_client;
pub mod bootstrap;
pub mod config;
pub mod db;
pub mod doc_tools;
pub mod hasn_api;
pub mod hasn_connector;
pub mod hasn_invoke;
pub mod hasn_tools;
pub mod hx_image_gen;
pub mod hx_web_search;
pub mod hx_ws;
pub mod hub_sync;
pub mod multi_tenant_resolver;
pub mod permissions;
pub mod registry;
pub mod register;
pub mod router;
pub mod secret_tools;
pub mod skill_market_tools;
pub mod sync;
pub mod sop_api;
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
#[allow(unused_imports)] // 公共 API 导出，供外部模块使用
pub use tenant::TenantContext;
