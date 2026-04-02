//! HuanXing multi-tenant configuration.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Top-level `[huanxing]` configuration section in config.toml.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct HuanXingConfig {
    /// Enable multi-tenant routing. When false, behaves as standard single-agent.
    pub enabled: bool,

    /// Path to the SQLite user database.
    /// Default: `{config_dir}/data/users.db`
    pub db_path: Option<PathBuf>,

    /// Root directory for per-tenant agent workspaces.
    /// **Deprecated**: use `resolve_agent_wrapper_dir()` instead. Kept for backward compat.
    /// Default: `{config_dir}/agents`
    pub agents_dir: Option<PathBuf>,

    /// Workspace directory for the Guardian agent (handles unregistered users).
    /// Default: `{config_dir}/guardian`
    pub guardian_workspace: Option<PathBuf>,

    /// Workspace directory for the Admin agent (server management).
    /// Default: `{config_dir}/admin`
    pub admin_workspace: Option<PathBuf>,

    /// Channel types routed to the Admin agent (e.g. `["feishu"]`).
    /// All messages from these channels go directly to Admin, bypassing
    /// normal tenant routing.
    #[serde(default)]
    pub admin_channels: Vec<String>,

    /// Agent templates keyed by template name (e.g. "finance", "assistant").
    #[serde(default)]
    pub templates: HashMap<String, TemplateConfig>,

    /// Default template name for new users.
    pub default_template: Option<String>,

    /// Default model for tenant agents. Overridden by per-user config.
    pub default_model: Option<String>,

    /// Default provider for tenant agents.
    pub default_provider: Option<String>,

    /// Tool names only available to Guardian agent (not to tenant agents).
    #[serde(default)]
    pub guardian_only_tools: Vec<String>,

    // ── Phase 1: Backend API integration ───────────────
    /// HuanXing backend API base URL.
    /// Default: `https://api.huanxing.dcfuture.cn`
    pub api_base_url: Option<String>,

    /// Agent authentication key (X-Agent-Key header for backend API).
    pub agent_key: Option<String>,

    /// Server identifier for heartbeat registration.
    pub server_id: Option<String>,

    /// Server IP address (reported to backend in heartbeat/registration).
    pub server_ip: Option<String>,

    /// HASN social network API base URL (defaults to api_base_url).
    pub hasn_base_url: Option<String>,

    /// LLM API base URL for tenant agents (e.g. OpenRouter/custom endpoint).
    pub llm_base_url: Option<String>,

    /// Heartbeat interval in seconds. Default: 300 (5 minutes).
    pub heartbeat_interval_secs: Option<u64>,

    /// Root directory for agent templates.
    /// **Deprecated**: templates are now fetched from the marketplace.
    pub templates_dir: Option<PathBuf>,

    /// Directory for user data backups.
    /// Default: `{config_dir}/backups`
    pub backup_dir: Option<PathBuf>,

    /// Directory for common skills shared across all user agents.
    /// Default: `{config_dir}/skills`
    pub common_skills_dir: Option<PathBuf>,

    /// Path to the huanxing-hub repository (skill marketplace).
    /// **Deprecated**: cloud/desktop unified to fetch from marketplace API.
    pub hub_dir: Option<PathBuf>,

    /// Hub Gitee 同步配置。
    #[serde(default)]
    pub hub_sync: HubSyncConfig,

    /// Multi-tenant heartbeat configuration.
    #[serde(default)]
    pub tenant_heartbeat: TenantHeartbeatConfig,

    /// HuanXing custom image generation tool (`[huanxing.hx_image_gen]`).
    #[serde(default)]
    pub hx_image_gen: HxImageGenConfig,

    /// HASN node connection configuration (`[huanxing.hasn]`).
    #[serde(default)]
    pub hasn: HasnNodeConfig,
}

/// Standalone image generation tool configuration for HuanXing gateway (`[huanxing.hx_image_gen]`).
///
/// When enabled, registers an `hx_image_gen` tool that generates images via
/// a custom gateway (e.g. new-api) using OpenAI-compatible payload and saves them
/// to the workspace `images/` directory.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HxImageGenConfig {
    /// Enable the HuanXing image generation tool. Default: false.
    #[serde(default)]
    pub enabled: bool,

    /// Array of models to try. Fallbacks to the next model if one fails.
    #[serde(default = "default_hx_image_gen_models")]
    pub models: Vec<String>,

    /// Override API Base URL for image generation. Optional.
    #[serde(default)]
    pub api_url: Option<String>,

    /// Override API Key for image generation. Optional.
    #[serde(default)]
    pub api_key: Option<String>,
}

