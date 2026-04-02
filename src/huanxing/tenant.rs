//! Per-tenant agent context.
//!
//! A [`TenantContext`] carries the per-user overrides that customize the shared
//! agent loop: system prompt, workspace directory, model, provider, tool
//! filter, memory, session manager, and conversation histories.
//!
//! The shared [`ChannelRuntimeContext`] provides channels, LLM pool,
//! and base tools — tenant context overrides the user-facing subset.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::Context;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::channels::session_backend::SessionBackend;
use crate::memory::{self, Memory};

use crate::security::SecurityPolicy;

// ── Workspace config.toml partial overlay ────────────────────
//
// Agent workspaces may contain a `config.toml` written at registration time.
// We parse all fields that are meaningful for per-tenant override;
// everything else is inherited from the global daemon config.

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct WorkspaceOverrides {
    api_key: Option<String>,
    default_provider: Option<String>,
    default_model: Option<String>,
    default_temperature: Option<f64>,
    #[serde(default)]
    agent: AgentOverrides,
    /// [memory] 节覆盖全局记忆配置（embedding_provider / vector_weight 等）
    #[serde(default)]
    memory: Option<toml::Table>,
    /// [knowledge] 节覆盖全局知识图谱配置
    #[serde(default)]
    knowledge: Option<toml::Table>,
    /// [autonomy] 节覆盖全局安全策略（allowed_commands / forbidden_paths / non_cli_excluded_tools 等）
    #[serde(default)]
    autonomy: Option<toml::Table>,
    /// [skills] 节覆盖技能注入模式等配置
    #[serde(default)]
    skills: Option<toml::Table>,
    /// [security] 节覆盖安全配置（canary_tokens / outbound_leak_guard 等）
    #[serde(default)]
    security: Option<toml::Table>,
    /// [channels_config] 节覆盖渠道配置
    #[serde(default)]
    channels_config: Option<toml::Table>,
    /// [heartbeat] 节覆盖心跳配置
    #[serde(default)]
    heartbeat: Option<toml::Table>,
    /// [cron] 节覆盖定时任务配置
    #[serde(default)]
    cron: Option<toml::Table>,
    /// [multimodal] 节覆盖多模态配置
    #[serde(default)]
    multimodal: Option<toml::Table>,
    /// [web_search] 节覆盖搜索配置
    #[serde(default)]
    web_search: Option<toml::Table>,
    /// [web_fetch] 节覆盖网页抓取配置
    #[serde(default)]
    web_fetch: Option<toml::Table>,
    /// [browser] 节覆盖浏览器配置
    #[serde(default)]
    browser: Option<toml::Table>,
    /// [http_request] 节覆盖 HTTP 请求配置
    #[serde(default)]
    http_request: Option<toml::Table>,
    /// [reliability] 节覆盖可靠性配置（fallback_providers / model_fallbacks / api_keys 等）
    #[serde(default)]
    reliability: Option<toml::Table>,
    /// [sop] 节覆盖 SOP 工作流引擎配置（sops_dir / default_execution_mode 等）
    #[serde(default)]
    sop: Option<toml::Table>,
    /// Catch-all for unknown sections in config.toml (e.g. [proxy], [composio], [mcp]).
    /// Prevents serde from failing on unrecognized fields.
    #[serde(flatten)]
    _extra: std::collections::HashMap<String, toml::Value>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct AgentOverrides {
    session: Option<serde_json::Value>,
    compact_context: Option<bool>,
    max_tool_iterations: Option<usize>,
    max_history_messages: Option<usize>,
    /// Catch-all for unknown fields.
    #[serde(flatten)]
    _extra: std::collections::HashMap<String, toml::Value>,
}

/// Type alias matching channels/mod.rs ConversationHistoryMap.
pub use crate::channels::ConversationHistoryMap;

/// Per-tenant agent context. Loaded from DB + workspace on first message,
/// then cached in [`TenantRouter`].
pub struct TenantContext {
    /// Agent ID (e.g. "001-18611348367-finance").
    pub agent_id: String,

    /// User ID (UUID from users table).
    pub user_id: String,

    /// Workspace directory for this tenant.
    /// Contains SOUL.md, USER.md, memory/, sessions.db, cron/, etc.
    pub workspace_dir: PathBuf,

    /// Owner directory for this tenant.
    /// In desktop, this points to ~/.huanxing/ for global memory and USER.md.
    pub owner_dir: PathBuf,

    /// Fully-built system prompt (SOUL.md + AGENTS.md + USER.md + BOOTSTRAP.md + MEMORY.md + skills).
    /// Constructed via `build_system_prompt()`, not just SOUL.md raw text.
    pub system_prompt: String,

    /// Model to use for this tenant (e.g. "deepseek-chat").
    pub model: Option<String>,

    /// Provider to use (e.g. "deepseek", "openrouter").
    pub provider: Option<String>,

    /// Template name (e.g. "finance").
    pub template: Option<String>,

    /// User's display name.
    pub nickname: Option<String>,

    /// Custom AI character name.
    pub star_name: Option<String>,

    /// Subscription plan.
    pub plan: Option<String>,

    /// Per-tenant temperature override (from workspace config.toml).
    pub temperature: Option<f64>,

    /// Override compact context setting.
    pub compact_context: Option<bool>,

    /// Override max tool iterations setting.
    pub max_tool_iterations: Option<usize>,

    /// Override max history messages setting.
    pub max_history_messages: Option<usize>,

    /// Per-tenant API key (from workspace config.toml, e.g. user-specific LLM token).
    pub api_key: Option<String>,

    /// Whether this is the guardian (unregistered users) context.
    pub is_guardian: bool,

    /// Per-tenant vector memory (brain.db in tenant workspace).
    pub memory: Arc<dyn Memory>,

    /// Per-tenant session persistence backend (JSONL or SQLite, based on config).
    pub session_manager: Option<Arc<dyn SessionBackend>>,

    /// Per-tenant conversation histories (isolated from other tenants).
    pub conversation_histories: ConversationHistoryMap,

    /// Per-tenant security policy built from workspace config.toml [autonomy] section.
    /// Overrides the global SecurityPolicy for this tenant's shell/file tool calls.
    /// None means no workspace-level override — tools fall back to global policy.
    pub security: Option<Arc<SecurityPolicy>>,

    /// Per-tenant non_cli_excluded_tools (from [autonomy] override).
    /// Tools listed here are hidden from non-CLI channels (QQ/飞书 etc).
    pub non_cli_excluded_tools: Vec<String>,

    /// Per-tenant heartbeat config (from [heartbeat] override or global).
    pub heartbeat: crate::config::HeartbeatConfig,

    /// Per-tenant cron config (from [cron] override or global).
    pub cron: crate::config::CronConfig,

    /// Per-tenant multimodal config (from [multimodal] override or global).
    pub multimodal: crate::config::MultimodalConfig,

    /// Per-tenant message timeout (from [channels_config] override or global).
    pub message_timeout_secs: u64,

    /// Per-tenant reliability config (from [reliability] override or global).
    /// Used when creating per-request resilient providers.
    pub reliability: crate::config::ReliabilityConfig,

    /// Per-tenant knowledge graph instance.
    /// In unified tenant mode this is typically shared via owner_dir.
    pub knowledge_graph: Option<std::sync::Arc<crate::memory::knowledge_graph::KnowledgeGraph>>,

    /// Multi-agent cross-workspace knowledge index.
    pub cross_knowledge_index:
        Option<std::sync::Arc<crate::memory::knowledge_cross::CrossWorkspaceKnowledgeIndex>>,

    /// Per-tenant knowledge config (for auto_capture, suggest_on_query, etc.).
    pub knowledge_config: crate::config::KnowledgeConfig,

    /// Effective runtime config after tenant-level path and config resolution.
    resolved_config: crate::config::Config,

    /// Global skills directory (Level 1: `{config_dir}/skills/`).
    pub global_skills_dir: Option<PathBuf>,

    /// User/tenant skills directory (Level 2: `{config_dir}/users/{td}/workspace/skills/`).
    pub user_skills_dir: Option<PathBuf>,
}

// Manual Debug impl because Arc<dyn Memory> doesn't impl Debug.
impl std::fmt::Debug for TenantContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TenantContext")
            .field("agent_id", &self.agent_id)
            .field("user_id", &self.user_id)
            .field("workspace_dir", &self.workspace_dir)
            .field("owner_dir", &self.owner_dir)
            .field("model", &self.model)
            .field("provider", &self.provider)
            .field("template", &self.template)
            .field("nickname", &self.nickname)
            .field("star_name", &self.star_name)
            .field("plan", &self.plan)
            .field("temperature", &self.temperature)
            .field("has_api_key", &self.api_key.is_some())
            .field("is_guardian", &self.is_guardian)
            .field("has_memory", &true)
            .field("has_session_manager", &self.session_manager.is_some())
            .field(
                "non_cli_excluded_tools_count",
                &self.non_cli_excluded_tools.len(),
            )
            .field("message_timeout_secs", &self.message_timeout_secs)
            .field(
                "has_reliability_fallbacks",
                &!self.reliability.fallback_providers.is_empty(),
            )
            .field("has_knowledge_graph", &self.knowledge_graph.is_some())
            .field("knowledge_enabled", &self.knowledge_config.enabled)
            .finish()
    }
}

