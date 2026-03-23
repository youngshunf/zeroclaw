//! HuanXing multi-tenant configuration.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Top-level `[huanxing]` configuration section in config.toml.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct HuanXingConfig {
    /// Enable multi-tenant routing. When false, behaves as standard single-agent.
    pub enabled: bool,

    /// Path to the SQLite user database.
    /// Default: `{workspace}/data/users.db`
    pub db_path: Option<PathBuf>,

    /// Root directory for per-tenant agent workspaces.
    /// Each tenant gets `{agents_dir}/{agent_id}/`.
    /// Default: `{workspace}/agents`
    pub agents_dir: Option<PathBuf>,

    /// Workspace directory for the Guardian agent (handles unregistered users).
    /// Default: `{workspace}/guardian`
    pub guardian_workspace: Option<PathBuf>,

    /// Workspace directory for the Admin agent (server management).
    /// Default: `{workspace}/admin`
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
    /// Default: `{workspace}/templates`
    pub templates_dir: Option<PathBuf>,

    /// Directory for user data backups.
    /// Default: `{workspace}/backups`
    pub backup_dir: Option<PathBuf>,

    /// Directory for common skills shared across all user agents.
    /// Default: `{workspace}/common-skills`
    pub common_skills_dir: Option<PathBuf>,

    /// Path to the huanxing-hub repository (skill marketplace).
    /// Contains `registry.json`, `skills/`, `templates/`.
    /// When set, enables registry-based skill loading and marketplace tools.
    pub hub_dir: Option<PathBuf>,

    /// Hub Gitee 同步配置。
    #[serde(default)]
    pub hub_sync: HubSyncConfig,

    /// Multi-tenant heartbeat configuration.
    #[serde(default)]
    pub tenant_heartbeat: TenantHeartbeatConfig,
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
        }
    }
}

impl HuanXingConfig {
    /// Resolve the database path, using workspace_dir as base if not absolute.
    pub fn resolve_db_path(&self, workspace_dir: &std::path::Path) -> PathBuf {
        self.db_path
            .clone()
            .unwrap_or_else(|| workspace_dir.join("data").join("users.db"))
    }

    /// Resolve the agents directory.
    ///
    /// 默认值为 `config_dir/agents`（即 `~/.huanxing/agents`），
    /// 而不是 `workspace_dir/agents`（workspace 是 ZeroClaw 原版单 Agent 工作区）。
    pub fn resolve_agents_dir(&self, config_dir: &std::path::Path) -> PathBuf {
        self.agents_dir
            .clone()
            .unwrap_or_else(|| config_dir.join("agents"))
    }

    /// Resolve the guardian workspace.
    pub fn resolve_guardian_workspace(&self, workspace_dir: &std::path::Path) -> PathBuf {
        self.guardian_workspace
            .clone()
            .unwrap_or_else(|| workspace_dir.join("guardian"))
    }

    /// Resolve the admin workspace.
    pub fn resolve_admin_workspace(&self, workspace_dir: &std::path::Path) -> PathBuf {
        self.admin_workspace
            .clone()
            .unwrap_or_else(|| workspace_dir.join("admin"))
    }

    /// Check if a channel type is routed to the Admin agent.
    pub fn is_admin_channel(&self, channel_type: &str) -> bool {
        self.admin_channels.iter().any(|c| c == channel_type)
    }

    /// Resolve the templates directory.
    pub fn resolve_templates_dir(&self, workspace_dir: &std::path::Path) -> PathBuf {
        self.templates_dir
            .clone()
            .unwrap_or_else(|| workspace_dir.join("templates"))
    }

    /// Resolve the backup directory.
    pub fn resolve_backup_dir(&self, workspace_dir: &std::path::Path) -> PathBuf {
        self.backup_dir
            .clone()
            .unwrap_or_else(|| workspace_dir.join("backups"))
    }

    /// Resolve the common skills directory.
    /// Auto-resolves to `{workspace}/common-skills` when not configured.
    pub fn resolve_common_skills_dir(&self, workspace_dir: &std::path::Path) -> PathBuf {
        self.common_skills_dir
            .clone()
            .unwrap_or_else(|| workspace_dir.join("common-skills"))
    }

    /// Resolve the hub directory (skill marketplace repository).
    /// Returns None if not configured.
    pub fn resolve_hub_dir(&self) -> Option<PathBuf> {
        self.hub_dir.clone()
    }

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