fn default_hx_image_gen_models() -> Vec<String> {
    vec!["dall-e-3".into()]
}

impl Default for HxImageGenConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            models: default_hx_image_gen_models(),
            api_url: None,
            api_key: None,
        }
    }
}

/// Configuration for multi-tenant heartbeat scheduling.
///
/// When enabled, the daemon scans all active tenants' `HEARTBEAT.md` files
/// and executes tasks whose cron schedule matches the current time.
/// Results are delivered through the tenant's bound channel (QQ/Feishu/etc).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct TenantHeartbeatConfig {
    /// Enable multi-tenant heartbeat scanning. Default: `false`.
    pub enabled: bool,

    /// Scan interval in minutes. How often to check all tenants' HEARTBEAT.md.
    /// The actual task trigger timing is controlled by each task's `schedule:cron`
    /// expression — this just sets how often we check.
    /// Default: `30`.
    pub scan_interval_minutes: u32,

    /// Maximum concurrent tenant heartbeat executions.
    /// Prevents LLM request storms when many tenants trigger at the same time.
    /// Default: `3`.
    pub max_concurrent: usize,

    /// Per-tenant agent execution timeout in seconds. Default: `120`.
    pub per_tenant_timeout_secs: u64,
}

impl Default for TenantHeartbeatConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            scan_interval_minutes: 30,
            max_concurrent: 3,
            per_tenant_timeout_secs: 120,
        }
    }
}

impl Default for HuanXingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            db_path: None,
            agents_dir: None,
            guardian_workspace: None,
            admin_workspace: None,
            admin_channels: Vec::new(),
            templates: HashMap::new(),
            default_template: None,
            default_model: None,
            default_provider: None,
            guardian_only_tools: vec![
                "hx_register_user".to_string(),
                "hx_invalidate_cache".to_string(),
            ],
            api_base_url: None,
            agent_key: None,
            server_id: None,
            server_ip: None,
            hasn_base_url: None,
            llm_base_url: None,
            heartbeat_interval_secs: None,
            templates_dir: None,
            backup_dir: None,
            common_skills_dir: None,
            hub_dir: None,
            hub_sync: HubSyncConfig::default(),
            tenant_heartbeat: TenantHeartbeatConfig::default(),
            hx_image_gen: HxImageGenConfig::default(),
            hasn: HasnNodeConfig::default(),
        }
    }
}

impl HuanXingConfig {
    // ── New canonical path resolution (dual-track architecture) ──────────

    /// Resolve the tenant root directory.
    ///
    /// Always returns `{config_dir}/users/{tenant_dir}/` where `tenant_dir`
    /// is in `{seq}-{phone}` format (e.g. `001-13888888888`), unless no tenant_dir
    /// is provided (system agents).
    pub fn resolve_tenant_root(
        &self,
        config_dir: &std::path::Path,
        tenant_dir: Option<&str>,
    ) -> PathBuf {
        if let Some(td) = tenant_dir {
            config_dir.join("users").join(td)
        } else {
            // System-level agents have no tenant_dir
            config_dir.to_path_buf()
        }
    }

    /// Resolve the owner workspace directory (global shared memory domain).
    ///
    /// Returns `{tenant_root}/workspace/`.
    pub fn resolve_owner_dir(
        &self,
        config_dir: &std::path::Path,
        tenant_dir: Option<&str>,
    ) -> PathBuf {
        self.resolve_tenant_root(config_dir, tenant_dir)
            .join("workspace")
    }

    /// Resolve the agent wrapper directory (outer container for a specific agent).
    ///
    /// - System agents (`admin`, `guardian`): `{config_dir}/admin/` or `{config_dir}/guardian/`
    /// - Regular agents: `{tenant_root}/agents/{agent_id}/`
    pub fn resolve_agent_wrapper_dir(
        &self,
        config_dir: &std::path::Path,
        tenant_dir: Option<&str>,
        agent_id: &str,
    ) -> PathBuf {
        // System-level agents live at config_dir root, outside any user directory
        if agent_id == "admin" || agent_id == "guardian" {
            return config_dir.join(agent_id);
        }
        self.resolve_tenant_root(config_dir, tenant_dir)
            .join("agents")
            .join(agent_id)
    }