impl WorkspaceOverrides {
    fn normalize(mut self) -> Self {
        self.api_key = normalize_non_empty_string(self.api_key);
        self.default_provider = normalize_non_empty_string(self.default_provider);
        self.default_model = normalize_non_empty_string(self.default_model);
        self
    }
}

impl AgentOverrides {
    fn as_partial_table(&self) -> Option<toml::Table> {
        let has_known_field = self.session.is_some()
            || self.compact_context.is_some()
            || self.max_tool_iterations.is_some()
            || self.max_history_messages.is_some();
        if has_known_field || !self._extra.is_empty() {
            toml::Value::try_from(self)
                .ok()
                .and_then(|value| value.as_table().cloned())
        } else {
            None
        }
    }
}

impl TenantContext {
    fn config_dir(global_config: &crate::config::Config) -> PathBuf {
        global_config
            .config_path
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| global_config.workspace_dir.clone())
    }

    async fn open_db(
        global_config: &crate::config::Config,
    ) -> anyhow::Result<crate::huanxing::db::TenantDb> {
        let config_dir = Self::config_dir(global_config);
        let db_path = global_config.huanxing.resolve_db_path(&config_dir);
        Ok(crate::huanxing::db::TenantDb::open(&db_path)?)
    }

    pub async fn load_from_record(
        record: &crate::huanxing::db::TenantRecord,
        global_config: &crate::config::Config,
    ) -> anyhow::Result<Self> {
        let config_dir = Self::config_dir(global_config);
        let tenant_dir = record.tenant_dir.as_deref();
        let owner_dir = global_config
            .huanxing
            .resolve_owner_dir(&config_dir, tenant_dir);
        let agent_workspace = global_config.huanxing.resolve_agent_workspace(
            &config_dir,
            tenant_dir,
            &record.agent_id,
        );

        Self::load(
            &record.agent_id,
            &record.user_id,
            agent_workspace,
            owner_dir,
            global_config.huanxing.default_model.clone(),
            global_config.huanxing.default_provider.clone(),
            record.template.clone(),
            record.nickname.clone(),
            record.star_name.clone(),
            record.plan.clone(),
            global_config,
        )
        .await
    }

    pub async fn load_by_agent_id(
        global_config: &crate::config::Config,
        agent_id: &str,
    ) -> anyhow::Result<Option<Self>> {
        if !global_config.huanxing.enabled {
            return Ok(None);
        }

        let db = Self::open_db(global_config).await?;
        let Some(record) = db.find_by_agent_id(agent_id).await? else {
            return Ok(None);
        };
        Ok(Some(Self::load_from_record(&record, global_config).await?))
    }

    pub async fn load_by_hasn_id(
        global_config: &crate::config::Config,
        hasn_id: &str,
    ) -> anyhow::Result<Option<Self>> {
        if !global_config.huanxing.enabled {
            return Ok(None);
        }

        let db = Self::open_db(global_config).await?;
        let Some(record) = db.find_by_hasn_id(hasn_id).await? else {
            return Ok(None);
        };
        Ok(Some(Self::load_from_record(&record, global_config).await?))
    }

    pub async fn load_by_agent_or_hasn(
        global_config: &crate::config::Config,
        lookup: &str,
    ) -> anyhow::Result<Option<Self>> {
        if let Some(ctx) = Self::load_by_agent_id(global_config, lookup).await? {
            return Ok(Some(ctx));
        }
        Self::load_by_hasn_id(global_config, lookup).await
    }

    pub(crate) fn runtime_config(&self) -> &crate::config::Config {
        &self.resolved_config
    }

    pub async fn create_agent(&self) -> anyhow::Result<crate::agent::Agent> {
        crate::agent::Agent::from_tenant_context(self).await
    }

    /// Load a tenant context from workspace directory.
    ///
    /// Builds the full system prompt from workspace files (SOUL.md, AGENTS.md,
    /// USER.md, BOOTSTRAP.md, MEMORY.md, skills/), creates per-tenant memory
    /// and session manager instances.
    pub async fn load(
        agent_id: &str,
        user_id: &str,
        workspace_dir: PathBuf,
        owner_dir: PathBuf,
        model: Option<String>,
        provider: Option<String>,
        template: Option<String>,
        nickname: Option<String>,
        star_name: Option<String>,
        plan: Option<String>,
        global_config: &crate::config::Config,
    ) -> anyhow::Result<Self> {
        // ── 0. Load workspace config.toml overrides ──────────────────
        //
        // Config cascading:
        //   Cloud  (3-level): global config.toml → user config.toml → agent config.toml
        //   Desktop (2-level): global config.toml → agent config.toml
        //
        // For desktop-created tenants, owner/agent api_key may be empty by design.
        // In that case, the effective LLM credential still comes from the global config.
        //
        // The agent-level config.toml lives at agent_wrapper_dir/config.toml
        // (i.e. the parent of workspace_dir, since workspace_dir = wrapper/workspace/).
        // For backward compat, we also check workspace_dir/config.toml directly.
        let agent_wrapper_dir = workspace_dir.parent().unwrap_or(&workspace_dir);
        let overrides =
            load_cascaded_overrides(&owner_dir, agent_wrapper_dir, &workspace_dir).await;

        // Effective model/provider: workspace config > DB record > global [huanxing] default
        let effective_model = overrides.default_model.clone().or(model.clone());
        let effective_provider = overrides.default_provider.clone().or(provider.clone());
        let effective_api_key = overrides
            .api_key
            .clone()
            .or_else(|| global_config.api_key.clone());
        let effective_temperature = overrides.default_temperature;
        let agent_override_table = overrides.agent.as_partial_table();
        let effective_agent_config =
            merge_config_section(&global_config.agent, agent_override_table.as_ref())
                .context("merge [agent] overrides")?;
        let effective_memory_config =
            merge_config_section(&global_config.memory, overrides.memory.as_ref())
                .context("merge [memory] overrides")?;
        let effective_knowledge_config =
            merge_config_section(&global_config.knowledge, overrides.knowledge.as_ref())
                .context("merge [knowledge] overrides")?;
        let effective_autonomy =
            merge_config_section(&global_config.autonomy, overrides.autonomy.as_ref())
                .context("merge [autonomy] overrides")?;
        let effective_skills_config =
            merge_config_section(&global_config.skills, overrides.skills.as_ref())
                .context("merge [skills] overrides")?;
        let effective_security_config =
            merge_config_section(&global_config.security, overrides.security.as_ref())
                .context("merge [security] overrides")?;
        let effective_channels_config = merge_config_section(
            &global_config.channels_config,
            overrides.channels_config.as_ref(),
        )
        .context("merge [channels_config] overrides")?;
        let effective_heartbeat =
            merge_config_section(&global_config.heartbeat, overrides.heartbeat.as_ref())
                .context("merge [heartbeat] overrides")?;
        let effective_cron = merge_config_section(&global_config.cron, overrides.cron.as_ref())
            .context("merge [cron] overrides")?;
        let effective_multimodal =
            merge_config_section(&global_config.multimodal, overrides.multimodal.as_ref())
                .context("merge [multimodal] overrides")?;
        let effective_web_search =
            merge_config_section(&global_config.web_search, overrides.web_search.as_ref())
                .context("merge [web_search] overrides")?;
        let effective_web_fetch =
            merge_config_section(&global_config.web_fetch, overrides.web_fetch.as_ref())
                .context("merge [web_fetch] overrides")?;
        let effective_browser =
            merge_config_section(&global_config.browser, overrides.browser.as_ref())
                .context("merge [browser] overrides")?;
        let effective_http_request =
            merge_config_section(&global_config.http_request, overrides.http_request.as_ref())
                .context("merge [http_request] overrides")?;
        let effective_sop = merge_config_section(&global_config.sop, overrides.sop.as_ref())
            .context("merge [sop] overrides")?;

        // ── A. Build full system prompt from workspace files ──────────
        let model_name = effective_model
            .as_deref()
            .or(global_config.default_model.as_deref())
            .unwrap_or("claude-sonnet-4-6");
        let mut skills_config_for_load = global_config.clone();
        skills_config_for_load.skills = effective_skills_config.clone();

        // Load skills from tenant workspace + common skills directory
        // using three-level cascade: global → user → agent
        let config_dir = global_config
            .config_path
            .parent()
            .unwrap_or(&global_config.workspace_dir);
        let common_skills_dir = global_config.huanxing.resolve_common_skills_dir(config_dir);

        // Derive user/tenant skills directory from workspace path:
        //   workspace_dir = {config_dir}/users/{td}/agents/{id}/workspace/
        //   user_skills_dir = {config_dir}/users/{td}/workspace/skills/
        let user_skills_dir = workspace_dir
            .parent() // agents/{id}
            .and_then(|p| p.parent()) // agents/
            .and_then(|p| p.parent()) // users/{td}
            .map(|tenant_root| tenant_root.join("workspace").join("skills"));

        // Global skills dir is the common_skills_dir/skills/ (platform-wide shared)
        let global_skills_dir = {
            let d = common_skills_dir.join("skills");
            if d.exists() { Some(d) } else if common_skills_dir.exists() { Some(common_skills_dir.clone()) } else { None }
        };

        let skills = crate::skills::load_skills_cascaded(
            global_skills_dir.as_deref(),
            user_skills_dir.as_deref(),
            &workspace_dir,
            &skills_config_for_load,
        );

        let all_skill_names: Vec<String> = skills.iter().map(|s| s.name.clone()).collect();
        tracing::info!(
            agent_id,
            total = skills.len(),
            names = ?all_skill_names,
            global_skills = ?global_skills_dir,
            user_skills = ?user_skills_dir,
            "【技能调试】三级级联加载技能完成"
        );

        let tool_descs: Vec<(&str, &str)> = Vec::new();

        // Resolve skills prompt injection mode: workspace [skills] > global [skills]
        let skills_prompt_mode = effective_skills_config.prompt_injection_mode;

        // Resolve autonomy level: workspace [autonomy] > global [autonomy]
        let autonomy_level = effective_autonomy.level.clone();

        let system_prompt = crate::channels::build_system_prompt_with_mode(
            &workspace_dir,
            &owner_dir,
            model_name,
            &tool_descs,
            &skills,
            Some(&global_config.identity),
            None,
            false,
            skills_prompt_mode,
            autonomy_level,
        );

        let has_skills_section = system_prompt.contains("<available_skills>");
        tracing::info!(
            agent_id,
            prompt_len = system_prompt.len(),
            skills_count = skills.len(),
            has_skills_section,
            ?skills_prompt_mode,
            has_workspace_skills_override = overrides.skills.is_some(),
            global_skills_mode = ?global_config.skills.prompt_injection_mode,
            "【技能调试】系统提示词构建完成"
        );

        // ── B. Create per-tenant memory ──────────────────────────────
        let tenant_memory: Arc<dyn Memory> = Arc::from(memory::create_memory(
            &effective_memory_config,
            &owner_dir,
            effective_api_key.as_deref(),
        )?);

        // ── B2. Create per-tenant knowledge graph ────────────────────
        let knowledge_graph: Option<
            std::sync::Arc<crate::memory::knowledge_graph::KnowledgeGraph>,
        > = if effective_knowledge_config.enabled {
            let kb_path = if effective_knowledge_config.db_path.starts_with('/')
                || effective_knowledge_config.db_path.starts_with('~')
            {
                // Absolute/home path: backward compat
                let expanded = effective_knowledge_config.db_path.replace(
                    '~',
                    &directories::UserDirs::new()
                        .map(|u| u.home_dir().to_string_lossy().to_string())
                        .unwrap_or_else(|| ".".to_string()),
                );
                std::path::PathBuf::from(expanded)
            } else {
                // Relative path: resolve against owner_dir
                // Desktop: owner_dir = ~/.huanxing/ → global shared
                // Cloud:   owner_dir = workspace_dir → per-agent isolated
                owner_dir.join(&effective_knowledge_config.db_path)
            };
            // Ensure parent directory exists
            if let Some(parent) = kb_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match crate::memory::knowledge_graph::KnowledgeGraph::new(
                &kb_path,
                effective_knowledge_config.max_nodes,
            ) {
                Ok(g) => {
                    tracing::info!(
                        agent_id,
                        db_path = %kb_path.display(),
                        "Knowledge graph initialized for tenant",
                    );
                    Some(std::sync::Arc::new(g))
                }
                Err(e) => {
                    tracing::warn!(
                        agent_id,
                        db_path = %kb_path.display(),
                        "Knowledge graph init failed: {e}",
                    );
                    None
                }
            }
        } else {
            None
        };

        // ── B3. Create multi-agent cross-workspace knowledge index ────────────
        let cross_knowledge_index = if effective_knowledge_config.enabled
            && effective_knowledge_config.cross_workspace_search
            && owner_dir == workspace_dir
        {
            if let Some(agents_dir) =
                crate::memory::knowledge_cross::agents_dir_from_workspace(&workspace_dir)
            {
                let max_nodes = effective_knowledge_config.max_nodes;
                Some(std::sync::Arc::new(
                    tokio::task::spawn_blocking(move || {
                        crate::memory::knowledge_cross::CrossWorkspaceKnowledgeIndex::discover(
                            &agents_dir,
                            max_nodes,
                        )
                    })
                    .await?,
                ))
            } else {
                None
            }
        } else {
            if effective_knowledge_config.enabled
                && effective_knowledge_config.cross_workspace_search
                && owner_dir != workspace_dir
            {
                tracing::debug!(
                    agent_id,
                    owner_dir = %owner_dir.display(),
                    workspace_dir = %workspace_dir.display(),
                    "Skip cross-workspace knowledge index because tenant knowledge is already shared via owner_dir",
                );
            }
            None
        };

        // ── C. Session backend (JSONL or SQLite based on channels_config) ──
        let tenant_session_manager: Option<Arc<dyn SessionBackend>> =
            create_session_backend(&workspace_dir, &effective_channels_config);

        // ── D. Independent conversation histories ────────────────────
        let conversation_histories: ConversationHistoryMap =
            Arc::new(Mutex::new(lru::LruCache::new(
                std::num::NonZeroUsize::new(crate::channels::MAX_CONVERSATION_SENDERS).unwrap(),
            )));

        // ── E. Per-tenant security policy from [autonomy] in workspace config.toml ──
        // 以全局 autonomy 为基础，用 workspace config.toml 中的 [autonomy] 节覆盖。
        // 若 workspace 没有 [autonomy] 节，则 tenant_security = None（工具回落到全局策略）。
        let tenant_security: Option<Arc<SecurityPolicy>> = overrides.autonomy.as_ref().map(|_| {
            Arc::new(SecurityPolicy::from_config(
                &effective_autonomy,
                &workspace_dir,
            ))
        });

        // ── F. Resolve remaining per-tenant config overrides ─────────
        let effective_non_cli_excluded = effective_autonomy.non_cli_excluded_tools.clone();
        let effective_message_timeout = effective_channels_config.message_timeout_secs;

        // ── G. Resolve per-tenant reliability ────────────────────────
        // 优先使用 workspace [reliability]；若租户有自己的 api_key 且 reliability.api_keys 为空，
        // 自动注入，使 fallback_providers 也能使用租户自己的 key。
        let mut effective_reliability =
            merge_config_section(&global_config.reliability, overrides.reliability.as_ref())
                .context("merge [reliability] overrides")?;
        if let Some(ref key) = effective_api_key {
            if effective_reliability.api_keys.is_empty() {
                effective_reliability.api_keys = vec![key.clone()];
            }
        }

        let mut resolved_config = global_config.clone();
        resolved_config.workspace_dir = workspace_dir.clone();
        resolved_config.default_model = effective_model.clone();
        resolved_config.default_provider = effective_provider.clone();
        resolved_config.default_temperature =
            effective_temperature.unwrap_or(global_config.default_temperature);
        resolved_config.api_key = effective_api_key.clone();
        resolved_config.agent = effective_agent_config;
        resolved_config.memory = effective_memory_config.clone();
        resolved_config.knowledge = effective_knowledge_config.clone();
        resolved_config.autonomy = effective_autonomy;
        resolved_config.skills = effective_skills_config;
        resolved_config.security = effective_security_config;
        resolved_config.channels_config = effective_channels_config.clone();
        resolved_config.heartbeat = effective_heartbeat.clone();
        resolved_config.cron = effective_cron.clone();
        resolved_config.multimodal = effective_multimodal.clone();
        resolved_config.web_search = effective_web_search;
        resolved_config.web_fetch = effective_web_fetch;
        resolved_config.browser = effective_browser;
        resolved_config.http_request = effective_http_request;
        resolved_config.reliability = effective_reliability.clone();
        resolved_config.sop = effective_sop;

        Ok(Self {
            agent_id: agent_id.to_string(),
            user_id: user_id.to_string(),
            workspace_dir,
            owner_dir,
            system_prompt,
            model: effective_model,
            provider: effective_provider,
            template,
            nickname,
            star_name,
            plan,
            temperature: effective_temperature,
            compact_context: overrides.agent.compact_context,
            max_tool_iterations: overrides.agent.max_tool_iterations,
            max_history_messages: overrides.agent.max_history_messages,
            api_key: effective_api_key,
            is_guardian: false,
            memory: tenant_memory,
            session_manager: tenant_session_manager,
            conversation_histories,
            security: tenant_security,
            non_cli_excluded_tools: effective_non_cli_excluded,
            heartbeat: effective_heartbeat,
            cron: effective_cron,
            multimodal: effective_multimodal,
            message_timeout_secs: effective_message_timeout,
            reliability: effective_reliability,
            knowledge_graph,
            cross_knowledge_index,
            knowledge_config: effective_knowledge_config,
            resolved_config,
            global_skills_dir,
            user_skills_dir,
        })
    }

    /// Create the guardian context for unregistered users.
    pub async fn guardian(
        workspace_dir: PathBuf,
        global_config: &crate::config::Config,
    ) -> anyhow::Result<Self> {
        // Ensure guardian workspace exists
        tokio::fs::create_dir_all(&workspace_dir).await?;

        // Load workspace config.toml overrides (guardian may have one too)
        let overrides = load_workspace_overrides(&workspace_dir).await;

        let effective_api_key = overrides
            .api_key
            .clone()
            .or_else(|| global_config.api_key.clone());
        let agent_override_table = overrides.agent.as_partial_table();
        let effective_agent_config =
            merge_config_section(&global_config.agent, agent_override_table.as_ref())
                .context("merge guardian [agent] overrides")?;
        let effective_memory_config =
            merge_config_section(&global_config.memory, overrides.memory.as_ref())
                .context("merge guardian [memory] overrides")?;
        let effective_autonomy =
            merge_config_section(&global_config.autonomy, overrides.autonomy.as_ref())
                .context("merge guardian [autonomy] overrides")?;
        let effective_skills_config =
            merge_config_section(&global_config.skills, overrides.skills.as_ref())
                .context("merge guardian [skills] overrides")?;
        let effective_security_config =
            merge_config_section(&global_config.security, overrides.security.as_ref())
                .context("merge guardian [security] overrides")?;
        let effective_channels_config = merge_config_section(
            &global_config.channels_config,
            overrides.channels_config.as_ref(),
        )
        .context("merge guardian [channels_config] overrides")?;
        let effective_heartbeat =
            merge_config_section(&global_config.heartbeat, overrides.heartbeat.as_ref())
                .context("merge guardian [heartbeat] overrides")?;
        let effective_cron = merge_config_section(&global_config.cron, overrides.cron.as_ref())
            .context("merge guardian [cron] overrides")?;
        let effective_multimodal =
            merge_config_section(&global_config.multimodal, overrides.multimodal.as_ref())
                .context("merge guardian [multimodal] overrides")?;
        let effective_web_search =
            merge_config_section(&global_config.web_search, overrides.web_search.as_ref())
                .context("merge guardian [web_search] overrides")?;
        let effective_web_fetch =
            merge_config_section(&global_config.web_fetch, overrides.web_fetch.as_ref())
                .context("merge guardian [web_fetch] overrides")?;
        let effective_browser =
            merge_config_section(&global_config.browser, overrides.browser.as_ref())
                .context("merge guardian [browser] overrides")?;
        let effective_http_request =
            merge_config_section(&global_config.http_request, overrides.http_request.as_ref())
                .context("merge guardian [http_request] overrides")?;
        let effective_sop = merge_config_section(&global_config.sop, overrides.sop.as_ref())
            .context("merge guardian [sop] overrides")?;

        let model_name = overrides
            .default_model
            .as_deref()
            .or(global_config.default_model.as_deref())
            .unwrap_or("claude-sonnet-4-6");
        let mut skills_config_for_load = global_config.clone();
        skills_config_for_load.skills = effective_skills_config.clone();

        // Load skills from guardian workspace + common skills directory
        let config_dir = global_config
            .config_path
            .parent()
            .unwrap_or(&global_config.workspace_dir);
        let common_skills_dir = global_config.huanxing.resolve_common_skills_dir(config_dir);
        let global_skills_dir = {
            let d = common_skills_dir.join("skills");
            if d.exists() { Some(d) } else if common_skills_dir.exists() { Some(common_skills_dir.clone()) } else { None }
        };
        let skills = crate::skills::load_skills_cascaded(
            global_skills_dir.as_deref(),
            None, // Guardian has no user-level skills
            &workspace_dir,
            &skills_config_for_load,
        );

        let tool_descs: Vec<(&str, &str)> = Vec::new();

        // Resolve skills prompt injection mode: workspace [skills] > global [skills]
        let guardian_skills_mode = effective_skills_config.prompt_injection_mode;

        let guardian_autonomy_level = effective_autonomy.level.clone();

        // Build full system prompt from guardian workspace files
        let system_prompt = if workspace_dir.join("SOUL.md").exists() {
            crate::channels::build_system_prompt_with_mode(
                &workspace_dir,
                &workspace_dir,
                model_name,
                &tool_descs,
                &skills,
                Some(&global_config.identity),
                None,
                false,
                guardian_skills_mode,
                guardian_autonomy_level,
            )
        } else {
            default_guardian_prompt()
        };

        // Per-tenant memory for guardian
        let guardian_memory: Arc<dyn Memory> = Arc::from(memory::create_memory(
            &effective_memory_config,
            &workspace_dir,
            effective_api_key.as_deref(),
        )?);

        // Per-tenant session backend for guardian
        let guardian_session_manager: Option<Arc<dyn SessionBackend>> =
            create_session_backend(&workspace_dir, &effective_channels_config);

        let conversation_histories: ConversationHistoryMap =
            Arc::new(Mutex::new(lru::LruCache::new(
                std::num::NonZeroUsize::new(crate::channels::MAX_CONVERSATION_SENDERS).unwrap(),
            )));

        let guardian_security: Option<Arc<SecurityPolicy>> =
            overrides.autonomy.as_ref().map(|_| {
                Arc::new(SecurityPolicy::from_config(
                    &effective_autonomy,
                    &workspace_dir,
                ))
            });

        let effective_non_cli_excluded = effective_autonomy.non_cli_excluded_tools.clone();

        let effective_message_timeout = effective_channels_config.message_timeout_secs;

        // ── G. Resolve guardian reliability ──────────────────────────
        let mut effective_reliability =
            merge_config_section(&global_config.reliability, overrides.reliability.as_ref())
                .context("merge guardian [reliability] overrides")?;
        if let Some(ref key) = effective_api_key {
            if effective_reliability.api_keys.is_empty() {
                effective_reliability.api_keys = vec![key.clone()];
            }
        }

        let mut resolved_config = global_config.clone();
        resolved_config.workspace_dir = workspace_dir.clone();
        resolved_config.default_model = normalize_non_empty_string(overrides.default_model.clone());
        resolved_config.default_provider =
            normalize_non_empty_string(overrides.default_provider.clone());
        resolved_config.default_temperature = overrides
            .default_temperature
            .unwrap_or(global_config.default_temperature);
        resolved_config.api_key = effective_api_key.clone();
        resolved_config.agent = effective_agent_config;
        resolved_config.memory = effective_memory_config.clone();
        resolved_config.autonomy = effective_autonomy;
        resolved_config.skills = effective_skills_config;
        resolved_config.security = effective_security_config;
        resolved_config.channels_config = effective_channels_config.clone();
        resolved_config.heartbeat = effective_heartbeat.clone();
        resolved_config.cron = effective_cron.clone();
        resolved_config.multimodal = effective_multimodal.clone();
        resolved_config.web_search = effective_web_search;
        resolved_config.web_fetch = effective_web_fetch;
        resolved_config.browser = effective_browser;
        resolved_config.http_request = effective_http_request;
        resolved_config.reliability = effective_reliability.clone();
        resolved_config.sop = effective_sop;

        Ok(Self {
            agent_id: "guardian".to_string(),
            user_id: String::new(),
            workspace_dir: workspace_dir.clone(),
            owner_dir: workspace_dir,
            system_prompt,
            model: overrides.default_model,
            provider: overrides.default_provider,
            template: None,
            nickname: None,
            star_name: None,
            plan: None,
            temperature: overrides.default_temperature,
            compact_context: overrides.agent.compact_context,
            max_tool_iterations: overrides.agent.max_tool_iterations,
            max_history_messages: overrides.agent.max_history_messages,
            api_key: effective_api_key,
            is_guardian: true,
            memory: guardian_memory,
            session_manager: guardian_session_manager,
            conversation_histories,
            security: guardian_security,
            non_cli_excluded_tools: effective_non_cli_excluded,
            heartbeat: effective_heartbeat,
            cron: effective_cron,
            multimodal: effective_multimodal,
            message_timeout_secs: effective_message_timeout,
            reliability: effective_reliability,
            knowledge_graph: None,
            cross_knowledge_index: None,
            knowledge_config: Default::default(),
            resolved_config,
            global_skills_dir,
            user_skills_dir: None,
        })
    }

    /// Create the admin agent context.
    /// Admin agent handles server management, routed from admin-designated channels.
    pub async fn admin(
        workspace_dir: PathBuf,
        global_config: &crate::config::Config,
    ) -> anyhow::Result<Self> {
        // Reuse the guardian() constructor logic, then patch agent_id and is_guardian.
        let mut ctx = Self::guardian(workspace_dir, global_config).await?;
        ctx.agent_id = "admin".to_string();
        ctx.is_guardian = false; // Admin is not guardian — it has its own tool permissions
        Ok(ctx)
    }
}

