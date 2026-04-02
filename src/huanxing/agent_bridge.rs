//! Agent Bridge — 统一 Agent 会话管理层
//!
//! 提供 hasn_id → agent_name 解析和同步 Agent 调用能力，
//! 供 WS （hx_ws.rs）和 HTTP（hasn_invoke.rs）两个入口共享复用。

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use crate::gateway::AppState;
use crate::huanxing::TenantContext;

/// Agent 桥接器 — hasn_id 解析 + 同步 Agent 调用
pub struct AgentBridge {
    /// hasn_id → TenantContext 缓存
    hasn_id_cache: Mutex<HashMap<String, Arc<TenantContext>>>,
}

impl AgentBridge {
    pub fn new() -> Self {
        Self {
            hasn_id_cache: Mutex::new(HashMap::new()),
        }
    }

    /// 通过 hasn_id 查找对应的 TenantContext。
    ///
    /// 1. 查缓存
    /// 2. 统一通过 TenantContext 解析 tenant + agent 运行时
    pub async fn resolve_tenant_by_hasn_id(
        &self,
        state: &AppState,
        hasn_id: &str,
    ) -> Option<Arc<TenantContext>> {
        // 1. 先查缓存
        {
            let cache = self.hasn_id_cache.lock().await;
            if let Some(ctx) = cache.get(hasn_id) {
                if ctx.workspace_dir.exists() {
                    return Some(ctx.clone());
                }
            }
        }

        // 2. 统一通过 TenantContext 解析
        let config = state.config.lock().clone();
        match TenantContext::load_by_hasn_id(&config, hasn_id).await {
            Ok(Some(ctx)) if ctx.workspace_dir.exists() => {
                debug!(
                    hasn_id,
                    workspace = %ctx.workspace_dir.display(),
                    "hasn_id 解析命中"
                );
                let ctx = Arc::new(ctx);
                self.hasn_id_cache
                    .lock()
                    .await
                    .insert(hasn_id.to_string(), ctx.clone());
                Some(ctx)
            }
            Ok(Some(ctx)) => {
                warn!(
                    hasn_id,
                    workspace = %ctx.workspace_dir.display(),
                    "数据库找到记录，但工作区目录不存在"
                );
                None
            }
            Ok(None) => {
                warn!(hasn_id, "未找到 hasn_id 对应的 Agent 工作区");
                None
            }
            Err(err) => {
                warn!(hasn_id, error = %err, "hasn_id 解析 TenantContext 失败");
                None
            }
        }
    }

    /// 同步调用 Agent 处理一条消息（HASN invoke 用）
    ///
    /// 1. 根据 workspace 路径创建 Agent
    /// 2. 使用 session_id 恢复历史上下文
    /// 3. 调用 agent.turn(message)
    /// 4. 持久化对话
    /// 5. 返回完整回复
    pub async fn invoke(
        &self,
        state: &AppState,
        tenant: &TenantContext,
        session_id: &str,
        message: &str,
    ) -> anyhow::Result<InvokeResult> {
        let mut agent = tenant.create_agent().await?;
        agent.set_memory_session_id(Some(session_id.to_string()));

        let session_key = format!("hasn_{session_id}");
        let per_user_backend = tenant
            .session_manager
            .clone()
            .or_else(|| state.session_backend.clone());

        // 恢复历史
        if let Some(ref backend) = per_user_backend {
            let messages = backend.load(&session_key);
            if !messages.is_empty() {
                debug!(
                    session_id,
                    history_count = messages.len(),
                    "恢复 HASN session 历史"
                );
                agent.seed_history(&messages);
            }
        }

        // 先持久化用户消息
        if let Some(ref backend) = per_user_backend {
            let user_msg = crate::providers::ChatMessage::user(message);
            let _ = backend.append(&session_key, &user_msg);
        }

        // 执行 Agent turn
        info!(
            session_id,
            workspace = %tenant.workspace_dir.display(),
            "HASN invoke: Agent.turn()"
        );
        let response = agent.turn(message).await?;

        // 持久化 Agent 回复
        if let Some(ref backend) = per_user_backend {
            let assistant_msg = crate::providers::ChatMessage::assistant(&response);
            let _ = backend.append(&session_key, &assistant_msg);
        }

        Ok(InvokeResult { reply: response })
    }

    /// 流式同步调用 Agent 处理一条消息（供 WS UI 消费用）
    pub async fn invoke_streaming(
        &self,
        state: &AppState,
        tenant: &TenantContext,
        session_id: &str,
        message: &str,
        tx: tokio::sync::mpsc::UnboundedSender<String>,
    ) -> anyhow::Result<()> {
        let mut agent = tenant.create_agent().await?;
        agent.set_memory_session_id(Some(session_id.to_string()));

        let session_key = format!("hasn_{session_id}");
        let per_user_backend = tenant
            .session_manager
            .clone()
            .or_else(|| state.session_backend.clone());

        if let Some(ref backend) = per_user_backend {
            let messages = backend.load(&session_key);
            if !messages.is_empty() {
                agent.seed_history(&messages);
            }
            let user_msg = crate::providers::ChatMessage::user(message);
            let _ = backend.append(&session_key, &user_msg);
        }

        info!(
            session_id,
            workspace = %tenant.workspace_dir.display(),
            "HASN invoke_streaming: Agent.turn_streamed()"
        );

        let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<crate::agent::TurnEvent>(100);

        let tx_clone = tx.clone();
        tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                match event {
                    crate::agent::TurnEvent::Chunk { delta, .. } => {
                        let _ = tx_clone.send(delta);
                    }
                    _ => {}
                }
            }
        });

        let full_reply = agent.turn_streamed(message, event_tx).await?;

        if let Some(ref backend) = per_user_backend {
            if !full_reply.is_empty() {
                let assistant_msg = crate::providers::ChatMessage::assistant(&full_reply);
                let _ = backend.append(&session_key, &assistant_msg);
            }
        }

        Ok(())
    }

    /// 清除 hasn_id 缓存（Agent 注册/注销时调用）
    pub async fn invalidate_cache(&self, hasn_id: &str) {
        self.hasn_id_cache.lock().await.remove(hasn_id);
    }
}

/// Agent invoke 结果
#[derive(Debug, serde::Serialize)]
pub struct InvokeResult {
    pub reply: String,
}

/// 全局 AgentBridge 单例
static BRIDGE: std::sync::OnceLock<AgentBridge> = std::sync::OnceLock::new();

/// 获取全局 AgentBridge 实例
pub fn global_bridge() -> &'static AgentBridge {
    BRIDGE.get_or_init(AgentBridge::new)
}
