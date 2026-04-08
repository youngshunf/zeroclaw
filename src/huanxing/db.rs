//! SQLite database operations for HuanXing tenant management.
//!
//! Reads from the existing HuanXing `users.db` schema (tables: `users`, `channels`).

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

/// A registered tenant (user) record.
#[derive(Debug, Clone)]
pub struct TenantRecord {
    /// Internal user ID (UUID).
    pub user_id: String,
    /// Agent ID (e.g. "001-18611348367-finance").
    pub agent_id: String,
    /// HASN Human ID (h_{uuid}), populated after HASN identity registration.
    pub hasn_id: Option<String>,
    /// Display name / nickname.
    pub nickname: Option<String>,
    /// Phone number.
    pub phone: Option<String>,
    /// Template used for this tenant.
    pub template: Option<String>,
    /// Subscription plan.
    pub plan: Option<String>,
    /// Account status (active/disabled).
    pub status: String,
    /// Custom AI name.
    pub star_name: Option<String>,
    /// Tenant directory name in `{seq}-{phone}` format (e.g. "001-13888888888").
    /// Used to resolve the tenant root: `{config_dir}/users/{tenant_dir}/`.
    pub tenant_dir: Option<String>,
    /// Plan expiry date.
    pub plan_expires: Option<String>,
    /// Created at timestamp.
    pub created_at: Option<String>,
    /// Last active timestamp.
    pub last_active: Option<String>,
    /// Backend access token.
    pub access_token: Option<String>,
    /// LLM gateway token (used as API key for user agents).
    pub llm_token: Option<String>,
    /// Gateway token.
    pub gateway_token: Option<String>,
    /// Token expiry.
    pub token_expires: Option<String>,
    /// Node ID (device fingerprint derived).
    pub node_id: Option<String>,
}

/// Verified credentials stored after phone-login, consumed by register.
#[derive(Debug, Clone)]
pub struct VerifiedCredentials {
    pub phone: String,
    pub user_id: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub llm_token: Option<String>,
    pub gateway_token: Option<String>,
    pub is_new_user: bool,
}

/// Channel binding record.
#[derive(Debug, Clone)]
pub struct RoutingRecord {
    pub id: i64,
    pub channel_type: String,
    pub sender_id: String,
    pub agent_id: String,
    pub user_id: String,
    /// HASN identity bridged to this external channel binding.
    pub hasn_id: Option<String>,
    pub created_at: Option<String>,
}

/// Agent record.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AgentRecord {
    pub agent_id: String,
    pub template: String,
    pub star_name: Option<String>,
    pub hasn_id: Option<String>,
}

/// Channel binding record.
#[derive(Debug, Clone)]
pub struct ChannelRecord {
    pub id: i64,
    pub user_id: String,
    pub agent_id: String,
    pub channel_type: String,
    pub peer_id: String,
    pub peer_name: Option<String>,
    pub bound_at: Option<String>,
}

/// Filter criteria for listing users.
#[derive(Debug, Default)]
pub struct UserFilter {
    pub status: Option<String>,
    pub template: Option<String>,
    pub plan: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Database statistics.
#[derive(Debug, Clone, Default)]
pub struct DbStats {
    pub total_users: u64,
    pub active_users: u64,
    pub disabled_users: u64,
    pub total_channels: u64,
    pub templates: Vec<(String, u64)>,
    pub plans: Vec<(String, u64)>,
}

/// Database handle for tenant lookups.
/// Uses a single rusqlite::Connection behind a tokio Mutex for async safety.
#[derive(Clone)]
pub struct TenantDb {
    #[allow(dead_code)]
    db_path: PathBuf,
    conn: Arc<Mutex<rusqlite::Connection>>,
}

impl TenantDb {
    /// Open (or create) the SQLite database at the given path.
    pub fn open(db_path: &Path) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = rusqlite::Connection::open(db_path).context("Failed to open tenant database")?;

        // WAL mode for better concurrent read performance
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")?;

        let db = Self {
            db_path: db_path.to_path_buf(),
            conn: Arc::new(Mutex::new(conn)),
        };
        db.ensure_schema_sync()?;
        Ok(db)
    }