// ── Helpers ──────────────────────────────────────────────────

fn normalize_non_empty_string(value: Option<String>) -> Option<String> {
    value.and_then(|raw| {
        if raw.trim().is_empty() {
            None
        } else {
            Some(raw)
        }
    })
}

fn merge_optional_string(lower: Option<String>, higher: Option<String>) -> Option<String> {
    normalize_non_empty_string(higher).or_else(|| normalize_non_empty_string(lower))
}

fn merge_optional_table(
    lower: Option<toml::Table>,
    higher: Option<toml::Table>,
) -> Option<toml::Table> {
    match (lower, higher) {
        (Some(lower), Some(higher)) => Some(deep_merge_tables(lower, higher)),
        (Some(lower), None) => Some(lower),
        (None, Some(higher)) => Some(higher),
        (None, None) => None,
    }
}

fn deep_merge_tables(mut lower: toml::Table, higher: toml::Table) -> toml::Table {
    for (key, higher_value) in higher {
        match (lower.remove(&key), higher_value) {
            (Some(toml::Value::Table(lower_table)), toml::Value::Table(higher_table)) => {
                lower.insert(
                    key,
                    toml::Value::Table(deep_merge_tables(lower_table, higher_table)),
                );
            }
            (_, higher_value) => {
                lower.insert(key, higher_value);
            }
        }
    }
    lower
}

