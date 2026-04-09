//! HuanXing multi-tenant routing layer.
//! ... (恢复原始 mod.rs 内容)
pub mod agent_bridge;
pub mod api_agents;
pub mod api_client;
pub mod api_sessions;
pub mod bootstrap;
pub mod channel_registry;
pub mod channels;
pub mod config;
pub mod db;
pub mod device_fingerprint;
pub mod doc_tools;
pub mod hasn_api;
pub mod hasn_connector;
pub mod hasn_invoke;
pub mod hasn_tools;
pub mod hub_sync;
pub mod hx_image_gen;
pub mod hx_web_search;
pub mod hx_ws;
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