    /// Resolve the agent workspace directory (inner execution domain).
    ///
    /// Returns `{agent_wrapper}/workspace/`.
    pub fn resolve_agent_workspace(
        &self,
        config_dir: &std::path::Path,
        tenant_dir: Option<&str>,
        agent_id: &str,
    ) -> PathBuf {
        self.resolve_agent_wrapper_dir(config_dir, tenant_dir, agent_id)
            .join("workspace")
    }

    /// Resolve the canonical agent config path at the wrapper root.
    ///
    /// Returns `{agent_wrapper}/config.toml`.
    pub fn resolve_agent_config_path(
        &self,
        config_dir: &std::path::Path,
        tenant_dir: Option<&str>,
        agent_id: &str,
    ) -> PathBuf {
        self.resolve_agent_wrapper_dir(config_dir, tenant_dir, agent_id)
            .join("config.toml")
    }

    /// Resolve the memory database path within the owner workspace.
    ///
    /// Returns `{owner_workspace}/memory/brain.db`.
    pub fn resolve_brain_db(
        &self,
        config_dir: &std::path::Path,
        tenant_dir: Option<&str>,
    ) -> PathBuf {
        self.resolve_owner_dir(config_dir, tenant_dir)
            .join("memory")
            .join("brain.db")
    }

    /// Resolve the session database path within the agent workspace.
    ///
    /// Returns `{agent_workspace}/sessions/sessions.db`.
    pub fn resolve_sessions_db(
        &self,
        config_dir: &std::path::Path,
        tenant_dir: Option<&str>,
        agent_id: &str,
    ) -> PathBuf {
        self.resolve_agent_workspace(config_dir, tenant_dir, agent_id)
            .join("sessions")
            .join("sessions.db")
    }

    // ── Legacy path resolution (kept for backward compatibility) ─────────

    /// Resolve the database path, using config_dir as base if not absolute.
    pub fn resolve_db_path(&self, config_dir: &std::path::Path) -> PathBuf {
        self.db_path
            .clone()
            .unwrap_or_else(|| config_dir.join("data").join("users.db"))
    }

    /// Resolve the agents directory.
    ///
    /// **Deprecated**: prefer `resolve_agent_wrapper_dir()` for new code.
    pub fn resolve_agents_dir(&self, config_dir: &std::path::Path) -> PathBuf {
        self.agents_dir
            .clone()
            .unwrap_or_else(|| config_dir.join("agents"))
    }

    /// Resolve the guardian workspace.
    pub fn resolve_guardian_workspace(&self, config_dir: &std::path::Path) -> PathBuf {
        self.guardian_workspace
            .clone()
            .unwrap_or_else(|| config_dir.join("guardian"))
    }

    /// Resolve the admin workspace.
    pub fn resolve_admin_workspace(&self, config_dir: &std::path::Path) -> PathBuf {
        self.admin_workspace
            .clone()
            .unwrap_or_else(|| config_dir.join("admin"))
    }

    /// Check if a channel type is routed to the Admin agent.
    pub fn is_admin_channel(&self, channel_type: &str) -> bool {
        self.admin_channels.iter().any(|c| c == channel_type)
    }

    /// Resolve the templates directory.
    /// **Deprecated**: templates are now fetched from the marketplace.
    pub fn resolve_templates_dir(&self, workspace_dir: &std::path::Path) -> PathBuf {
        self.templates_dir
            .clone()
            .unwrap_or_else(|| workspace_dir.join("templates"))
    }

    /// Resolve the backup directory.
    pub fn resolve_backup_dir(&self, config_dir: &std::path::Path) -> PathBuf {
        self.backup_dir
            .clone()
            .unwrap_or_else(|| config_dir.join("backups"))
    }

    /// Resolve the common skills directory.
    /// Default: `{config_dir}/skills`
    pub fn resolve_common_skills_dir(&self, config_dir: &std::path::Path) -> PathBuf {
        self.common_skills_dir
            .clone()
            .unwrap_or_else(|| config_dir.join("skills"))
    }