fn merge_config_section<T>(base: &T, override_table: Option<&toml::Table>) -> anyhow::Result<T>
where
    T: Clone + Serialize + DeserializeOwned,
{
    let Some(override_table) = override_table else {
        return Ok(base.clone());
    };
    let base_value =
        toml::Value::try_from(base).context("serialize base config section to toml")?;
    let merged_value = match base_value {
        toml::Value::Table(base_table) => {
            toml::Value::Table(deep_merge_tables(base_table, override_table.clone()))
        }
        _ => anyhow::bail!("config section must serialize to a TOML table"),
    };
    merged_value
        .try_into()
        .context("deserialize merged config section from toml")
}

/// Load workspace config.toml overrides. Returns defaults on any error.
///
/// Searches for config.toml at the given workspace directory.
async fn load_workspace_overrides(workspace_dir: &std::path::Path) -> WorkspaceOverrides {
    load_overrides_from_path(&workspace_dir.join("config.toml")).await
}

/// Load and cascade config.toml overrides uniformly (3-level).
///
/// global config.toml → user config.toml → agent config.toml (agent overrides user)
///
/// The user-level config is at `{tenant_root}/config.toml` (same dir as owner workspace).
/// The agent-level config is at `{agent_wrapper_dir}/config.toml`.
async fn load_cascaded_overrides(
    _owner_dir: &std::path::Path,
    agent_wrapper_dir: &std::path::Path,
    workspace_dir: &std::path::Path,
) -> WorkspaceOverrides {
    // 3-level: user config → agent config
    // tenant_root is owner_dir's parent (owner_dir = tenant_root/workspace/)
    let tenant_root = _owner_dir.parent().unwrap_or(_owner_dir);
    let user_config = load_overrides_from_path(&tenant_root.join("config.toml")).await;

    // Try agent_wrapper_dir/config.toml first, fallback to workspace_dir/config.toml
    let agent_config_path = match crate::huanxing::config::promote_legacy_agent_config(
        agent_wrapper_dir,
        workspace_dir,
    ) {
        Ok(Some(path)) => path,
        Ok(None) => agent_wrapper_dir.join("config.toml"),
        Err(e) => {
            tracing::warn!(
                wrapper = %agent_wrapper_dir.display(),
                workspace = %workspace_dir.display(),
                error = %e,
                "Failed to promote legacy agent config"
            );
            agent_wrapper_dir.join("config.toml")
        }
    };
    let agent_config = if agent_config_path.exists() {
        load_overrides_from_path(&agent_config_path).await
    } else {
        load_overrides_from_path(&workspace_dir.join("config.toml")).await
    };

    // Merge: agent overrides user
    merge_overrides(user_config, agent_config)
}

