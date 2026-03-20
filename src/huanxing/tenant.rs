//! Per-tenant agent context.
//!
//! A [`TenantContext`] carries the per-user overrides that customize the shared
//! agent loop: system prompt, workspace directory, model, provider, tool
//! filter, memory, session manager, and conversation histories.
//!
//! The shared [`ChannelRuntimeContext`] provides channels, LLM pool,
//! and base tools — tenant context overrides the user-facing subset.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::Deserialize;

use crate::channels::session_backend::SessionBackend;
use crate::memory::{self, Memory};
use crate::providers::ChatMessage;

// ── Workspace config.toml partial overlay ────────────────────
//
// Agent workspaces may contain a `config.toml` written at registration time.
// We only parse the fields that are meaningful for per-tenant override;
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
    #[serde(default)]
    memory: Option<MemoryOverrides>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct AgentOverrides {
    session: Option<serde_json::Value>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct MemoryOverrides {
    auto_save: Option<bool>,
    backend: Option<String>,
}

/// Type alias matching channels/mod.rs ConversationHistoryMap.
pub type ConversationHistoryMap = Arc<Mutex<HashMap<String, Vec<ChatMessage>>>>;

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
}

// Manual Debug impl because Arc<dyn Memory> doesn't impl Debug.
impl std::fmt::Debug for TenantContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TenantContext")
            .field("agent_id", &self.agent_id)
            .field("user_id", &self.user_id)
            .field("workspace_dir", &self.workspace_dir)
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
            .finish()
    }
}