    /// Create tables if they don't exist.
    /// Compatible with existing HuanXing schema where users.user_id is PRIMARY KEY.
    fn ensure_schema_sync(&self) -> Result<()> {
        // Use try_lock since we're in a sync context during initialization
        let conn = self
            .conn
            .try_lock()
            .map_err(|_| anyhow::anyhow!("DB lock failed"))?;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS users (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id       TEXT NOT NULL UNIQUE,
                phone         TEXT UNIQUE,
                hasn_id       TEXT UNIQUE,
                nickname      TEXT,
                tenant_dir    TEXT,
                status        TEXT DEFAULT 'active',
                plan          TEXT DEFAULT 'free',
                plan_expires  TEXT,
                access_token  TEXT,
                llm_token     TEXT,
                gateway_token TEXT,
                token_expires TEXT,
                server_id     TEXT,
                created_at    DATETIME DEFAULT (datetime('now')),
                updated_at    DATETIME DEFAULT (datetime('now')),
                last_active   TEXT
            );
            CREATE TABLE IF NOT EXISTS agents (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                agent_id      TEXT NOT NULL UNIQUE,
                user_id       TEXT NOT NULL,
                template      TEXT NOT NULL DEFAULT 'finance',
                star_name     TEXT,
                hasn_id       TEXT UNIQUE,
                status        TEXT DEFAULT 'active',
                created_at    DATETIME DEFAULT (datetime('now')),
                updated_at    DATETIME DEFAULT (datetime('now')),
                FOREIGN KEY (user_id) REFERENCES users(user_id)
            );
            CREATE TABLE IF NOT EXISTS routing (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                channel_type  TEXT NOT NULL,
                sender_id     TEXT NOT NULL,
                agent_id      TEXT NOT NULL,
                user_id       TEXT NOT NULL,
                hasn_id       TEXT,
                created_at    DATETIME DEFAULT (datetime('now')),
                UNIQUE(channel_type, sender_id),
                FOREIGN KEY (agent_id) REFERENCES agents(agent_id),
                FOREIGN KEY (user_id) REFERENCES users(user_id)
            );
            CREATE TABLE IF NOT EXISTS verified_credentials (
                phone         TEXT PRIMARY KEY,
                user_id       TEXT NOT NULL,
                access_token  TEXT NOT NULL,
                refresh_token TEXT,
                llm_token     TEXT,
                gateway_token TEXT,
                is_new_user   INTEGER DEFAULT 0,
                created_at    TEXT DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_users_phone ON users(phone);
            CREATE INDEX IF NOT EXISTS idx_agents_user ON agents(user_id);
            CREATE INDEX IF NOT EXISTS idx_routing_lookup
                ON routing(channel_type, sender_id);
            ",
        )?;

        // Migrate existing users table: add new columns if missing
        let columns: Vec<String> = conn
            .prepare("PRAGMA table_info(users)")?
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        for (col, default) in [
            ("access_token", "NULL"),
            ("llm_token", "NULL"),
            ("gateway_token", "NULL"),
            ("token_expires", "NULL"),
            ("server_id", "NULL"),
            ("tenant_dir", "NULL"),
            ("hasn_id", "NULL"),
        ] {
            if !columns.iter().any(|c| c == col) {
                conn.execute_batch(&format!(
                    "ALTER TABLE users ADD COLUMN {col} TEXT DEFAULT {default};"
                ))?;
                tracing::info!("Migrated users table: added column {col}");
            }
        }

        // Migrate existing agents table: add hasn_id if missing
        let agents_exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='agents')",
                [],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if agents_exists {
            let agent_columns: Vec<String> = conn
                .prepare("PRAGMA table_info(agents)")?
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<std::result::Result<Vec<_>, _>>()?;

            if !agent_columns.iter().any(|c| c == "hasn_id") {
                conn.execute_batch(
                    "ALTER TABLE agents ADD COLUMN hasn_id TEXT DEFAULT NULL;
                     CREATE UNIQUE INDEX IF NOT EXISTS idx_agents_hasn_id ON agents(hasn_id) WHERE hasn_id IS NOT NULL;"
                )?;
                tracing::info!("Migrated agents table: added column hasn_id");
            }
        }

