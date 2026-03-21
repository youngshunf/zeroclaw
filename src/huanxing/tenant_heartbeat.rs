//! Multi-tenant heartbeat manager.
//!
//! Scans all active tenants' `HEARTBEAT.md` files on a configurable interval,
//! checks each task's `schedule:cron` expression against the current time,
//! and executes matching tasks using the tenant's agent context.
//! Results are delivered through the tenant's bound channel (QQ/Feishu/etc).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, Utc};
use tokio::sync::Semaphore;
use tokio::time::{self, Duration};
use tracing::{error, info, warn};

use crate::channels::get_live_channel;
use crate::channels::traits::SendMessage;
use crate::config::Config;
use crate::huanxing::config::TenantHeartbeatConfig;
use crate::huanxing::db::{ChannelRecord, TenantDb, TenantRecord, UserFilter};
use crate::observability::{Observer, ObserverEvent};

/// Manages heartbeat scheduling for all active tenants.
pub struct TenantHeartbeatManager {
    config: Config,
    tenant_heartbeat_config: TenantHeartbeatConfig,
    db: Arc<TenantDb>,
    observer: Arc<dyn Observer>,
    /// Track last execution per (user_id, task_hash) to avoid re-running
    /// the same task within the same schedule window.
    task_last_run: tokio::sync::RwLock<HashMap<(String, u64), chrono::DateTime<chrono::Utc>>>,
}

impl TenantHeartbeatManager {
    /// Create a new tenant heartbeat manager.
    ///
    /// Returns `None` if HuanXing is not enabled or the DB cannot be opened.
    pub fn new(config: Config) -> Result<Self> {
        let hx_config = &config.huanxing;

        if !hx_config.enabled {
            anyhow::bail!("HuanXing is not enabled");
        }

        let tenant_heartbeat_config = hx_config.tenant_heartbeat.clone();
        let db_path = hx_config.resolve_db_path(&config.workspace_dir);
        let db = Arc::new(TenantDb::open(&db_path)?);

        let observer: Arc<dyn Observer> =
            Arc::from(crate::observability::create_observer(&config.observability));

        Ok(Self {
            config,
            tenant_heartbeat_config,
            db,
            observer,
            task_last_run: tokio::sync::RwLock::new(HashMap::new()),
        })
    }

    /// Run the tenant heartbeat loop (runs until cancelled).
    pub async fn run(&self) -> Result<()> {
        if !self.tenant_heartbeat_config.enabled {
            info!("Tenant heartbeat disabled");
            return Ok(());
        }

        let interval_mins = self.tenant_heartbeat_config.scan_interval_minutes.max(1);
        info!(
            "💓 Tenant heartbeat started: scanning every {} minutes, max {} concurrent",
            interval_mins, self.tenant_heartbeat_config.max_concurrent
        );

        let mut interval = time::interval(Duration::from_secs(u64::from(interval_mins) * 60));

        loop {
            interval.tick().await;

            match self.scan_and_execute().await {
                Ok(executed) => {
                    if executed > 0 {
                        info!(
                            "💓 Tenant heartbeat: executed tasks for {} tenants",
                            executed
                        );
                    }
                }
                Err(e) => {
                    error!("💓 Tenant heartbeat scan error: {}", e);
                    self.observer.record_event(&ObserverEvent::Error {
                        component: "tenant-heartbeat".into(),
                        message: e.to_string(),
                    });
                }
            }
        }
    }

