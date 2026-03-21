//! Multi-tenant message router.
//!
//! The [`TenantRouter`] is the core of HuanXing multi-tenancy. On every inbound
//! message it resolves `(channel_name, sender_id)` → [`TenantContext`] by:
//!
//! 1. Checking an in-memory cache (RwLock<HashMap> for concurrent reads).
//! 2. Querying the SQLite database on cache miss.
//! 3. Falling back to the Guardian context for unregistered senders.
//!
//! The resolved [`TenantContext`] determines which system prompt, workspace,
//! model, and provider are used for the agent loop — effectively giving each
//! registered user their own AI assistant while sharing channel connections
//! and the LLM provider pool.

use super::config::HuanXingConfig;
use super::db::TenantDb;
use super::tenant::TenantContext;
use super::ApiClient;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use crate::config::Config;

/// Cache key: `"{channel}:{sender_id}"`.
fn cache_key(channel: &str, sender_id: &str) -> String {
    format!("{channel}:{sender_id}")
}

/// Multi-tenant message router.
pub struct TenantRouter {
    db: TenantDb,
    config: HuanXingConfig,
    workspace_dir: PathBuf,
    /// Global zeroclaw config — passed to TenantContext::load() for memory/session/prompt setup.
    global_config: Arc<Config>,
    /// In-memory cache: cache_key → TenantContext.
    cache: RwLock<HashMap<String, Arc<TenantContext>>>,
    /// Pre-loaded guardian context.
    guardian: Arc<TenantContext>,
    /// Pre-loaded admin context (if admin workspace exists).
    admin: Option<Arc<TenantContext>>,
}

impl TenantRouter {
    /// Initialize the router: open DB, load guardian context.
    pub async fn new(
        config: HuanXingConfig,
        workspace_dir: PathBuf,
        global_config: Arc<Config>,
    ) -> anyhow::Result<Self> {
        let db_path = config.resolve_db_path(&workspace_dir);
        let db = TenantDb::open(&db_path)?;

        let guardian_dir = config.resolve_guardian_workspace(&workspace_dir);
        let guardian = Arc::new(TenantContext::guardian(guardian_dir, &global_config).await?);

        // Load admin context if admin workspace exists.
        let admin_dir = config.resolve_admin_workspace(&workspace_dir);
        let admin = if admin_dir.join("SOUL.md").exists() {
            match TenantContext::admin(admin_dir.clone(), &global_config).await {
                Ok(ctx) => {
                    tracing::info!(
                        admin_dir = %admin_dir.display(),
                        admin_channels = ?config.admin_channels,
                        "Admin agent loaded"
                    );
                    Some(Arc::new(ctx))
                }
                Err(e) => {
                    tracing::warn!(
                        admin_dir = %admin_dir.display(),
                        "Failed to load admin agent: {e}, admin features disabled"
                    );
                    None
                }
            }
        } else {
            tracing::debug!(
                "No admin workspace at {}, admin features disabled",
                admin_dir.display()
            );
            None
        };

        tracing::info!(
            db_path = %db_path.display(),
            guardian_dir = %guardian.workspace_dir.display(),
            has_admin = admin.is_some(),
            "HuanXing tenant router initialized"
        );

        Ok(Self {
            db,
            config,
            workspace_dir,
            global_config,
            cache: RwLock::new(HashMap::new()),
            guardian,
            admin,
        })
    }