/// Load overrides from a specific config.toml path.
async fn load_overrides_from_path(config_path: &std::path::Path) -> WorkspaceOverrides {
    if !config_path.exists() {
        return WorkspaceOverrides::default();
    }
    match tokio::fs::read_to_string(config_path).await {
        Ok(content) => match toml::from_str::<WorkspaceOverrides>(&content) {
            Ok(overrides) => {
                let overrides = overrides.normalize();
                tracing::info!(
                    config_path = %config_path.display(),
                    has_api_key = overrides.api_key.is_some(),
                    has_model = overrides.default_model.is_some(),
                    has_provider = overrides.default_provider.is_some(),
                    has_session = overrides.agent.session.is_some(),
                    has_skills = overrides.skills.is_some(),
                    has_autonomy = overrides.autonomy.is_some(),
                    has_memory = overrides.memory.is_some(),
                    has_reliability = overrides.reliability.is_some(),
                    "Loaded config.toml overrides"
                );
                overrides
            }
            Err(e) => {
                tracing::warn!(
                    path = %config_path.display(),
                    error = %e,
                    "Failed to parse config.toml, using defaults"
                );
                WorkspaceOverrides::default()
            }
        },
        Err(e) => {
            tracing::warn!(
                path = %config_path.display(),
                error = %e,
                "Failed to read config.toml, using defaults"
            );
            WorkspaceOverrides::default()
        }
    }
}

