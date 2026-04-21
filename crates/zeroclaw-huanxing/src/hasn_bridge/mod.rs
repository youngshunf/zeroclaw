//! hasn_bridge —— 唤星桌面端与 hasn-node 的桥接层
//!
//! 职责：
//! - `spawner.rs`：`HuanxingNativeSpawner` 实现 `hasn_node::AgentSpawner`，
//!   负责把入站消息路由到桌面端 runtime，并把 `TurnEvent` 转为 `ReplyChunk`
//! - `agent_bridge.rs`：`HasnAgentBridge` —— spawner 的消息注入/流式转发 helper
//! - `invoke.rs`：前端 `/api/v1/agent/hasn-invoke` 端点 handler
//! - `tools.rs`：Agent 运行时工具（`hasn_send` / `hasn_contacts` 等）
//! - `provisioner.rs`：PROVISION 帧反向回调（Phase 6+ 待实装）
//! - `discovery.rs`：`HuanxingAgentDiscovery` crash 恢复注册路径（Phase 6+ 待实装）
//!
//! HASN 协议层（WS 连接、路由、权限判决、ChatStorage 持久化、云端同步）
//! 已全部由独立的 `hasn-node` crate 承载。桌面端不再拥有重复实现。

pub mod agent_bridge;
pub mod invoke;
pub mod spawner;
pub mod tools;

// Phase 6+ 占位
pub mod discovery;
pub mod provisioner;

// 对外 re-export：保持 main.rs 等宿主调用路径稳定
pub use spawner::{
    configure_huanxing_native_runtime,
    initialize_embedded_huanxing_node,
    register_huanxing_native_spawner,
};