    /// Scan all active tenants and execute matching heartbeat tasks.
    /// Returns the number of tenants that had tasks executed.
    async fn scan_and_execute(&self) -> Result<usize> {
        let now = Utc::now();
        let window_minutes = self.tenant_heartbeat_config.scan_interval_minutes;

        // Load all active users
        let filter = UserFilter {
            status: Some("active".to_string()),
            limit: Some(500), // Reasonable upper bound
            ..Default::default()
        };
        let (users, total) = self.db.list_users(&filter).await?;
        if users.is_empty() {
            return Ok(0);
        }
        info!(
            "💓 Tenant heartbeat: scanning {} active users (total: {})",
            users.len(),
            total
        );

        let semaphore = Arc::new(Semaphore::new(self.tenant_heartbeat_config.max_concurrent));
        let mut handles = Vec::new();
        let mut executed_count = 0usize;

        for user in users {
            let workspace_dir = self.resolve_workspace(&user);
            let heartbeat_path = workspace_dir.join("HEARTBEAT.md");

            // Skip tenants without HEARTBEAT.md
            if !heartbeat_path.exists() {
                continue;
            }

            // Read and parse scheduled tasks (huanxing-owned cron parsing)
            let scheduled_tasks =
                match collect_scheduled_tasks(&heartbeat_path, now, window_minutes) {
                    Ok(tasks) => tasks,
                    Err(e) => {
                        warn!(
                            "💓 Tenant {}: failed to collect tasks: {}",
                            user.agent_id, e
                        );
                        continue;
                    }
                };

            if scheduled_tasks.is_empty() {
                continue;
            }

            // Filter out tasks that were already executed in this window
            let mut tasks_to_run = Vec::new();
            {
                let last_run = self.task_last_run.read().await;
                for task in &scheduled_tasks {
                    let task_hash = Self::hash_task(&task.text);
                    let key = (user.user_id.clone(), task_hash);
                    if let Some(last) = last_run.get(&key) {
                        // Skip if executed within the scan window
                        let elapsed = now.signed_duration_since(*last);
                        if elapsed.num_minutes() < i64::from(window_minutes) {
                            continue;
                        }
                    }
                    tasks_to_run.push(task.clone());
                }
            }

            if tasks_to_run.is_empty() {
                continue;
            }

            // Get delivery channel
            let channels = match self.db.get_channels(&user.user_id).await {
                Ok(ch) => ch,
                Err(e) => {
                    warn!("💓 Tenant {}: failed to get channels: {}", user.agent_id, e);
                    continue;
                }
            };

            if channels.is_empty() {
                warn!("💓 Tenant {}: no bound channels, skipping", user.agent_id);
                continue;
            }

            executed_count += 1;

            // Execute tasks with concurrency control
            let permit = Arc::clone(&semaphore);
            let config = self.config.clone();
            let user_clone = user.clone();
            let workspace = workspace_dir.clone();
            let timeout_secs = self.tenant_heartbeat_config.per_tenant_timeout_secs;
            let task_last_run = &self.task_last_run;

            // Record task execution times before spawning
            {
                let mut last_run = task_last_run.write().await;
                for task in &tasks_to_run {
                    let task_hash = Self::hash_task(&task.text);
                    last_run.insert((user.user_id.clone(), task_hash), now);
                }
            }

            let handle = tokio::spawn(async move {
                let _permit = permit.acquire().await;

                let task_descriptions: Vec<String> =
                    tasks_to_run.iter().map(|t| t.text.clone()).collect();

                let prompt = format!(
                    "[heartbeat] 以下定时任务已触发，请立即执行：\n\n{}",
                    task_descriptions
                        .iter()
                        .enumerate()
                        .map(|(i, t)| format!("{}. {}", i + 1, t))
                        .collect::<Vec<_>>()
                        .join("\n")
                );

                info!(
                    "💓 Tenant {}: executing {} scheduled tasks",
                    user_clone.agent_id,
                    tasks_to_run.len()
                );

                // Build per-tenant config
                let tenant_config = build_tenant_config(&config, &user_clone, &workspace);

                // Execute with timeout
                let result = tokio::time::timeout(
                    Duration::from_secs(timeout_secs),
                    crate::agent::loop_::run(
                        tenant_config,
                        Some(prompt),
                        None,
                        None,
                        config.default_temperature,
                        vec![],
                        false,
                        None,
                        None,
                    ),
                )
                .await;

                let output = match result {
                    Ok(Ok(response)) => {
                        if response.trim().is_empty()
                            || response.trim().eq_ignore_ascii_case("HEARTBEAT_OK")
                        {
                            None
                        } else {
                            Some(response)
                        }
                    }
                    Ok(Err(e)) => {
                        error!(
                            "💓 Tenant {}: agent execution failed: {}",
                            user_clone.agent_id, e
                        );
                        None
                    }
                    Err(_) => {
                        error!(
                            "💓 Tenant {}: agent execution timed out ({}s)",
                            user_clone.agent_id, timeout_secs
                        );
                        None
                    }
                };

                // Deliver result to tenant's channel
                if let Some(ref message) = output {
                    if let Err(e) = deliver_to_channels(&channels, message).await {
                        error!("💓 Tenant {}: delivery failed: {}", user_clone.agent_id, e);
                    } else {
                        info!(
                            "💓 Tenant {}: delivered heartbeat message",
                            user_clone.agent_id
                        );
                    }
                }
            });

            handles.push(handle);
        }

        // Wait for all spawned tasks
        for handle in handles {
            if let Err(e) = handle.await {
                error!("💓 Tenant heartbeat task panicked: {}", e);
            }
        }

        // Periodically clean up old entries from task_last_run
        self.cleanup_old_entries().await;

        Ok(executed_count)
    }

    /// Resolve workspace directory for a tenant.
    fn resolve_workspace(&self, tenant: &TenantRecord) -> PathBuf {
        if let Some(ref ws) = tenant.workspace {
            PathBuf::from(ws)
        } else {
            let hx = &self.config.huanxing;
            let config_dir = self.config.config_path.parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| self.config.workspace_dir.clone());
            hx.resolve_agents_dir(&config_dir)
                .join(&tenant.agent_id)
        }
    }

    /// Simple hash of task text for deduplication.
    fn hash_task(text: &str) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        text.hash(&mut hasher);
        hasher.finish()
    }

    /// Remove entries older than 24 hours from the task_last_run map.
    async fn cleanup_old_entries(&self) {
        let now = Utc::now();
        let mut last_run = self.task_last_run.write().await;
        last_run.retain(|_, v| now.signed_duration_since(*v).num_hours() < 24);
    }
}