/// Merge two WorkspaceOverrides: `higher` takes priority over `lower`.
/// For Option fields, higher wins if present; otherwise lower is kept.
fn merge_overrides(lower: WorkspaceOverrides, higher: WorkspaceOverrides) -> WorkspaceOverrides {
    WorkspaceOverrides {
        api_key: merge_optional_string(lower.api_key, higher.api_key),
        default_provider: merge_optional_string(lower.default_provider, higher.default_provider),
        default_model: merge_optional_string(lower.default_model, higher.default_model),
        default_temperature: higher.default_temperature.or(lower.default_temperature),
        agent: AgentOverrides {
            session: higher.agent.session.or(lower.agent.session),
            compact_context: higher.agent.compact_context.or(lower.agent.compact_context),
            max_tool_iterations: higher
                .agent
                .max_tool_iterations
                .or(lower.agent.max_tool_iterations),
            max_history_messages: higher
                .agent
                .max_history_messages
                .or(lower.agent.max_history_messages),
            _extra: lower
                .agent
                ._extra
                .into_iter()
                .chain(higher.agent._extra)
                .collect(),
        },
        memory: merge_optional_table(lower.memory, higher.memory),
        knowledge: merge_optional_table(lower.knowledge, higher.knowledge),
        autonomy: merge_optional_table(lower.autonomy, higher.autonomy),
        skills: merge_optional_table(lower.skills, higher.skills),
        security: merge_optional_table(lower.security, higher.security),
        channels_config: merge_optional_table(lower.channels_config, higher.channels_config),
        heartbeat: merge_optional_table(lower.heartbeat, higher.heartbeat),
        cron: merge_optional_table(lower.cron, higher.cron),
        multimodal: merge_optional_table(lower.multimodal, higher.multimodal),
        web_search: merge_optional_table(lower.web_search, higher.web_search),
        web_fetch: merge_optional_table(lower.web_fetch, higher.web_fetch),
        browser: merge_optional_table(lower.browser, higher.browser),
        http_request: merge_optional_table(lower.http_request, higher.http_request),
        reliability: merge_optional_table(lower.reliability, higher.reliability),
        sop: merge_optional_table(lower.sop, higher.sop),
        _extra: lower._extra.into_iter().chain(higher._extra).collect(),
    }
}

fn default_guardian_prompt() -> String {
    r#"你是唤星云服务的迎宾助手。

当用户发送消息时，请引导他们完成注册流程：
1. 询问用户手机号
2. 发送短信验证码
3. 验证后完成注册

注册完成后，用户将获得专属的 AI 助手。"#
        .to_string()
}

