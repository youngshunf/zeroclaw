//! HuanxingNativeSpawner — 唤星原生 Agent 进程内桥接骨架
//!
//! Phase 2: 只实现 trait 接口，dispatch 返回 todo!()
//! Phase 5: 填充 AgentFactory 调用逻辑
//!
//! 设计要点（per COMP-03）:
//! - 进程内直接调用，零 HTTP / IPC 开销
//! - 通过 hasn_node::spawner::AgentSpawner trait 桥接
//! - Phase 5 注入 AgentFactory 后实现完整 dispatch

use async_trait::async_trait;
use hasn_node::spawner::{AgentSpawner, InboundContext, ReplyChunk};
use tokio::sync::mpsc;

/// 唤星原生 Spawner — 进程内直接调用 AgentFactory（零 IPC 开销）
///
/// Phase 2 骨架：trait 编译验证 + SpawnerRegistry 注册验证
/// Phase 5 填充：注入 AgentFactory，dispatch 调用生成响应流
pub struct HuanxingNativeSpawner {
    // Phase 5 添加: agent_factory: Arc<AgentFactory>
}

impl HuanxingNativeSpawner {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for HuanxingNativeSpawner {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AgentSpawner for HuanxingNativeSpawner {
    fn name(&self) -> &str {
        "huanxing_native"
    }

    async fn dispatch(
        &self,
        _ctx: InboundContext,
    ) -> anyhow::Result<mpsc::Receiver<ReplyChunk>> {
        todo!("Phase 5 实现：通过 AgentFactory 进程内调用唤星 Agent 生成响应")
    }

    async fn probe(&self) -> bool {
        // Phase 5 改为检查 AgentFactory 是否就绪
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use hasn_node::spawner::SpawnerRegistry;

    #[test]
    fn test_register_huanxing_native_spawner() {
        let mut registry = SpawnerRegistry::new();
        let spawner = Arc::new(HuanxingNativeSpawner::new());
        registry.register(spawner);
        assert!(registry.get("huanxing_native").is_some());
        assert_eq!(registry.list_names(), vec!["huanxing_native"]);
    }

    #[test]
    fn test_spawner_name() {
        let spawner = HuanxingNativeSpawner::new();
        assert_eq!(spawner.name(), "huanxing_native");
    }

    #[test]
    fn test_default_impl() {
        let spawner = HuanxingNativeSpawner::default();
        assert_eq!(spawner.name(), "huanxing_native");
    }
}