    /// Resolve the hub directory (skill marketplace repository).
    /// **Deprecated**: returns None if not configured.
    pub fn resolve_hub_dir(&self) -> Option<PathBuf> {
        self.hub_dir.clone()
    }

    // ── Utility methods ─────────────────────────────────────────────────

    /// Get the backend API base URL.
    pub fn api_url(&self) -> &str {
        self.api_base_url
            .as_deref()
            .unwrap_or("https://api.huanxing.dcfuture.cn")
    }

    /// Get the HASN API base URL (falls back to api_base_url).
    pub fn hasn_url(&self) -> &str {
        self.hasn_base_url
            .as_deref()
            .unwrap_or_else(|| self.api_url())
    }

    /// Get the heartbeat interval.
    pub fn heartbeat_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.heartbeat_interval_secs.unwrap_or(300))
    }

    /// Get the server ID (falls back to hostname).
    pub fn server_id_or_hostname(&self) -> String {
        self.server_id.clone().unwrap_or_else(|| {
            hostname::get()
                .ok()
                .and_then(|h| h.into_string().ok())
                .unwrap_or_else(|| "zeroclaw-unknown".to_string())
        })
    }
}

pub fn agent_wrapper_dir_from_workspace(workspace_dir: &std::path::Path) -> PathBuf {
    workspace_dir
        .parent()
        .unwrap_or(workspace_dir)
        .to_path_buf()
}

pub fn agent_config_path_from_workspace(workspace_dir: &std::path::Path) -> PathBuf {
    agent_wrapper_dir_from_workspace(workspace_dir).join("config.toml")
}

pub fn promote_legacy_agent_config(
    agent_wrapper_dir: &std::path::Path,
    workspace_dir: &std::path::Path,
) -> std::io::Result<Option<PathBuf>> {
    let canonical_path = agent_wrapper_dir.join("config.toml");
    if canonical_path.exists() {
        return Ok(Some(canonical_path));
    }

    let legacy_path = workspace_dir.join("config.toml");
    if !legacy_path.exists() {
        return Ok(None);
    }

    fs::create_dir_all(agent_wrapper_dir)?;
    match fs::rename(&legacy_path, &canonical_path) {
        Ok(()) => Ok(Some(canonical_path)),
        Err(_) => {
            fs::copy(&legacy_path, &canonical_path)?;
            fs::remove_file(&legacy_path)?;
            Ok(Some(canonical_path))
        }
    }
}

pub fn promote_legacy_agent_config_from_workspace(
    workspace_dir: &std::path::Path,
) -> std::io::Result<Option<PathBuf>> {
    promote_legacy_agent_config(
        &agent_wrapper_dir_from_workspace(workspace_dir),
        workspace_dir,
    )
}

/// Per-template configuration for tenant agent creation.

/// HASN 节点连接配置 (`[huanxing.hasn]`).
///
/// 配置当前 ZeroClaw 实例作为 HASN 节点接入中央网络。
/// 桌面端和云端使用完全相同的配置结构，仅参数值不同。
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct HasnNodeConfig {
    /// 是否启用 HASN 节点功能。默认: false
    pub enabled: bool,

    /// HASN 中央节点 WS URL。
    /// 示例: `wss://api.huanxing.dcfuture.cn/api/v1/hasn/ws/node`
    pub central_url: Option<String>,

    /// 节点 API Key (hasn_ak_xxx 格式) 或 JWT token。
    pub api_key: Option<String>,

    /// 节点类型: desktop / mobile / web / cloud
    #[serde(default = "default_node_type")]
    pub node_type: String,

    /// 最大 Agent 承载量。桌面端默认 3，云端可配置更高。
    #[serde(default = "default_node_capacity")]
    pub capacity: i32,

    /// 启动时自动连接 HASN 网络。云端节点通常设为 true。
    #[serde(default)]
    pub auto_connect: bool,

    /// 最大重连次数。默认: 10
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
}

fn default_node_type() -> String {
    "desktop".to_string()
}
fn default_node_capacity() -> i32 {
    3
}
fn default_max_retries() -> u32 {
    10
}

impl Default for HasnNodeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            central_url: None,
            api_key: None,
            node_type: default_node_type(),
            capacity: default_node_capacity(),
            auto_connect: false,
            max_retries: default_max_retries(),
        }
    }
}