/// Build a Config suitable for running `agent::run` in a tenant's context.
///
/// Strategy: load the tenant's full config.toml if present, then set workspace_dir.
/// Falls back to base config with workspace override if tenant has no config.toml.
fn build_tenant_config(
    base: &Config,
    _tenant: &TenantRecord,
    workspace_dir: &std::path::Path,
) -> Config {
    let tenant_config_path = workspace_dir.join("config.toml");

    let mut config = if tenant_config_path.exists() {
        // Load the tenant's full config.toml
        match std::fs::read_to_string(&tenant_config_path) {
            Ok(contents) => match toml::from_str::<Config>(&contents) {
                Ok(tenant_cfg) => tenant_cfg,
                Err(e) => {
                    tracing::warn!(
                        "Failed to parse tenant config at {}: {}, falling back to base",
                        tenant_config_path.display(),
                        e
                    );
                    base.clone()
                }
            },
            Err(e) => {
                tracing::warn!(
                    "Failed to read tenant config at {}: {}, falling back to base",
                    tenant_config_path.display(),
                    e
                );
                base.clone()
            }
        }
    } else {
        base.clone()
    };

    // Always override workspace_dir to the tenant's directory
    config.workspace_dir = workspace_dir.to_path_buf();

    // Inherit HuanXing config from base (tenant configs don't have [huanxing])
    config.huanxing = base.huanxing.clone();

    config
}

/// Deliver a message to a tenant via their bound channels.
/// Tries each channel in order until one succeeds.
async fn deliver_to_channels(channels: &[ChannelRecord], message: &str) -> Result<()> {
    for ch in channels {
        if let Some(channel) = get_live_channel(&ch.channel_type) {
            match channel.send(&SendMessage::new(message, &ch.peer_id)).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    warn!(
                        "Channel {} delivery to {} failed: {}, trying next",
                        ch.channel_type, ch.peer_id, e
                    );
                    continue;
                }
            }
        }
    }

    anyhow::bail!(
        "No live channel could deliver message (tried {} channels)",
        channels.len()
    )
}

// ── Huanxing-owned scheduled task parsing ─────────────────────────
// Parses HEARTBEAT.md with `schedule:cron` support without modifying
// the upstream HeartbeatTask struct.

/// A scheduled heartbeat task with cron expression.
#[derive(Debug, Clone)]
struct ScheduledTask {
    pub text: String,
    pub schedule: String,
}

/// Read HEARTBEAT.md and collect tasks whose cron schedule matches `now`.
///
/// Task format in HEARTBEAT.md:
///   `- [high|schedule:*/5 * * * *] Check email`
///   `- [schedule:0 9 * * 1-5] Morning standup reminder`
///   `- [active|schedule:0 */2 * * *] Sync data`
///
/// Only tasks with a `schedule:` tag and status != paused/completed are returned.
fn collect_scheduled_tasks(
    heartbeat_path: &Path,
    now: DateTime<Utc>,
    window_minutes: u32,
) -> Result<Vec<ScheduledTask>> {
    let content = std::fs::read_to_string(heartbeat_path)?;
    let mut tasks = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        let Some(text) = trimmed.strip_prefix("- ") else {
            continue;
        };
        if text.is_empty() {
            continue;
        }

        // Parse [meta] prefix if present
        let Some(rest) = text.strip_prefix('[') else {
            continue; // No metadata = no schedule, skip
        };
        let Some((meta, task_text)) = rest.split_once(']') else {
            continue;
        };
        let task_text = task_text.trim();
        if task_text.is_empty() {
            continue;
        }

        // Parse meta tags: look for schedule: and check status
        let mut schedule: Option<String> = None;
        let mut is_paused = false;

        for part in meta.split('|') {
            let part = part.trim();
            if let Some(cron_expr) = part.strip_prefix("schedule:") {
                let expr = cron_expr.trim();
                if !expr.is_empty() {
                    schedule = Some(expr.to_string());
                }
            } else {
                match part.to_ascii_lowercase().as_str() {
                    "paused" | "pause" | "completed" | "complete" | "done" => {
                        is_paused = true;
                    }
                    _ => {}
                }
            }
        }

        // Only include tasks with a schedule that are active
        if is_paused {
            continue;
        }
        let Some(cron_expr) = schedule else {
            continue;
        };

        // Check if cron matches current time window
        if schedule_matches(&cron_expr, now, window_minutes) {
            tasks.push(ScheduledTask {
                text: task_text.to_string(),
                schedule: cron_expr,
            });
        }
    }

    Ok(tasks)
}

/// Check if a cron expression matches within [now - window, now].
fn schedule_matches(expr: &str, now: DateTime<Utc>, window_minutes: u32) -> bool {
    match crate::cron::normalize_expression(expr) {
        Ok(normalized) => match cron::Schedule::from_str(&normalized) {
            Ok(cron_schedule) => {
                let window = chrono::Duration::minutes(i64::from(window_minutes));
                let window_start = now - window;
                for next in cron_schedule.after(&window_start).take(5) {
                    if next > now {
                        break;
                    }
                    return true;
                }
                false
            }
            Err(e) => {
                warn!("Invalid cron schedule '{expr}': {e}");
                false
            }
        },
        Err(e) => {
            warn!("Failed to normalize cron expression '{expr}': {e}");
            false
        }
    }
}
