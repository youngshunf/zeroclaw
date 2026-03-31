//! Message context resolver abstraction.
//!
//! Provides a trait ([`MessageContextResolver`]) that decouples message
//! processing from context discovery.  The default implementation
//! ([`DefaultContextResolver`]) returns the global configuration, while
//! the `huanxing` feature supplies a multi-tenant implementation that
//! routes each sender to an isolated tenant context.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;

use crate::channels::session_backend::SessionBackend;
use crate::memory::Memory;

use crate::security::policy::SecurityPolicy;

// Re-export the conversation history map type used by channels/mod.rs.
pub use super::ConversationHistoryMap;

/// Runtime context for processing a single message.
///
/// Produced by [`MessageContextResolver::resolve`].  Contains all
/// per-tenant (or global) resources needed during a channel message
/// processing cycle.
#[derive(Clone)]
pub struct MessageContext {
    // ── Identity ────────────────────────────────────────
    /// Agent identifier (e.g. `"001-18611348367-finance"` or `"guardian"`).
    pub agent_id: String,

    /// Whether this context represents the guardian (unregistered users).
    pub is_guardian: bool,

    /// User display name.
    pub nickname: Option<String>,

    /// AI character name.
    pub star_name: Option<String>,

    // ── Model / Provider ────────────────────────────────
    /// LLM model name override.
    pub model: Option<String>,

    /// LLM provider name override.
    pub provider: Option<String>,

    /// Per-tenant API key.
    pub api_key: Option<String>,

    /// Temperature override.
    pub temperature: Option<f64>,

    // ── Prompts ─────────────────────────────────────────
    /// Fully-built system prompt.
    pub system_prompt: String,

    // ── Storage (isolated per tenant) ───────────────────
    /// Vector memory backend.
    pub memory: Arc<dyn Memory>,

    /// Conversation history map (per-sender keyed).
    pub conversation_histories: ConversationHistoryMap,

    /// Session persistence backend (JSONL / SQLite).
    pub session_manager: Option<Arc<dyn SessionBackend>>,

    /// Workspace directory.
    pub workspace_dir: PathBuf,

    /// Per-tenant knowledge graph instance.
    /// On desktop: shared across all agents via owner_dir.
    /// On cloud: isolated per-agent workspace.
    /// None when knowledge graph is disabled.
    pub knowledge_graph: Option<Arc<crate::memory::knowledge_graph::KnowledgeGraph>>,

    /// Multi-agent cross-workspace knowledge index.
    pub cross_knowledge_index: Option<Arc<crate::memory::knowledge_cross::CrossWorkspaceKnowledgeIndex>>,

    /// Knowledge graph configuration (for auto_capture, suggest_on_query, etc.).
    pub knowledge_config: crate::config::KnowledgeConfig,

    // ── Security ────────────────────────────────────────
    /// Per-tenant security policy for shell / file tools.
    pub security: Option<Arc<SecurityPolicy>>,

    // ── Per-tenant runtime overrides ────────────────────
    /// Tools excluded from non-CLI channels for this tenant.
    pub non_cli_excluded_tools: Option<Vec<String>>,

    /// Override compact context config for this tenant.
    pub compact_context: Option<bool>,

    /// Override max tool iterations config for this tenant.
    pub max_tool_iterations: Option<usize>,

    /// Override max history messages config for this tenant.
    pub max_history_messages: Option<usize>,

    /// Message timeout (seconds) for this tenant.
    pub message_timeout_secs: Option<u64>,

    /// Multimodal config override for this tenant.
    pub multimodal: Option<crate::config::MultimodalConfig>,

    /// Per-tenant reliability config (from workspace [reliability] or global).
    /// Used to create the per-request resilient provider with tenant's own keys/fallbacks.
    pub reliability: Option<crate::config::ReliabilityConfig>,
}

/// Resolves the runtime [`MessageContext`] for an incoming message.
///
/// Implementations must be cheaply cloneable (wrapped in `Arc`).
#[async_trait]
pub trait MessageContextResolver: Send + Sync {
    /// Resolve context for the given channel + sender.
    ///
    /// Called once per inbound message at the start of the processing
    /// pipeline.  The returned [`MessageContext`] contains all resources
    /// (model, memory, history, session backend, etc.) needed for the
    /// rest of the pipeline.
    async fn resolve(&self, channel: &str, sender_id: &str) -> MessageContext;

    /// Invalidate cached context for a specific sender.
    ///
    /// Called after registration, config changes, or agent deletion so
    /// the next `resolve()` rebuilds the context from source.
    fn invalidate(&self, _channel: &str, _sender_id: &str) {}

    /// Whether the resolver operates in multi-tenant mode.
    fn is_multi_tenant(&self) -> bool {
        false
    }
}

/// Default resolver — returns a fixed global context for every sender.
///
/// Used when no multi-tenant feature is active.
pub struct DefaultContextResolver {
    ctx: MessageContext,
}

impl DefaultContextResolver {
    /// Create a new default resolver wrapping the given global context.
    pub fn new(ctx: MessageContext) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl MessageContextResolver for DefaultContextResolver {
    async fn resolve(&self, _channel: &str, _sender_id: &str) -> MessageContext {
        self.ctx.clone()
    }
}