/// Per-template configuration for tenant agent creation.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TemplateConfig {
    /// Path to the SOUL.md template file (relative to workspace or absolute).
    pub soul: PathBuf,

    /// Tools available to agents created from this template.
    #[serde(default)]
    pub tools: Vec<String>,

    /// Model override for this template.
    pub model: Option<String>,

    /// Provider override for this template.
    pub provider: Option<String>,
}

/// Napcat (QQ via OneBot) channel configuration.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NapcatConfig {
    /// Napcat WebSocket endpoint (for example `ws://127.0.0.1:3001`)
    #[serde(alias = "ws_url")]
    pub websocket_url: String,
    /// Optional Napcat HTTP API base URL. If omitted, derived from websocket_url.
    #[serde(default)]
    pub api_base_url: String,
    /// Optional access token (Authorization Bearer token)
    pub access_token: Option<String>,
    /// Allowed user IDs. Empty = deny all, "*" = allow all
    #[serde(default)]
    pub allowed_users: Vec<String>,
}

/// WeChatPadPro (WeChat iPad protocol) channel configuration.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WechatPadConfig {
    /// WeChatPadPro REST API base URL (e.g. "http://127.0.0.1:8849")
    pub api_base_url: String,
    /// WeChatPadPro admin key for API authentication
    pub admin_key: String,
    /// Authorization token (generated from admin_key via `/admin/GanAuthKey1`)
    #[serde(default)]
    pub token: Option<String>,
    /// Webhook listener bind address. ZeroClaw starts an HTTP server on this
    /// address to receive message callbacks from WeChatPadPro.
    #[serde(default = "default_wechat_pad_webhook_bind")]
    pub webhook_bind: String,
    /// Webhook secret for HMAC-SHA256 signature verification.
    #[serde(default)]
    pub webhook_secret: Option<String>,
    /// Logged-in WeChat wxid (used to filter out self-sent messages).
    #[serde(default)]
    pub wxid: Option<String>,
    /// Allowed user wxids. `"*"` = allow all. Empty = deny all.
    #[serde(default)]
    pub allowed_users: Vec<String>,
    /// Allowed group IDs. Empty = ignore all group messages.
    #[serde(default)]
    pub allowed_groups: Vec<String>,
    /// In group chats, only respond when @-mentioned. Default: `true`.
    #[serde(default = "default_true")]
    pub group_at_only: bool,
    /// Max messages per minute rate limit. Default: `20`.
    #[serde(default = "default_wechat_pad_rate_limit")]
    pub rate_limit_per_minute: u32,
}

fn default_wechat_pad_webhook_bind() -> String {
    "0.0.0.0:9850".to_string()
}

fn default_wechat_pad_rate_limit() -> u32 {
    20
}

fn default_true() -> bool {
    true
}

/// Hub Gitee 同步配置。
///
/// 控制从 Gitee 拉取 huanxing-hub 仓库（模板和技能）的行为。
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct HubSyncConfig {
    /// Gitee 仓库路径（"owner/repo"）。
    /// 默认：`"huanxing-team/huanxing-hub"`
    pub gitee_repo: String,

    /// 同步的分支名。默认：`"main"`
    pub gitee_branch: String,

    /// 启动时自动检查并同步。默认：`true`
    pub auto_sync_on_startup: bool,

    /// 超过多少小时后触发自动同步。默认：`24`
    pub sync_interval_hours: u64,
}

impl Default for HubSyncConfig {
    fn default() -> Self {
        Self {
            gitee_repo: "huanxing-team/huanxing-hub".to_string(),
            gitee_branch: "main".to_string(),
            auto_sync_on_startup: true,
            sync_interval_hours: 24,
        }
    }
}

/// 微信（iLink AI）渠道配置。
///
/// 通过桌面端扫码登录后，凭证自动写入此配置节；
/// ZeroClaw 启动时读取此节创建 `WeixinChannel` 实例。
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WeixinConfig {
    /// iLink API 的 bot_token（扫码登录后获取）。
    pub bot_token: String,
    /// iLink bot ID。
    #[serde(default)]
    pub bot_id: String,
    /// iLink API base URL（扫码登录时由服务端返回）。
    #[serde(default = "default_weixin_base_url")]
    pub base_url: String,
}

fn default_weixin_base_url() -> String {
    "https://ilinkai.weixin.qq.com".to_string()
}