    /// Resolve a tenant context for the given channel message.
    /// This is the hot path — called on every inbound message.
    pub async fn resolve(&self, channel: &str, sender_id: &str) -> Arc<TenantContext> {
        // 0. Admin channel — route directly to admin agent.
        if self.config.is_admin_channel(channel) {
            if let Some(admin) = &self.admin {
                return admin.clone();
            }
            // Admin workspace not configured, fall through to normal routing.
        }

        let key = cache_key(channel, sender_id);

        // 1. Cache hit — fast path (read lock only).
        {
            let cache = self.cache.read().unwrap_or_else(|e| e.into_inner());
            if let Some(ctx) = cache.get(&key) {
                return ctx.clone();
            }
        }

        // 2. Cache miss — query DB.
        match self.db.find_by_channel(channel, sender_id).await {
            Ok(Some(record)) => {
                let config_dir = self.global_config.config_path.parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| self.workspace_dir.clone());
                let agents_dir = self.config.resolve_agents_dir(&config_dir);
                let tenant_workspace = agents_dir.join(&record.agent_id);

                match TenantContext::load(
                    &record.agent_id,
                    &record.user_id,
                    tenant_workspace,
                    self.config.default_model.clone(),
                    self.config.default_provider.clone(),
                    record.template.clone(),
                    record.nickname.clone(),
                    record.star_name.clone(),
                    record.plan.clone(),
                    &self.global_config,
                )
                .await
                {
                    Ok(ctx) => {
                        let ctx = Arc::new(ctx);
                        let mut cache = self.cache.write().unwrap_or_else(|e| e.into_inner());
                        cache.insert(key, ctx.clone());
                        tracing::info!(
                            agent_id = %ctx.agent_id,
                            channel,
                            sender_id,
                            "Tenant context loaded from DB"
                        );
                        ctx
                    }
                    Err(e) => {
                        tracing::warn!(
                            channel,
                            sender_id,
                            agent_id = %record.agent_id,
                            "Failed to load tenant context: {e}, falling back to guardian"
                        );
                        self.guardian.clone()
                    }
                }
            }
            Ok(None) => {
                tracing::debug!(channel, sender_id, "No tenant found, using guardian");
                self.guardian.clone()
            }
            Err(e) => {
                tracing::warn!(
                    channel,
                    sender_id,
                    "Tenant DB lookup failed: {e}, falling back to guardian"
                );
                self.guardian.clone()
            }
        }
    }

    /// Invalidate cache for a specific sender (e.g. after registration).
    pub fn invalidate(&self, channel: &str, sender_id: &str) {
        let key = cache_key(channel, sender_id);
        let mut cache = self.cache.write().unwrap_or_else(|e| e.into_inner());
        cache.remove(&key);
        tracing::debug!(channel, sender_id, "Tenant cache invalidated");
    }

    /// Invalidate all cached contexts for a user (across all channels).
    pub fn invalidate_user(&self, user_id: &str) {
        let mut cache = self.cache.write().unwrap_or_else(|e| e.into_inner());
        cache.retain(|_key, ctx| ctx.user_id != user_id);
        tracing::debug!(user_id, "All tenant cache entries invalidated for user");
    }

    /// Invalidate cached tenant contexts by agent_id.
    pub fn invalidate_agent(&self, agent_id: &str) {
        let mut cache = self.cache.write().unwrap_or_else(|e| e.into_inner());
        cache.retain(|_key, ctx| ctx.agent_id != agent_id);
        tracing::debug!(agent_id, "Tenant cache entries invalidated for agent");
    }

    /// Invalidate the entire cache (e.g. after config reload).
    pub fn invalidate_all(&self) {
        let mut cache = self.cache.write().unwrap_or_else(|e| e.into_inner());
        cache.clear();
        tracing::info!("Tenant cache fully cleared");
    }

    /// Get the guardian context directly.
    pub fn guardian(&self) -> Arc<TenantContext> {
        self.guardian.clone()
    }

    /// Number of cached tenant contexts.
    pub fn cache_size(&self) -> usize {
        let cache = self.cache.read().unwrap_or_else(|e| e.into_inner());
        cache.len()
    }

    /// Check if multi-tenant routing is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get a reference to the configuration.
    pub fn config(&self) -> &HuanXingConfig {
        &self.config
    }

    /// Get a reference to the database.
    pub fn db(&self) -> &TenantDb {
        &self.db
    }

    /// Start server lifecycle: register with backend + periodic heartbeat.
    ///
    /// Mirrors OpenClaw's `autoRegisterServer()` in `server.ts`.
    /// Call this **once** after initialization (non-blocking, spawns background task).
    pub fn start_server_lifecycle(&self) {
        let Some(ref agent_key) = self.config.agent_key else {
            tracing::info!(
                "HuanXing agent_key not configured, skipping server registration & heartbeat"
            );
            return;
        };

        let api = ApiClient::new(
            self.config.api_url(),
            agent_key,
            &self.config.server_id_or_hostname(),
        );
        let db = self.db.clone();
        let server_id = self.config.server_id_or_hostname();
        let server_ip = self.config.server_ip.clone();
        let heartbeat_interval = self.config.heartbeat_interval();

        tokio::spawn(async move {
            // ── Step 1: Register server ──────────────────────────────
            let stats = db.get_stats().await.unwrap_or_default();
            let mut payload = serde_json::json!({
                "server_id": server_id,
                "server_name": server_id,
                "gateway_status": "running",
                "user_count": stats.total_users,
                "active_user_count": stats.active_users,
            });
            if let Some(ref ip) = server_ip {
                payload["ip_address"] = serde_json::Value::String(ip.clone());
            }

            match api
                .agent_post("/api/v1/huanxing/agent/servers/register", &payload)
                .await
            {
                Ok(_) => tracing::info!(server_id = %server_id, "Server registered to backend"),
                Err(e) => tracing::warn!("Server auto-registration failed (non-fatal): {e}"),
            }

            // ── Step 2: Periodic heartbeat ───────────────────────────
            // Wait 30s after startup before first heartbeat
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;

            loop {
                let stats = db.get_stats().await.unwrap_or_default();
                let sys = collect_system_metrics();

                let mut hb = serde_json::json!({
                    "gateway_status": "running",
                    "user_count": stats.total_users,
                    "active_user_count": stats.active_users,
                    "cpu_usage": sys.cpu_usage,
                    "memory_usage": sys.memory_usage,
                });
                if let Some(disk) = sys.disk_usage {
                    hb["disk_usage"] = serde_json::json!(disk);
                }

                let path = format!("/api/v1/huanxing/agent/servers/{}/heartbeat", server_id);
                if let Err(e) = api.agent_post(&path, &hb).await {
                    tracing::warn!("Heartbeat failed: {e}");
                } else {
                    tracing::debug!(
                        "Heartbeat sent (users={}, active={})",
                        stats.total_users,
                        stats.active_users
                    );
                }

                tokio::time::sleep(heartbeat_interval).await;
            }
        });

        tracing::info!(
            interval_secs = self.config.heartbeat_interval().as_secs(),
            "Server lifecycle started (register + heartbeat)"
        );
    }
}

