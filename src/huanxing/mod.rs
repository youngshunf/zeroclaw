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

pub mod api_client;
pub mod config;
pub mod db;
pub mod doc_tools;
pub mod hasn_tools;
pub mod permissions;
pub mod registry;
pub mod router;
pub mod secret_tools;
pub mod skill_market_tools;
pub mod sync;
pub mod templates;
pub mod tenant;
pub mod tenant_heartbeat;
pub mod tools;

pub use api_client::ApiClient;
pub use config::HuanXingConfig;
pub use db::TenantDb;
pub use router::TenantRouter;
pub use tenant::TenantContext;