/// Create a session backend for a given workspace directory.
///
/// 供 gateway/ws.rs 等外部模块调用，按 `agent_name` 解析 per-user workspace 后
/// 创建隔离的会话持久化后端，实现多租户数据隔离。
pub fn create_session_backend_for_workspace(
    workspace_dir: &std::path::Path,
    global_config: &crate::config::Config,
) -> Option<Arc<dyn SessionBackend>> {
    create_session_backend(workspace_dir, &global_config.channels_config)
}

/// Create a session backend based on `channels_config.session_backend`.
/// Returns `None` if session persistence is disabled or creation fails.
fn create_session_backend(
    workspace_dir: &std::path::Path,
    channels_config: &crate::config::ChannelsConfig,
) -> Option<Arc<dyn SessionBackend>> {
    if !channels_config.session_persistence {
        return None;
    }
    match channels_config.session_backend.as_str() {
        "sqlite" => {
            match crate::channels::session_sqlite::SqliteSessionBackend::new(workspace_dir) {
                Ok(b) => {
                    // Auto-migrate existing JSONL files
                    if let Ok(n) = b.migrate_from_jsonl(workspace_dir) {
                        if n > 0 {
                            tracing::info!(
                                workspace = %workspace_dir.display(),
                                migrated = n,
                                "Migrated JSONL sessions to SQLite"
                            );
                        }
                    }
                    Some(Arc::new(b))
                }
                Err(e) => {
                    tracing::warn!(
                        workspace = %workspace_dir.display(),
                        error = %e,
                        "Failed to create SQLite session backend, falling back to JSONL"
                    );
                    create_jsonl_fallback(workspace_dir)
                }
            }
        }
        _ => create_jsonl_fallback(workspace_dir),
    }
}

