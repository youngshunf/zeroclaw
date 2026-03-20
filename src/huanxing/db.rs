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
    /// Workspace path.
    pub workspace: Option<String>,
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
    /// Server ID.
    pub server_id: Option<String>,
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
pub struct ChannelRecord {
    pub id: i64,
    pub user_id: String,
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
                user_id       TEXT PRIMARY KEY,
                phone         TEXT UNIQUE,
                nickname      TEXT,
                star_name     TEXT,
                template      TEXT NOT NULL DEFAULT 'finance',
                agent_id      TEXT UNIQUE,
                workspace     TEXT,
                status        TEXT DEFAULT 'active',
                plan          TEXT DEFAULT 'star_dust',
                plan_expires  TEXT,
                access_token  TEXT,
                llm_token     TEXT,
                gateway_token TEXT,
                token_expires TEXT,
                server_id     TEXT,
                created_at    TEXT DEFAULT (datetime('now')),
                last_active   TEXT
            );
            CREATE TABLE IF NOT EXISTS channels (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id       TEXT NOT NULL,
                channel_type  TEXT NOT NULL,
                peer_id       TEXT NOT NULL,
                peer_name     TEXT,
                bound_at      TEXT DEFAULT (datetime('now')),
                FOREIGN KEY (user_id) REFERENCES users(user_id),
                UNIQUE(channel_type, peer_id)
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
            CREATE TABLE IF NOT EXISTS routing (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                agent_id      TEXT NOT NULL,
                channel_type  TEXT NOT NULL,
                peer_id       TEXT NOT NULL,
                created_at    TEXT DEFAULT (datetime('now')),
                UNIQUE(channel_type, peer_id)
            );
            CREATE INDEX IF NOT EXISTS idx_channels_lookup
                ON channels(channel_type, peer_id);
            CREATE INDEX IF NOT EXISTS idx_users_phone ON users(phone);
            CREATE INDEX IF NOT EXISTS idx_users_agent ON users(agent_id);
            CREATE INDEX IF NOT EXISTS idx_routing_lookup
                ON routing(channel_type, peer_id);
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
        ] {
            if !columns.iter().any(|c| c == col) {
                conn.execute_batch(&format!(
                    "ALTER TABLE users ADD COLUMN {col} TEXT DEFAULT {default};"
                ))?;
                tracing::info!("Migrated users table: added column {col}");
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
        peer_id: &str,
    ) -> Result<Option<TenantRecord>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare_cached(
            "SELECT u.user_id, u.agent_id, u.nickname, u.phone, u.template,
                    u.plan, u.status, u.star_name, u.workspace, u.plan_expires,
                    u.created_at, u.last_active,
                    u.access_token, u.llm_token, u.gateway_token, u.token_expires, u.server_id
             FROM users u
             JOIN channels c ON u.user_id = c.user_id
             WHERE c.channel_type = ?1 AND c.peer_id = ?2 AND u.status = 'active'
             LIMIT 1",
        )?;

        let result = stmt.query_row(rusqlite::params![channel_type, peer_id], |row| {
            Ok(Self::row_to_record(row))
        });

        match result {
            Ok(record) => Ok(Some(record)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Register a new user with channel binding (includes tokens).
    pub async fn register_user(
        &self,
        user_id: &str,
        phone: &str,
        agent_id: &str,
        nickname: Option<&str>,
        template: &str,
        star_name: Option<&str>,
        channel_type: &str,
        peer_id: &str,
        workspace: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().await;

        conn.execute(
            "INSERT INTO users (user_id, phone, agent_id, nickname, template, star_name, workspace, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'active')",
            rusqlite::params![user_id, phone, agent_id, nickname, template, star_name, workspace],
        )?;

        conn.execute(
            "INSERT INTO channels (user_id, channel_type, peer_id)
             VALUES (?1, ?2, ?3)",
            rusqlite::params![user_id, channel_type, peer_id],
        )?;

        tracing::info!(
            user_id,
            phone,
            agent_id,
            channel_type,
            peer_id,
            "User registered"
        );

        Ok(())
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
        workspace: Option<&str>,
        access_token: Option<&str>,
        llm_token: Option<&str>,
        gateway_token: Option<&str>,
        server_id: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().await;

        conn.execute(
            "INSERT OR REPLACE INTO users
             (user_id, phone, agent_id, nickname, template, star_name, workspace, status,
              access_token, llm_token, gateway_token, server_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'active', ?8, ?9, ?10, ?11)",
            rusqlite::params![
                user_id,
                phone,
                agent_id,
                nickname,
                template,
                star_name,
                workspace,
                access_token,
                llm_token,
                gateway_token,
                server_id
            ],
        )?;

        tracing::info!(user_id, phone, agent_id, "User saved with tokens");

        Ok(())
    }

    /// Save verified credentials after successful phone-login.
    /// These are consumed by hx_register_user.
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

    /// Add a routing entry (agent_id → channel+peer_id).
    pub async fn add_routing(
        &self,
        agent_id: &str,
        channel_type: &str,
        peer_id: &str,
    ) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT OR REPLACE INTO routing (agent_id, channel_type, peer_id)
             VALUES (?1, ?2, ?3)",
            rusqlite::params![agent_id, channel_type, peer_id],
        )?;
        Ok(())
    }

    // ── Phase 1: Extended lookups ─────────────────────

    /// Find a user by phone number.
    pub async fn find_by_phone(&self, phone: &str) -> Result<Option<TenantRecord>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare_cached(
            "SELECT user_id, agent_id, nickname, phone, template,
                    plan, status, star_name, workspace, plan_expires,
                    created_at, last_active,
                    access_token, llm_token, gateway_token, token_expires, server_id
             FROM users WHERE phone = ?1 LIMIT 1",
        )?;
        match stmt.query_row(rusqlite::params![phone], |row| Ok(Self::row_to_record(row))) {
            Ok(record) => Ok(Some(record)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Find a user by agent_id.
    pub async fn find_by_agent_id(&self, agent_id: &str) -> Result<Option<TenantRecord>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare_cached(
            "SELECT user_id, agent_id, nickname, phone, template,
                    plan, status, star_name, workspace, plan_expires,
                    created_at, last_active,
                    access_token, llm_token, gateway_token, token_expires, server_id
             FROM users WHERE agent_id = ?1 LIMIT 1",
        )?;
        match stmt.query_row(rusqlite::params![agent_id], |row| {
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
            "SELECT user_id, agent_id, nickname, phone, template,
                    plan, status, star_name, workspace, plan_expires,
                    created_at, last_active,
                    access_token, llm_token, gateway_token, token_expires, server_id
             FROM users WHERE user_id = ?1 LIMIT 1",
        )?;
        match stmt.query_row(rusqlite::params![user_id], |row| {
            Ok(Self::row_to_record(row))
        }) {
            Ok(record) => Ok(Some(record)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get all channel bindings for a user.
    pub async fn get_channels(&self, user_id: &str) -> Result<Vec<ChannelRecord>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare_cached(
            "SELECT id, user_id, channel_type, peer_id, peer_name, bound_at
             FROM channels WHERE user_id = ?1 ORDER BY bound_at",
        )?;
        let rows = stmt
            .query_map(rusqlite::params![user_id], |row| {
                Ok(ChannelRecord {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    channel_type: row.get(2)?,
                    peer_id: row.get(3)?,
                    peer_name: row.get(4)?,
                    bound_at: row.get(5)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Bind a new channel to an existing user.
    pub async fn bind_channel(
        &self,
        user_id: &str,
        channel_type: &str,
        peer_id: &str,
        peer_name: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT OR REPLACE INTO channels (user_id, channel_type, peer_id, peer_name)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![user_id, channel_type, peer_id, peer_name],
        )?;
        tracing::info!(user_id, channel_type, peer_id, "Channel bound");
        Ok(())
    }

    /// Update user fields. Only non-None values are updated.
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
        let mut sets = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(v) = nickname {
            sets.push("nickname = ?");
            params.push(Box::new(v.to_string()));
        }
        if let Some(v) = star_name {
            sets.push("star_name = ?");
            params.push(Box::new(v.to_string()));
        }
        if let Some(v) = plan {
            sets.push("plan = ?");
            params.push(Box::new(v.to_string()));
        }
        if let Some(v) = plan_expires {
            sets.push("plan_expires = ?");
            params.push(Box::new(v.to_string()));
        }
        if let Some(v) = status {
            sets.push("status = ?");
            params.push(Box::new(v.to_string()));
        }

        if sets.is_empty() {
            return Ok(false);
        }

        params.push(Box::new(user_id.to_string()));
        let sql = format!("UPDATE users SET {} WHERE user_id = ?", sets.join(", "));
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let rows = conn.execute(&sql, param_refs.as_slice())?;
        Ok(rows > 0)
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
            "SELECT user_id, agent_id, nickname, phone, template,
                    plan, status, star_name, workspace, plan_expires,
                    created_at, last_active,
                    access_token, llm_token, gateway_token, token_expires, server_id FROM users",
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

    /// Get aggregate statistics.
    pub async fn get_stats(&self) -> Result<DbStats> {
        let conn = self.conn.lock().await;

        let total_users: u64 = conn.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))?;
        let active_users: u64 = conn.query_row(
            "SELECT COUNT(*) FROM users WHERE status = 'active'",
            [],
            |r| r.get(0),
        )?;
        let disabled_users = total_users - active_users;
        let total_channels: u64 =
            conn.query_row("SELECT COUNT(*) FROM channels", [], |r| r.get(0))?;

        let mut templates = Vec::new();
        {
            let mut stmt =
                conn.prepare("SELECT template, COUNT(*) FROM users GROUP BY template")?;
            let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, u64>(1)?)))?;
            for row in rows {
                templates.push(row?);
            }
        }

        let mut plans = Vec::new();
        {
            let mut stmt = conn.prepare("SELECT plan, COUNT(*) FROM users GROUP BY plan")?;
            let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, u64>(1)?)))?;
            for row in rows {
                plans.push(row?);
            }
        }

        Ok(DbStats {
            total_users,
            active_users,
            disabled_users,
            total_channels,
            templates,
            plans,
        })
    }

    /// Get the next user sequence number (for agent_id generation).
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
            nickname: row.get(2).unwrap_or(None),
            phone: row.get(3).unwrap_or(None),
            template: row.get(4).unwrap_or(None),
            plan: row.get(5).unwrap_or(None),
            status: row.get(6).unwrap_or_else(|_| "active".to_string()),
            star_name: row.get(7).unwrap_or(None),
            workspace: row.get(8).unwrap_or(None),
            plan_expires: row.get(9).unwrap_or(None),
            created_at: row.get(10).unwrap_or(None),
            last_active: row.get(11).unwrap_or(None),
            // These may not exist in old queries that only select 12 columns
            access_token: row.get(12).unwrap_or(None),
            llm_token: row.get(13).unwrap_or(None),
            gateway_token: row.get(14).unwrap_or(None),
            token_expires: row.get(15).unwrap_or(None),
            server_id: row.get(16).unwrap_or(None),
        }
    }

    /// Same as row_to_record but only for the basic 12-column queries.
    fn row_to_record_basic(row: &rusqlite::Row<'_>) -> TenantRecord {
        TenantRecord {
            user_id: row.get(0).unwrap_or_default(),
            agent_id: row.get(1).unwrap_or_default(),
            nickname: row.get(2).unwrap_or(None),
            phone: row.get(3).unwrap_or(None),
            template: row.get(4).unwrap_or(None),
            plan: row.get(5).unwrap_or(None),
            status: row.get(6).unwrap_or_else(|_| "active".to_string()),
            star_name: row.get(7).unwrap_or(None),
            workspace: row.get(8).unwrap_or(None),
            plan_expires: row.get(9).unwrap_or(None),
            created_at: row.get(10).unwrap_or(None),
            last_active: row.get(11).unwrap_or(None),
            access_token: None,
            llm_token: None,
            gateway_token: None,
            token_expires: None,
            server_id: None,
        }
    }
}