// ── System metrics collection ────────────────────────────────

struct SystemMetrics {
    cpu_usage: f64,
    memory_usage: f64,
    disk_usage: Option<f64>,
}

fn collect_system_metrics() -> SystemMetrics {
    // CPU: parse 1-minute load average from /proc/loadavg (Linux only)
    let cpu_usage = {
        #[cfg(target_os = "linux")]
        {
            let load = std::fs::read_to_string("/proc/loadavg")
                .ok()
                .and_then(|s| {
                    s.split_whitespace()
                        .next()
                        .and_then(|v| v.parse::<f64>().ok())
                })
                .unwrap_or(0.0);
            let cpus = std::thread::available_parallelism()
                .map(|n| n.get() as f64)
                .unwrap_or(1.0);
            ((load / cpus * 100.0).min(100.0) * 100.0).round() / 100.0
        }
        #[cfg(not(target_os = "linux"))]
        {
            0.0
        }
    };

    // Memory: parse /proc/meminfo (Linux only)
    let memory_usage = {
        #[cfg(target_os = "linux")]
        {
            if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
                let mut total: u64 = 0;
                let mut available: u64 = 0;
                for line in content.lines() {
                    if line.starts_with("MemTotal:") {
                        total = line
                            .split_whitespace()
                            .nth(1)
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0);
                    } else if line.starts_with("MemAvailable:") {
                        available = line
                            .split_whitespace()
                            .nth(1)
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0);
                    }
                }
                if total > 0 {
                    ((total - available) as f64 / total as f64 * 100.0 * 100.0).round() / 100.0
                } else {
                    0.0
                }
            } else {
                0.0
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            0.0
        }
    };

    SystemMetrics {
        cpu_usage,
        memory_usage,
        disk_usage: None,
    }
}