        // Migrate routing table: add hasn_id if missing
        let routing_exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='routing')",
                [],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if routing_exists {
            let routing_columns: Vec<String> = conn
                .prepare("PRAGMA table_info(routing)")?
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<std::result::Result<Vec<_>, _>>()?;

            if !routing_columns.iter().any(|c| c == "hasn_id") {
                conn.execute_batch(
                    "ALTER TABLE routing ADD COLUMN hasn_id TEXT DEFAULT NULL;"
                )?;
                tracing::info!("Migrated routing table: added column hasn_id");
            }
        }

        Ok(())
    }

    // ── Core lookups (existing) ───────────────────────

    /// Find a tenant by channel type and peer ID.
    /// This is the hot path — called on every inbound message (on cache miss).
    pub async fn find_by_channel(
        &self,
        channel_type: &str,
        sender_id: &str,
    ) -> Result<Option<TenantRecord>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare_cached(
            "SELECT u.user_id, a.agent_id, u.nickname, u.phone, a.template,
                    u.plan, u.status, a.star_name, u.tenant_dir,
                    u.plan_expires, u.created_at, u.last_active,
                    u.access_token, u.llm_token, u.gateway_token, NULL as token_expires, u.server_id,
                    u.hasn_id
             FROM routing r
             JOIN users u ON r.user_id = u.user_id
             JOIN agents a ON r.agent_id = a.agent_id
             WHERE r.channel_type = ?1 AND r.sender_id = ?2
               AND u.status = 'active'
             LIMIT 1",
        )?;
        match stmt.query_row(rusqlite::params![channel_type, sender_id], |row| {
            Ok(Self::row_to_record(row))
        }) {
            Ok(record) => Ok(Some(record)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Save a full user record including tokens (used by register flow).
    pub async fn save_user_full(
        &self,
        user_id: &str,
        phone: &str,
        agent_id: &str,
        nickname: Option<&str>,
        template: &str,
        star_name: Option<&str>,
        _workspace: Option<&str>, // Kept for API compatibility but ignored
        tenant_dir: Option<&str>,
        hasn_id: Option<&str>,
        access_token: Option<&str>,
        llm_token: Option<&str>,
        gateway_token: Option<&str>,
        node_id: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT OR REPLACE INTO users (
                user_id, phone, nickname, tenant_dir, hasn_id,
                access_token, llm_token, gateway_token, server_id
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                user_id,
                phone,
                nickname,
                tenant_dir,
                hasn_id,
                access_token,
                llm_token,
                gateway_token,
                node_id,
            ],
        )?;

        conn.execute(
            "INSERT OR REPLACE INTO agents (agent_id, user_id, template, star_name)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![agent_id, user_id, template, star_name],
        )?;

        Ok(())
    }

    pub async fn save_verified_credentials(&self, creds: &VerifiedCredentials) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT OR REPLACE INTO verified_credentials
             (phone, user_id, access_token, refresh_token, llm_token, gateway_token, is_new_user)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                creds.phone,
                creds.user_id,
                creds.access_token,
                creds.refresh_token,
                creds.llm_token,
                creds.gateway_token,
                creds.is_new_user as i32,
            ],
        )?;
        tracing::info!(phone = %creds.phone, user_id = %creds.user_id, "Verified credentials saved");
        Ok(())
    }

    /// Consume (read + delete) verified credentials for a phone number.
    /// Returns None if no credentials found.
    pub async fn consume_verified_credentials(
        &self,
        phone: &str,
    ) -> Result<Option<VerifiedCredentials>> {
        let conn = self.conn.lock().await;
        let result = conn.query_row(
            "SELECT phone, user_id, access_token, refresh_token, llm_token, gateway_token, is_new_user
             FROM verified_credentials WHERE phone = ?1",
            rusqlite::params![phone],
            |row| {
                Ok(VerifiedCredentials {
                    phone: row.get(0)?,
                    user_id: row.get(1)?,
                    access_token: row.get(2)?,
                    refresh_token: row.get(3)?,
                    llm_token: row.get(4)?,
                    gateway_token: row.get(5)?,
                    is_new_user: row.get::<_, i32>(6)? != 0,
                })
            },
        );

        match result {
            Ok(creds) => {
                // Delete after reading (consume)
                conn.execute(
                    "DELETE FROM verified_credentials WHERE phone = ?1",
                    rusqlite::params![phone],
                )?;
                tracing::info!(phone, "Verified credentials consumed");
                Ok(Some(creds))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Add a routing entry.
    pub async fn add_routing(
        &self,
        agent_id: &str,
        channel_type: &str,
        sender_id: &str,
    ) -> Result<()> {
        let conn = self.conn.lock().await;

        // Find user_id for this agent
        let user_id: String = conn.query_row(
            "SELECT user_id FROM agents WHERE agent_id = ?1",
            rusqlite::params![agent_id],
            |row| row.get(0),
        )?;

        conn.execute(
            "INSERT OR REPLACE INTO routing (channel_type, sender_id, agent_id, user_id)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![channel_type, sender_id, agent_id, user_id],
        )?;
        Ok(())
    }

    // ── Phase 1: Extended lookups ─────────────────────

    /// Find a user by phone number.
    pub async fn find_by_phone(&self, phone: &str) -> Result<Option<TenantRecord>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare_cached(
            "SELECT u.user_id, a.agent_id, u.nickname, u.phone, a.template,
                    u.plan, u.status, a.star_name, u.tenant_dir,
                    u.plan_expires, u.created_at, u.last_active,
                    u.access_token, u.llm_token, u.gateway_token, NULL as token_expires, u.server_id,
                    u.hasn_id
             FROM users u
             LEFT JOIN agents a ON u.user_id = a.user_id
             WHERE u.phone = ?1 LIMIT 1",
        )?;
        match stmt.query_row(rusqlite::params![phone], |row| Ok(Self::row_to_record(row))) {
            Ok(record) => Ok(Some(record)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn find_by_agent_id(&self, agent_id: &str) -> Result<Option<TenantRecord>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare_cached(
            "SELECT u.user_id, a.agent_id, u.nickname, u.phone, a.template,
                    u.plan, u.status, a.star_name, u.tenant_dir,
                    u.plan_expires, u.created_at, u.last_active,
                    u.access_token, u.llm_token, u.gateway_token, NULL as token_expires, u.server_id,
                    u.hasn_id
             FROM agents a
             JOIN users u ON a.user_id = u.user_id
             WHERE a.agent_id = ?1 LIMIT 1",
        )?;
        match stmt.query_row(rusqlite::params![agent_id], |row| {
            Ok(Self::row_to_record(row))
        }) {
            Ok(record) => Ok(Some(record)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn find_by_hasn_id(&self, hasn_id: &str) -> Result<Option<TenantRecord>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare_cached(
            "SELECT u.user_id, a.agent_id, u.nickname, u.phone, a.template,
                    u.plan, u.status, a.star_name, u.tenant_dir,
                    u.plan_expires, u.created_at, u.last_active,
                    u.access_token, u.llm_token, u.gateway_token, NULL as token_expires, u.server_id,
                    u.hasn_id
             FROM agents a
             JOIN users u ON a.user_id = u.user_id
             WHERE a.hasn_id = ?1 AND u.status = 'active' LIMIT 1",
        )?;
        match stmt.query_row(rusqlite::params![hasn_id], |row| {
            Ok(Self::row_to_record(row))
        }) {
            Ok(record) => Ok(Some(record)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Find a user by user_id.
    pub async fn get_user(&self, user_id: &str) -> Result<Option<TenantRecord>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare_cached(
            "SELECT u.user_id, a.agent_id, u.nickname, u.phone, a.template,
                    u.plan, u.status, a.star_name, u.tenant_dir,
                    u.plan_expires, u.created_at, u.last_active,
                    u.access_token, u.llm_token, u.gateway_token, NULL as token_expires, u.server_id,
                    u.hasn_id
             FROM users u
             LEFT JOIN agents a ON u.user_id = a.user_id
             WHERE u.user_id = ?1 LIMIT 1",
        )?;
        match stmt.query_row(rusqlite::params![user_id], |row| {
            Ok(Self::row_to_record(row))
        }) {
            Ok(record) => Ok(Some(record)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn get_channels(&self, user_id: &str) -> Result<Vec<ChannelRecord>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare_cached(
            "SELECT r.id, r.user_id, r.agent_id, r.channel_type, r.sender_id as peer_id, r.sender_id as peer_name, r.created_at as bound_at
             FROM routing r WHERE r.user_id = ?1 ORDER BY r.created_at",
        )?;
        let rows = stmt
            .query_map(rusqlite::params![user_id], |row| {
                Ok(ChannelRecord {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    agent_id: row.get(2)?,
                    channel_type: row.get(3)?,
                    peer_id: row.get(4)?,
                    peer_name: row.get(5)?,
                    bound_at: row.get(6)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub async fn get_agent_channels(&self, agent_id: &str) -> Result<Vec<ChannelRecord>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare_cached(
            "SELECT r.id, r.user_id, r.agent_id, r.channel_type, r.sender_id as peer_id, r.sender_id as peer_name, r.created_at as bound_at
             FROM routing r WHERE r.agent_id = ?1 ORDER BY r.created_at",
        )?;
        let rows = stmt
            .query_map(rusqlite::params![agent_id], |row| {
                Ok(ChannelRecord {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    agent_id: row.get(2)?,
                    channel_type: row.get(3)?,
                    peer_id: row.get(4)?,
                    peer_name: row.get(5)?,
                    bound_at: row.get(6)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Bind a new channel to an existing user's default agent.
    pub async fn bind_channel_default(
        &self,
        user_id: &str,
        channel_type: &str,
        peer_id: &str,
    ) -> Result<()> {
        let conn = self.conn.lock().await;

        // Find first agent for this user
        let agent_id: String = match conn.query_row(
            "SELECT agent_id FROM agents WHERE user_id = ?1 LIMIT 1",
            rusqlite::params![user_id],
            |row| row.get(0),
        ) {
            Ok(id) => id,
            Err(_) => return Err(anyhow::anyhow!("No agent found for user")),
        };

        conn.execute(
            "INSERT OR REPLACE INTO routing (channel_type, sender_id, agent_id, user_id)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![channel_type, peer_id, agent_id, user_id],
        )?;
        tracing::info!(
            user_id,
            channel_type,
            peer_id,
            "Channel bound into routing (default)"
        );
        Ok(())
    }

    /// Bind a new channel to a specific agent.
    pub async fn bind_channel_to_agent(
        &self,
        user_id: &str,
        channel_type: &str,
        peer_id: &str,
        agent_id: &str,
    ) -> Result<()> {
        let conn = self.conn.lock().await;

        conn.execute(
            "INSERT OR REPLACE INTO routing (channel_type, sender_id, agent_id, user_id)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![channel_type, peer_id, agent_id, user_id],
        )?;
        tracing::info!(
            user_id,
            agent_id,
            channel_type,
            peer_id,
            "Channel bound to specific agent"
        );
        Ok(())
    }

    pub async fn get_user_agents(&self, user_id: &str) -> Result<Vec<AgentRecord>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare_cached(
            "SELECT agent_id, template, star_name, hasn_id FROM agents WHERE user_id = ?1 ORDER BY created_at"
        )?;
        let rows = stmt
            .query_map(rusqlite::params![user_id], |row| {
                Ok(AgentRecord {
                    agent_id: row.get(0)?,
                    template: row.get(1).unwrap_or_default(),
                    star_name: row.get(2).unwrap_or(None),
                    hasn_id: row.get(3).unwrap_or(None),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub async fn update_agent_hasn_id(&self, agent_id: &str, hasn_id: &str) -> Result<bool> {
        let conn = self.conn.lock().await;
        let rows = conn.execute(
            "UPDATE agents
             SET hasn_id = ?1, updated_at = datetime('now')
             WHERE agent_id = ?2",
            rusqlite::params![hasn_id, agent_id],
        )?;
        Ok(rows > 0)
    }

    /// Update the HASN Human ID for a user (called after HASN identity registration).
    pub async fn update_user_hasn_id(&self, user_id: &str, hasn_id: &str) -> Result<bool> {
        let conn = self.conn.lock().await;
        let rows = conn.execute(
            "UPDATE users SET hasn_id = ?1, updated_at = datetime('now') WHERE user_id = ?2",
            rusqlite::params![hasn_id, user_id],
        )?;
        Ok(rows > 0)
    }

    pub async fn update_user(
        &self,
        user_id: &str,
        nickname: Option<&str>,
        star_name: Option<&str>,
        plan: Option<&str>,
        plan_expires: Option<&str>,
        status: Option<&str>,
    ) -> Result<bool> {
        let conn = self.conn.lock().await;

        // Update user table
        let mut user_sets = Vec::new();
        let mut user_params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(v) = nickname {
            user_sets.push("nickname = ?");
            user_params.push(Box::new(v.to_string()));
        }
        if let Some(v) = plan {
            user_sets.push("plan = ?");
            user_params.push(Box::new(v.to_string()));
        }
        if let Some(v) = plan_expires {
            user_sets.push("plan_expires = ?");
            user_params.push(Box::new(v.to_string()));
        }
        if let Some(v) = status {
            user_sets.push("status = ?");
            user_params.push(Box::new(v.to_string()));
        }

        let mut did_update = false;

        if !user_sets.is_empty() {
            user_params.push(Box::new(user_id.to_string()));
            let user_sql = format!(
                "UPDATE users SET {} WHERE user_id = ?",
                user_sets.join(", ")
            );
            let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                user_params.iter().map(|p| p.as_ref()).collect();
            let rows = conn.execute(&user_sql, param_refs.as_slice())?;
            if rows > 0 {
                did_update = true;
            }
        }

        // Update agents table
        if let Some(v) = star_name {
            let rows = conn.execute(
                "UPDATE agents SET star_name = ? WHERE user_id = ?",
                rusqlite::params![v, user_id],
            )?;
            if rows > 0 {
                did_update = true;
            }
        }

        if did_update {
            tracing::info!(user_id, "User updated via update_user");
        }

        Ok(did_update)
    }

    /// List users with optional filters.
    pub async fn list_users(&self, filter: &UserFilter) -> Result<(Vec<TenantRecord>, u64)> {
        let conn = self.conn.lock().await;

        // Count total matching
        let mut count_sql = "SELECT COUNT(*) FROM users WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(ref s) = filter.status {
            count_sql.push_str(" AND status = ?");
            params.push(Box::new(s.clone()));
        }
        if let Some(ref t) = filter.template {
            count_sql.push_str(" AND template = ?");
            params.push(Box::new(t.clone()));
        }
        if let Some(ref p) = filter.plan {
            count_sql.push_str(" AND plan = ?");
            params.push(Box::new(p.clone()));
        }

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let total: u64 = conn.query_row(&count_sql, param_refs.as_slice(), |r| r.get(0))?;

        // Fetch page
        let limit = filter.limit.unwrap_or(50).min(500);
        let offset = filter.offset.unwrap_or(0);

        let mut data_sql = count_sql.replace(
            "SELECT COUNT(*) FROM users",
            "SELECT u.user_id, a.agent_id, u.nickname, u.phone, a.template,
                    u.plan, u.status, a.star_name, u.tenant_dir,
                    u.plan_expires, u.created_at, u.last_active,
                    u.access_token, u.llm_token, u.gateway_token, NULL as token_expires, u.server_id,
                    u.hasn_id
             FROM users u
             LEFT JOIN agents a ON u.user_id = a.user_id",
        );
        data_sql.push_str(&format!(
            " ORDER BY created_at DESC LIMIT {limit} OFFSET {offset}"
        ));

        // Rebuild params for data query
        let mut params2: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        if let Some(ref s) = filter.status {
            params2.push(Box::new(s.clone()));
        }
        if let Some(ref t) = filter.template {
            params2.push(Box::new(t.clone()));
        }
        if let Some(ref p) = filter.plan {
            params2.push(Box::new(p.clone()));
        }
        let param_refs2: Vec<&dyn rusqlite::types::ToSql> =
            params2.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&data_sql)?;
        let rows = stmt
            .query_map(param_refs2.as_slice(), |row| Ok(Self::row_to_record(row)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok((rows, total))
    }

    /// Get the default (first) tenant_dir from the database for local desktop mode API fallback.
    pub async fn get_first_tenant_dir(&self) -> Result<Option<String>> {
        let conn = self.conn.lock().await;
        match conn.query_row(
            "SELECT tenant_dir FROM users ORDER BY created_at ASC LIMIT 1",
            [],
            |row| row.get(0),
        ) {
            Ok(dir) => Ok(Some(dir)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get aggregate statistics.
    pub async fn get_stats(&self) -> Result<DbStats> {
        let conn = self.conn.lock().await;
        let total_users: u64 =
            conn.query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))?;
        let active_users: u64 = conn.query_row(
            "SELECT COUNT(*) FROM users WHERE status = 'active'",
            [],
            |row| row.get(0),
        )?;
        let disabled_users: u64 = conn.query_row(
            "SELECT COUNT(*) FROM users WHERE status != 'active'",
            [],
            |row| row.get(0),
        )?;
        let total_channels: u64 =
            conn.query_row("SELECT COUNT(*) FROM routing", [], |row| row.get(0))?;

        let mut stmt = conn.prepare("SELECT template, COUNT(*) FROM agents GROUP BY template")?;
        let templates = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;

        let mut stmt = conn.prepare("SELECT plan, COUNT(*) FROM users GROUP BY plan")?;
        let plans = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(DbStats {
            total_users,
            active_users,
            disabled_users,
            total_channels,
            templates,
            plans,
        })
    }

    pub async fn get_next_user_seq(&self) -> Result<u32> {
        let conn = self.conn.lock().await;
        let count: u32 = conn.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))?;
        Ok(count + 1)
    }

    /// Update last_active timestamp for a user.
    pub async fn touch_last_active(&self, user_id: &str) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "UPDATE users SET last_active = datetime('now') WHERE user_id = ?1",
            rusqlite::params![user_id],
        )?;
        Ok(())
    }

    // ── Internal helpers ──────────────────────────────

    fn row_to_record(row: &rusqlite::Row<'_>) -> TenantRecord {
        TenantRecord {
            user_id: row.get(0).unwrap_or_default(),
            agent_id: row.get(1).unwrap_or_default(),
            hasn_id: row.get(17).unwrap_or(None),
            nickname: row.get(2).unwrap_or(None),
            phone: row.get(3).unwrap_or(None),
            template: row.get(4).unwrap_or(None),
            plan: row.get(5).unwrap_or(None),
            status: row.get(6).unwrap_or_else(|_| "active".to_string()),
            star_name: row.get(7).unwrap_or(None),
            tenant_dir: row.get(8).unwrap_or(None),
            plan_expires: row.get(9).unwrap_or(None),
            created_at: row.get(10).unwrap_or(None),
            last_active: row.get(11).unwrap_or(None),
            access_token: row.get(12).unwrap_or(None),
            llm_token: row.get(13).unwrap_or(None),
            gateway_token: row.get(14).unwrap_or(None),
            token_expires: row.get(15).unwrap_or(None),
            node_id: row.get(16).unwrap_or(None),
        }
    }

    /// Same as row_to_record but only for the basic 12-column queries.
    fn row_to_record_basic(row: &rusqlite::Row<'_>) -> TenantRecord {
        TenantRecord {
            user_id: row.get(0).unwrap_or_default(),
            agent_id: row.get(1).unwrap_or_default(),
            hasn_id: None,
            nickname: row.get(2).unwrap_or(None),
            phone: row.get(3).unwrap_or(None),
            template: row.get(4).unwrap_or(None),
            plan: row.get(5).unwrap_or(None),
            status: row.get(6).unwrap_or_else(|_| "active".to_string()),
            star_name: row.get(7).unwrap_or(None),
            tenant_dir: None,
            plan_expires: row.get(8).unwrap_or(None),
            created_at: row.get(9).unwrap_or(None),
            last_active: row.get(10).unwrap_or(None),
            access_token: None,
            llm_token: None,
            gateway_token: None,
            token_expires: None,
            node_id: None,
        }
    }
}