impl TenantContext {
    /// Load a tenant context from workspace directory.
    ///
    /// Builds the full system prompt from workspace files (SOUL.md, AGENTS.md,
    /// USER.md, BOOTSTRAP.md, MEMORY.md, skills/), creates per-tenant memory
    /// and session manager instances.
    pub async fn load(
        agent_id: &str,
        user_id: &str,
        workspace_dir: PathBuf,
        model: Option<String>,
        provider: Option<String>,
        template: Option<String>,
        nickname: Option<String>,
        star_name: Option<String>,
        plan: Option<String>,
        global_config: &crate::config::Config,
    ) -> anyhow::Result<Self> {
        // ── 0. Load workspace config.toml overrides ──────────────────
        let overrides = load_workspace_overrides(&workspace_dir).await;

        // Effective model/provider: workspace config > DB record > global [huanxing] default
        let effective_model = overrides.default_model.clone().or(model.clone());
        let effective_provider = overrides.default_provider.clone().or(provider.clone());
        let effective_api_key = overrides
            .api_key
            .clone()
            .or_else(|| global_config.api_key.clone());
        let effective_temperature = overrides.default_temperature;

        // ── A. Build full system prompt from workspace files ──────────
        let model_name = effective_model
            .as_deref()
            .or(global_config.default_model.as_deref())
            .unwrap_or("claude-sonnet-4-6");

        // Load skills from tenant workspace + common skills directory
        let common_skills_dir = global_config
            .huanxing
            .resolve_common_skills_dir(&global_config.workspace_dir);

        let mut skills = crate::skills::load_skills_with_config(&workspace_dir, global_config);
        let ws_skill_names: Vec<String> = skills.iter().map(|s| s.name.clone()).collect();
        tracing::info!(
            agent_id,
            workspace = %workspace_dir.display(),
            count = skills.len(),
            names = ?ws_skill_names,
            "【技能调试】agent 私有技能目录加载结果"
        );

        tracing::info!(
            agent_id,
            common_skills_dir = %common_skills_dir.display(),
            exists = common_skills_dir.exists(),
            "【技能调试】公共技能目录状态"
        );

        if common_skills_dir.exists() {
            let ws_names: std::collections::HashSet<String> =
                skills.iter().map(|s| s.name.clone()).collect();
            let common_skills = crate::skills::load_skills_with_config(&common_skills_dir, global_config);
            let common_names: Vec<String> = common_skills.iter().map(|s| s.name.clone()).collect();
            tracing::info!(
                agent_id,
                count = common_skills.len(),
                names = ?common_names,
                "【技能调试】公共技能目录加载结果"
            );
            for skill in common_skills {
                if !ws_names.contains(&skill.name) {
                    skills.push(skill);
                }
            }
        }

        let all_skill_names: Vec<String> = skills.iter().map(|s| s.name.clone()).collect();
        tracing::info!(
            agent_id,
            total = skills.len(),
            names = ?all_skill_names,
            "【技能调试】合并后技能总数"
        );

        let tool_descs: Vec<(&str, &str)> = Vec::new();

        let system_prompt = crate::channels::build_system_prompt(
            &workspace_dir,
            model_name,
            &tool_descs,
            &skills,
            Some(&global_config.identity),
            None,
        );

        let has_skills_section = system_prompt.contains("<available_skills>");
        tracing::info!(
            agent_id,
            prompt_len = system_prompt.len(),
            skills_count = skills.len(),
            has_skills_section,
            "【技能调试】系统提示词构建完成"
        );

        // ── B. Create per-tenant memory ──────────────────────────────
        let effective_memory_config = {
            let mut cfg = global_config.memory.clone();
            if let Some(ref mem_ov) = overrides.memory {
                if let Some(auto_save) = mem_ov.auto_save {
                    cfg.auto_save = auto_save;
                }
                if let Some(ref backend) = mem_ov.backend {
                    cfg.backend = backend.clone();
                }
            }
            cfg
        };
        let tenant_memory: Arc<dyn Memory> = Arc::from(memory::create_memory(
            &effective_memory_config,
            &workspace_dir,
            effective_api_key.as_deref(),
        )?);

        // ── C. Session backend (JSONL or SQLite based on channels_config) ──
        let tenant_session_manager: Option<Arc<dyn SessionBackend>> =
            create_session_backend(&workspace_dir, global_config);

        // ── D. Independent conversation histories ────────────────────
        let conversation_histories: ConversationHistoryMap = Arc::new(Mutex::new(HashMap::new()));

        Ok(Self {
            agent_id: agent_id.to_string(),
            user_id: user_id.to_string(),
            workspace_dir,
            system_prompt,
            model: effective_model,
            provider: effective_provider,
            template,
            nickname,
            star_name,
            plan,
            temperature: effective_temperature,
            api_key: effective_api_key,
            is_guardian: false,
            memory: tenant_memory,
            session_manager: tenant_session_manager,
            conversation_histories,
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

        let model_name = overrides
            .default_model
            .as_deref()
            .or(global_config.default_model.as_deref())
            .unwrap_or("claude-sonnet-4-6");

        // Load skills from guardian workspace + common skills directory
        let common_skills_dir = global_config
            .huanxing
            .resolve_common_skills_dir(&global_config.workspace_dir);
        let mut skills = crate::skills::load_skills_with_config(&workspace_dir, global_config);
        if common_skills_dir.exists() {
            let ws_names: std::collections::HashSet<String> =
                skills.iter().map(|s| s.name.clone()).collect();
            for skill in crate::skills::load_skills_with_config(&common_skills_dir, global_config) {
                if !ws_names.contains(&skill.name) {
                    skills.push(skill);
                }
            }
        }

        let tool_descs: Vec<(&str, &str)> = Vec::new();

        // Build full system prompt from guardian workspace files
        let system_prompt = if workspace_dir.join("SOUL.md").exists() {
            crate::channels::build_system_prompt(
                &workspace_dir,
                model_name,
                &tool_descs,
                &skills,
                Some(&global_config.identity),
                None,
            )
        } else {
            default_guardian_prompt()
        };

        // Per-tenant memory for guardian
        let effective_memory_config = {
            let mut cfg = global_config.memory.clone();
            if let Some(ref mem_ov) = overrides.memory {
                if let Some(auto_save) = mem_ov.auto_save {
                    cfg.auto_save = auto_save;
                }
                if let Some(ref backend) = mem_ov.backend {
                    cfg.backend = backend.clone();
                }
            }
            cfg
        };
        let guardian_memory: Arc<dyn Memory> = Arc::from(memory::create_memory(
            &effective_memory_config,
            &workspace_dir,
            effective_api_key.as_deref(),
        )?);

        // Per-tenant session backend for guardian
        let guardian_session_manager: Option<Arc<dyn SessionBackend>> =
            create_session_backend(&workspace_dir, global_config);

        let conversation_histories: ConversationHistoryMap = Arc::new(Mutex::new(HashMap::new()));

        Ok(Self {
            agent_id: "guardian".to_string(),
            user_id: String::new(),
            workspace_dir,
            system_prompt,
            model: overrides.default_model,
            provider: overrides.default_provider,
            template: None,
            nickname: None,
            star_name: None,
            plan: None,
            temperature: overrides.default_temperature,
            api_key: effective_api_key,
            is_guardian: true,
            memory: guardian_memory,
            session_manager: guardian_session_manager,
            conversation_histories,
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

/// Load workspace config.toml overrides. Returns defaults on any error.
async fn load_workspace_overrides(workspace_dir: &std::path::Path) -> WorkspaceOverrides {
    let config_path = workspace_dir.join("config.toml");
    if !config_path.exists() {
        return WorkspaceOverrides::default();
    }
    match tokio::fs::read_to_string(&config_path).await {
        Ok(content) => match toml::from_str::<WorkspaceOverrides>(&content) {
            Ok(overrides) => {
                tracing::debug!(
                    workspace = %workspace_dir.display(),
                    has_api_key = overrides.api_key.is_some(),
                    has_model = overrides.default_model.is_some(),
                    has_provider = overrides.default_provider.is_some(),
                    has_session = overrides.agent.session.is_some(),
                    "Loaded workspace config.toml overrides"
                );
                overrides
            }
            Err(e) => {
                tracing::warn!(
                    path = %config_path.display(),
                    error = %e,
                    "Failed to parse workspace config.toml, using defaults"
                );
                WorkspaceOverrides::default()
            }
        },
        Err(e) => {
            tracing::warn!(
                path = %config_path.display(),
                error = %e,
                "Failed to read workspace config.toml, using defaults"
            );
            WorkspaceOverrides::default()
        }
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
    create_session_backend(workspace_dir, global_config)
}

/// Create a session backend based on `channels_config.session_backend`.
/// Returns `None` if session persistence is disabled or creation fails.
fn create_session_backend(
    workspace_dir: &std::path::Path,
    global_config: &crate::config::Config,
) -> Option<Arc<dyn SessionBackend>> {
    if !global_config.channels_config.session_persistence {
        return None;
    }
    match global_config.channels_config.session_backend.as_str() {
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