fn create_jsonl_fallback(workspace_dir: &std::path::Path) -> Option<Arc<dyn SessionBackend>> {
    match crate::channels::session_store::SessionStore::new(workspace_dir) {
        Ok(store) => Some(Arc::new(store)),
        Err(e) => {
            tracing::warn!(
                workspace = %workspace_dir.display(),
                error = %e,
                "Failed to create JSONL session backend"
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TenantContext;
    use crate::config::Config;
    use crate::huanxing::db::TenantDb;
    use tempfile::tempdir;

    async fn seed_tenant(
        config_dir: &std::path::Path,
        tenant_dir: &str,
        agent_id: &str,
        hasn_id: &str,
    ) {
        let db_path = config_dir.join("data").join("users.db");
        let db = TenantDb::open(&db_path).unwrap();
        db.save_user_full(
            "user-1",
            "13800000000",
            agent_id,
            Some("Tester"),
            "assistant",
            Some("Star"),
            None,
            Some(tenant_dir),
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
        assert!(db.update_agent_hasn_id(agent_id, hasn_id).await.unwrap());
    }

    fn test_config(config_dir: &std::path::Path) -> Config {
        let mut config = Config::default();
        config.huanxing.enabled = true;
        config.config_path = config_dir.join("config.toml");
        config.workspace_dir = config_dir.join("workspace");
        config.knowledge.enabled = false;
        config
    }

    async fn create_workspace_tree(
        config_dir: &std::path::Path,
        tenant_dir: &str,
        agent_id: &str,
    ) -> (std::path::PathBuf, std::path::PathBuf) {
        let owner_dir = config_dir.join("users").join(tenant_dir).join("workspace");
        let agent_workspace = config_dir
            .join("users")
            .join(tenant_dir)
            .join("agents")
            .join(agent_id)
            .join("workspace");
        tokio::fs::create_dir_all(&owner_dir).await.unwrap();
        tokio::fs::create_dir_all(&agent_workspace).await.unwrap();
        tokio::fs::write(owner_dir.join("USER.md"), "# User\n")
            .await
            .unwrap();
        tokio::fs::write(agent_workspace.join("SOUL.md"), "# Soul\n")
            .await
            .unwrap();
        (owner_dir, agent_workspace)
    }

    #[tokio::test]
    async fn load_by_agent_or_hasn_resolves_same_tenant_context() {
        let temp = tempdir().unwrap();
        let config_dir = temp.path();
        let tenant_dir = "001-13800000000";
        let agent_id = "default";
        let hasn_id = "hasn-agent-1";
        let (expected_owner_dir, expected_workspace_dir) =
            create_workspace_tree(config_dir, tenant_dir, agent_id).await;
        seed_tenant(config_dir, tenant_dir, agent_id, hasn_id).await;

        let config = test_config(config_dir);

        let by_agent = TenantContext::load_by_agent_or_hasn(&config, agent_id)
            .await
            .unwrap()
            .expect("agent lookup should resolve");
        let by_hasn = TenantContext::load_by_agent_or_hasn(&config, hasn_id)
            .await
            .unwrap()
            .expect("hasn lookup should resolve");

        assert_eq!(by_agent.agent_id, agent_id);
        assert_eq!(by_hasn.agent_id, agent_id);
        assert_eq!(by_agent.user_id, "user-1");
        assert_eq!(by_hasn.user_id, "user-1");
        assert_eq!(by_agent.owner_dir, expected_owner_dir);
        assert_eq!(by_hasn.owner_dir, expected_owner_dir);
        assert_eq!(by_agent.workspace_dir, expected_workspace_dir);
        assert_eq!(by_hasn.workspace_dir, expected_workspace_dir);
    }

    #[tokio::test]
    async fn create_agent_uses_tenant_prompt_snapshot_instead_of_rereading_workspace() {
        let temp = tempdir().unwrap();
        let config_dir = temp.path();
        let tenant_dir = "001-13800000000";
        let agent_id = "default";
        let (_owner_dir, agent_workspace_dir) =
            create_workspace_tree(config_dir, tenant_dir, agent_id).await;
        seed_tenant(config_dir, tenant_dir, agent_id, "hasn-agent-1").await;

        let config = test_config(config_dir);
        let mut tenant = TenantContext::load_by_agent_id(&config, agent_id)
            .await
            .unwrap()
            .expect("agent lookup should resolve");

        tokio::fs::write(agent_workspace_dir.join("SOUL.md"), "# Changed on disk\n")
            .await
            .unwrap();

        tenant.system_prompt = "tenant-context prompt snapshot".to_string();
        let mut agent = tenant.create_agent().await.unwrap();
        agent.seed_history(&[]);

        let history = agent.history();
        assert!(
            matches!(
                &history[0],
                crate::providers::ConversationMessage::Chat(msg)
                    if msg.role == "system"
                        && msg.content == "tenant-context prompt snapshot"
            ),
            "agent should use the prompt already resolved by TenantContext"
        );
    }

    #[tokio::test]
    async fn tenant_context_deep_merges_partial_user_and_agent_sections() {
        let temp = tempdir().unwrap();
        let config_dir = temp.path();
        let tenant_dir = "001-13800000000";
        let agent_id = "default";
        let (_owner_dir, agent_workspace_dir) =
            create_workspace_tree(config_dir, tenant_dir, agent_id).await;
        seed_tenant(config_dir, tenant_dir, agent_id, "hasn-agent-1").await;

        let user_config_path = config_dir
            .join("users")
            .join(tenant_dir)
            .join("config.toml");
        tokio::fs::write(
            &user_config_path,
            r#"
[memory]
embedding_model = "user-embedding"

[skills]
open_skills_enabled = true

[reliability]
provider_backoff_ms = 321
"#,
        )
        .await
        .unwrap();

        let agent_config_path = agent_workspace_dir.parent().unwrap().join("config.toml");
        tokio::fs::write(
            &agent_config_path,
            r#"
[skills]
prompt_injection_mode = "compact"

[reliability]
fallback_providers = ["agent-fallback"]

[channels_config]
message_timeout_secs = 42
"#,
        )
        .await
        .unwrap();

        let mut config = test_config(config_dir);
        config.memory.backend = "sqlite".to_string();
        config.memory.auto_save = true;
        config.memory.embedding_model = "global-embedding".to_string();
        config.skills.allow_scripts = true;
        config.skills.prompt_injection_mode = crate::config::SkillsPromptInjectionMode::Full;
        config.reliability.provider_retries = 7;
        config.reliability.provider_backoff_ms = 1000;

        let tenant = TenantContext::load_by_agent_id(&config, agent_id)
            .await
            .unwrap()
            .expect("agent lookup should resolve");

        assert_eq!(tenant.runtime_config().memory.backend, "sqlite");
        assert!(tenant.runtime_config().memory.auto_save);
        assert_eq!(
            tenant.runtime_config().memory.embedding_model,
            "user-embedding"
        );
        assert!(tenant.runtime_config().skills.open_skills_enabled);
        assert!(tenant.runtime_config().skills.allow_scripts);
        assert_eq!(
            tenant.runtime_config().skills.prompt_injection_mode,
            crate::config::SkillsPromptInjectionMode::Compact
        );
        assert_eq!(tenant.runtime_config().reliability.provider_retries, 7);
        assert_eq!(tenant.runtime_config().reliability.provider_backoff_ms, 321);
        assert_eq!(
            tenant.runtime_config().reliability.fallback_providers,
            vec!["agent-fallback".to_string()]
        );
        assert_eq!(tenant.message_timeout_secs, 42);
        assert_eq!(
            tenant.runtime_config().channels_config.message_timeout_secs,
            42
        );
    }

    #[tokio::test]
    async fn tenant_context_ignores_empty_string_root_overrides() {
        let temp = tempdir().unwrap();
        let config_dir = temp.path();
        let tenant_dir = "001-13800000000";
        let agent_id = "default";
        let (_owner_dir, agent_workspace_dir) =
            create_workspace_tree(config_dir, tenant_dir, agent_id).await;
        seed_tenant(config_dir, tenant_dir, agent_id, "hasn-agent-1").await;

        let agent_config_path = agent_workspace_dir.parent().unwrap().join("config.toml");
        tokio::fs::write(
            &agent_config_path,
            r#"
api_key = ""
default_provider = ""
default_model = ""
"#,
        )
        .await
        .unwrap();

        let mut config = test_config(config_dir);
        config.api_key = Some("global-api-key".to_string());
        config.default_provider = Some("global-provider".to_string());
        config.default_model = Some("global-model".to_string());
        config.huanxing.default_provider = Some("tenant-provider".to_string());
        config.huanxing.default_model = Some("tenant-model".to_string());

        let tenant = TenantContext::load_by_agent_id(&config, agent_id)
            .await
            .unwrap()
            .expect("agent lookup should resolve");

        assert_eq!(tenant.api_key.as_deref(), Some("global-api-key"));
        assert_eq!(tenant.provider.as_deref(), Some("tenant-provider"));
        assert_eq!(tenant.model.as_deref(), Some("tenant-model"));
        assert_eq!(
            tenant.runtime_config().default_provider.as_deref(),
            Some("tenant-provider")
        );
        assert_eq!(
            tenant.runtime_config().default_model.as_deref(),
            Some("tenant-model")
        );
    }

    #[tokio::test]
    async fn tenant_context_prefers_agent_api_key_when_owner_api_key_is_empty() {
        let temp = tempdir().unwrap();
        let config_dir = temp.path();
        let tenant_dir = "001-13800000000";
        let agent_id = "default";
        let (_owner_dir, agent_workspace_dir) =
            create_workspace_tree(config_dir, tenant_dir, agent_id).await;
        seed_tenant(config_dir, tenant_dir, agent_id, "hasn-agent-1").await;

        let user_config_path = config_dir
            .join("users")
            .join(tenant_dir)
            .join("config.toml");
        tokio::fs::write(&user_config_path, "api_key = \"\"\n")
            .await
            .unwrap();

        let agent_config_path = agent_workspace_dir.parent().unwrap().join("config.toml");
        tokio::fs::write(
            &agent_config_path,
            "api_key = \"agent-level-token\"\ndefault_provider = \"agent-provider\"\n",
        )
        .await
        .unwrap();

        let mut config = test_config(config_dir);
        config.api_key = Some("global-api-key".to_string());

        let tenant = TenantContext::load_by_agent_id(&config, agent_id)
            .await
            .unwrap()
            .expect("agent lookup should resolve");

        assert_eq!(tenant.api_key.as_deref(), Some("agent-level-token"));
        assert_eq!(
            tenant.runtime_config().api_key.as_deref(),
            Some("agent-level-token")
        );
        assert_eq!(tenant.provider.as_deref(), Some("agent-provider"));
    }

    #[tokio::test]
    async fn load_by_agent_id_promotes_legacy_workspace_config_to_wrapper() {
        let temp = tempdir().unwrap();
        let config_dir = temp.path();
        let tenant_dir = "001-13800000000";
        let agent_id = "default";
        let (_owner_dir, agent_workspace_dir) =
            create_workspace_tree(config_dir, tenant_dir, agent_id).await;
        seed_tenant(config_dir, tenant_dir, agent_id, "hasn-agent-1").await;

        let legacy_config_path = agent_workspace_dir.join("config.toml");
        tokio::fs::write(
            &legacy_config_path,
            "default_provider = \"legacy-provider\"\ndefault_model = \"legacy-model\"\n",
        )
        .await
        .unwrap();

        let config = test_config(config_dir);
        let tenant = TenantContext::load_by_agent_id(&config, agent_id)
            .await
            .unwrap()
            .expect("agent lookup should resolve");
        let wrapper_config_path = agent_workspace_dir.parent().unwrap().join("config.toml");

        assert_eq!(tenant.provider.as_deref(), Some("legacy-provider"));
        assert_eq!(tenant.model.as_deref(), Some("legacy-model"));
        assert!(wrapper_config_path.exists());
        assert!(!legacy_config_path.exists());
    }

    #[tokio::test]
    async fn tenant_context_skips_cross_index_when_knowledge_is_shared_via_owner_dir() {
        let temp = tempdir().unwrap();
        let config_dir = temp.path();
        let tenant_dir = "001-13800000000";
        let agent_id = "default";
        let (_owner_dir, _agent_workspace_dir) =
            create_workspace_tree(config_dir, tenant_dir, agent_id).await;
        seed_tenant(config_dir, tenant_dir, agent_id, "hasn-agent-1").await;

        let mut config = test_config(config_dir);
        config.knowledge.enabled = true;
        config.knowledge.cross_workspace_search = true;

        let tenant = TenantContext::load_by_agent_id(&config, agent_id)
            .await
            .unwrap()
            .expect("agent lookup should resolve");

        assert!(tenant.knowledge_graph.is_some());
        assert!(tenant.cross_knowledge_index.is_none());
    }
}
