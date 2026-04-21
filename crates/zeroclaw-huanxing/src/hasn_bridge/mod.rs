//! hasn_bridge —— 唤星桌面端与 hasn-node 的桥接层
//!
//! 职责：
//! - `spawner.rs`：`HuanxingNativeSpawner` 实现 `hasn_node::AgentSpawner`，
//!   负责把入站消息路由到桌面端 runtime，并把 `TurnEvent` 转为 `ReplyChunk`
//! - `agent_bridge.rs`：`HasnAgentBridge` —— spawner 的消息注入/流式转发 helper
//! - `sync.rs`：唤星云端→本地 chat_db 的增量同步（联系人/会话列表）
//! - `invoke.rs`：前端 `/api/v1/agent/hasn-invoke` 端点 handler
//! - `tools.rs`：Agent 运行时工具（`hasn_send` / `hasn_contacts` 等）
//! - `provisioner.rs`：PROVISION 帧反向回调（Phase 6+ 待实装）
//! - `discovery.rs`：`HuanxingAgentDiscovery` crash 恢复注册路径（Phase 6+ 待实装）
//!
//! 本模块是设计文档 `docs/架构设计/HASN-Node独立运行时/14-桌面端完整迁移实施计划.md`
//! 定义的桥接层实体。对应原 `hasn_*.rs` 5 个文件已物理搬迁至此。

pub mod agent_bridge;
pub mod invoke;
pub mod spawner;
pub mod sync;
pub mod tools;

// Phase 14.2 M3：从 `crates/zeroclaw-huanxing/src/hasn_{chat_db,connector,router}.rs`
// 物理搬迁进来的 legacy 模型层。桌面端本地 `~/.huanxing/users/*/data/hasn_chat.db`
// 仍由 chat_db 管理；WS 入站/出站热路径已由 hasn-node 接管（见 spawner.rs
// 顶部 cutover 表）。这三个模块保留在 hasn_bridge 命名空间下是为了满足
// 14.9 验收：`huanxing-zeroclaw/src/` 零 `hasn_` 前缀模块（除 `hasn_bridge/`）。
pub mod chat_db;
pub mod connector;
pub mod router;

// Phase 6+ 占位
pub mod discovery;
pub mod provisioner;

// 对外 re-export：保持 main.rs 等宿主调用路径稳定
pub use spawner::{
    configure_huanxing_native_runtime,
    initialize_embedded_huanxing_node,
    register_huanxing_native_spawner,
};
