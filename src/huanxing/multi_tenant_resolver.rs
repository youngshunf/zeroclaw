//! Multi-tenant message context resolver for HuanXing.
//!
//! Wraps [`TenantRouter`] to implement the generic
//! [`MessageContextResolver`] trait, mapping each (channel, sender_id)
//! pair to an isolated [`TenantContext`].

use std::sync::Arc;

use async_trait::async_trait;

use crate::channels::context_resolver::{MessageContext, MessageContextResolver};
use crate::huanxing::router::TenantRouter;

/// Multi-tenant resolver — delegates to [`TenantRouter`] for per-sender
/// context isolation.
pub struct MultiTenantResolver {
    router: Arc<TenantRouter>,
}

impl MultiTenantResolver {
    /// Wrap an existing [`TenantRouter`] as a [`MessageContextResolver`].
    pub fn new(router: Arc<TenantRouter>) -> Self {
        Self { router }
    }

    /// Access the underlying router (e.g. for registration/admin ops).
    pub fn router(&self) -> &Arc<TenantRouter> {
        &self.router
    }
}

#[async_trait]
impl MessageContextResolver for MultiTenantResolver {
    async fn resolve(&self, channel: &str, sender_id: &str) -> MessageContext {
        let tenant = self.router.resolve(channel, sender_id).await;
        MessageContext {
            agent_id: tenant.agent_id.clone(),
            is_guardian: tenant.is_guardian,
            nickname: tenant.nickname.clone(),
            star_name: tenant.star_name.clone(),
            model: tenant.model.clone(),
            provider: tenant.provider.clone(),
            api_key: tenant.api_key.clone(),
            temperature: tenant.temperature,
            system_prompt: tenant.system_prompt.clone(),
            memory: Arc::clone(&tenant.memory),
            conversation_histories: Arc::clone(&tenant.conversation_histories),
            session_manager: tenant.session_manager.clone(),
            workspace_dir: tenant.workspace_dir.clone(),
            knowledge_graph: tenant.knowledge_graph.clone(),
            cross_knowledge_index: tenant.cross_knowledge_index.clone(),
            knowledge_config: tenant.knowledge_config.clone(),
            security: tenant.security.clone(),
            non_cli_excluded_tools: Some(tenant.non_cli_excluded_tools.clone()),
            compact_context: tenant.compact_context,
            max_tool_iterations: tenant.max_tool_iterations,
            max_history_messages: tenant.max_history_messages,
            message_timeout_secs: Some(tenant.message_timeout_secs),
            multimodal: Some(tenant.multimodal.clone()),
            reliability: Some(tenant.reliability.clone()),
        }
    }

    fn invalidate(&self, channel: &str, sender_id: &str) {
        self.router.invalidate(channel, sender_id);
    }

    fn is_multi_tenant(&self) -> bool {
        true
    }
}
