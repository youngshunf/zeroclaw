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
        let config_dir = global_config.config_path.parent().unwrap_or(&global_config.workspace_dir);
        let db_path = config.resolve_db_path(config_dir);
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
        //    Admin is accessed via pre-configured designated channels (e.g. feishu).
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

                // Resolve paths using the unified architecture.
                // Root: config_dir/users/{tenant_dir}/...
                let tenant_dir_str = record.tenant_dir.as_deref();
                let owner_dir = self.config.resolve_owner_dir(&config_dir, tenant_dir_str);
                let agent_workspace = self.config.resolve_agent_workspace(
                    &config_dir, tenant_dir_str, &record.agent_id,
                );

                match TenantContext::load(
                    &record.agent_id,
                    &record.user_id,
                    agent_workspace.clone(),
                    owner_dir,
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
                            ?tenant_dir_str,
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
            // 3. No binding found — route to Guardian.
            //    Guardian guides unregistered users through:
            //    - Phone verification & registration
            //    - Agent selection/creation from marketplace
            //    - Route binding (writes channels table)
            //    - Cache invalidation so next message routes normally
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
            let sys = collect_system_metrics();
            let mut payload = serde_json::json!({
                "server_id": server_id,
                "server_name": server_id,
                "gateway_status": "running",
                "user_count": stats.total_users,
                "active_user_count": stats.active_users,
                "cpu_usage": sys.cpu_usage,
                "memory_usage": sys.memory_usage,
                "total_memory_gb": sys.total_memory_gb,
                "zeroclaw_version": env!("CARGO_PKG_VERSION"),
            });
            if let Some(disk) = sys.disk_usage {
                payload["disk_usage"] = serde_json::json!(disk);
            }
            if let Some(disk_gb) = sys.total_disk_gb {
                payload["total_disk_gb"] = serde_json::json!(disk_gb);
            }
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
                    "total_memory_gb": sys.total_memory_gb,
                    "zeroclaw_version": env!("CARGO_PKG_VERSION"),
                });
                if let Some(disk) = sys.disk_usage {
                    hb["disk_usage"] = serde_json::json!(disk);
                }
                if let Some(disk_gb) = sys.total_disk_gb {
                    hb["total_disk_gb"] = serde_json::json!(disk_gb);
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
    total_memory_gb: f64,
    disk_usage: Option<f64>,
    total_disk_gb: Option<f64>,
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
    let (memory_usage, total_memory_gb) = {
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
                let usage = if total > 0 {
                    ((total - available) as f64 / total as f64 * 100.0 * 100.0).round() / 100.0
                } else {
                    0.0
                };
                let total_gb = (total as f64 / (1024.0 * 1024.0) * 100.0).round() / 100.0;
                (usage, total_gb)
            } else {
                (0.0, 0.0)
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            (0.0, 0.0)
        }
    };

    // Disk: run `df -k /`
    let (disk_usage, total_disk_gb) = {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            if let Ok(output) = std::process::Command::new("df").arg("-k").arg("/").output() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let mut lines = stdout.lines();
                let _header = lines.next();
                if let Some(data) = lines.next() {
                    let parts: Vec<&str> = data.split_whitespace().collect();
                    if parts.len() >= 4 {
                        let tz_block = if cfg!(target_os = "macos") { parts[1] } else { parts[1] };
                        let used_block = if cfg!(target_os = "macos") { parts[2] } else { parts[2] };
                        if let (Ok(total_k), Ok(used_k)) = (tz_block.parse::<f64>(), used_block.parse::<f64>()) {
                            if total_k > 0.0 {
                                let usage = (used_k / total_k * 100.0 * 100.0).round() / 100.0;
                                let total_gb = (total_k / (1024.0 * 1024.0) * 100.0).round() / 100.0;
                                (Some(usage), Some(total_gb))
                            } else {
                                (None, None)
                            }
                        } else {
                            (None, None)
                        }
                    } else {
                        (None, None)
                    }
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            }
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            (None, None)
        }
    };

    SystemMetrics {
        cpu_usage,
        memory_usage,
        total_memory_gb,
        disk_usage,
        total_disk_gb,
    }
}
