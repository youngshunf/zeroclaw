//! 桌面端 Session REST API。
//!
//! 为桌面端前端提供完整的 Session CRUD，每个 Agent 的会话独立存储在
//! `{config_dir}/users/{tenant}/agents/{agent_name}/workspace/sessions/sessions.db`。
//!
//! # 端点
//!
//! ```text
//! GET    /api/sessions                        → 列出所有会话（可按 agent_id 过滤）
//! POST   /api/sessions                        → 创建新会话
//! GET    /api/sessions/{id}                   → 获取会话详情（带分页消息）
//! PUT    /api/sessions/{id}                   → 重命名会话
//! DELETE /api/sessions/{id}                   → 删除会话
//! DELETE /api/sessions/{id}/messages          → 清空会话消息
//! POST   /api/sessions/{id}/generate-title    → LLM 自动生成标题
//! ```

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
};
use chrono::Utc;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::path::Path as FsPath;

use crate::gateway::AppState;

// ── 数据结构 ──────────────────────────────────────────────

/// 会话摘要（列表用）
#[derive(Debug, Serialize)]
pub struct SessionInfo {
    pub id: String,
    pub title: String,
    pub agent_id: String,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: u32,
}

/// 单条消息
#[derive(Debug, Serialize)]
pub struct SessionMessage {
    pub id: i64,
    pub role: String,
    pub content: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress_lines: Option<Vec<String>>,
}

/// 会话详情（含分页消息）
#[derive(Debug, Serialize)]
pub struct SessionDetail {
    pub id: String,
    pub title: String,
    pub agent_id: String,
    pub created_at: String,
    pub updated_at: String,
    pub messages: Vec<SessionMessage>,
    pub has_more: bool,
    pub oldest_id: Option<i64>,
    pub total_count: u32,
}

/// POST /api/sessions 请求体
#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    pub title: Option<String>,
    pub agent_id: Option<String>,
}

/// PUT /api/sessions/{id} 请求体
#[derive(Debug, Deserialize)]
pub struct RenameSessionRequest {
    pub title: String,
}

/// GET /api/sessions 查询参数
#[derive(Debug, Deserialize)]
pub struct ListSessionsQuery {
    pub agent_id: Option<String>,
}

/// GET /api/sessions/{id} 查询参数
#[derive(Debug, Deserialize)]
pub struct GetSessionQuery {
    pub agent_id: Option<String>,
    pub limit: Option<i64>,
    pub before: Option<i64>,
}

// ── 路由注册 ──────────────────────────────────────────────

pub fn session_routes() -> Router<AppState> {
    Router::new()
        .route("/api/sessions", get(list_sessions).post(create_session))
        .route(
            "/api/sessions/{id}",
            get(get_session).put(rename_session).delete(delete_session),
        )
        .route("/api/sessions/{id}/messages", delete(clear_messages))
        .route("/api/sessions/{id}/generate-title", post(generate_title))
}

// ── Session DB 工具函数 ──────────────────────────────────

/// 打开（或创建）某 Agent 的 sessions.db，并确保 desktop_sessions 表存在。
///
/// 与 `SqliteSessionBackend` 共享同一个 db 文件，但使用独立的 `desktop_sessions` 表
/// 存储 title / created_at 等桌面端特有字段。
fn open_agent_sessions_db(agent_workspace: &FsPath) -> rusqlite::Result<Connection> {
    let sessions_dir = agent_workspace.join("sessions");
    std::fs::create_dir_all(&sessions_dir).ok();
    let db_path = sessions_dir.join("sessions.db");

    let conn = Connection::open(&db_path)?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;",
    )?;

    // 创建桌面端专用的会话元数据表（title + created_at）
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS desktop_sessions (
            session_id   TEXT PRIMARY KEY,
            agent_id     TEXT NOT NULL,
            title        TEXT NOT NULL DEFAULT '',
            created_at   TEXT NOT NULL,
            updated_at   TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_desktop_sessions_agent ON desktop_sessions(agent_id);",
    )?;

    Ok(conn)
}

fn resolve_agent_session_workspace(
    config: &crate::config::schema::Config,
    config_dir: &FsPath,
    tenant_dir: Option<&str>,
    agent_id: &str,
) -> std::path::PathBuf {
    config
        .huanxing
        .resolve_agent_workspace(config_dir, tenant_dir, agent_id)
}

/// 从 agent workspace 中读取所有会话（合并 desktop_sessions + session_metadata）
fn list_agent_sessions(agent_id: &str, agent_workspace: &FsPath) -> Vec<SessionInfo> {
    let conn = match open_agent_sessions_db(agent_workspace) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(agent_id, "打开 sessions.db 失败: {e}");
            return Vec::new();
        }
    };

    let mut stmt = match conn.prepare(
        "SELECT ds.session_id, ds.title, ds.created_at, ds.updated_at,
                COALESCE(sm.message_count, 0) as msg_count
         FROM desktop_sessions ds
         LEFT JOIN session_metadata sm ON sm.session_key = ds.session_id
         WHERE ds.agent_id = ?1
         ORDER BY ds.updated_at DESC",
    ) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(agent_id, "准备查询失败: {e}");
            return Vec::new();
        }
    };

    let rows = match stmt.query_map(params![agent_id], |row| {
        Ok(SessionInfo {
            id: row.get(0)?,
            title: row.get(1)?,
            agent_id: agent_id.to_string(),
            created_at: row.get(2)?,
            updated_at: row.get(3)?,
            message_count: {
                let v: i64 = row.get(4)?;
                #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
                let u = v as u32;
                u
            },
        })
    }) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    rows.filter_map(|r| r.ok()).collect()
}

// ── 处理函数 ──────────────────────────────────────────────

/// GET /api/sessions — 列出所有会话，可按 agent_id 过滤
async fn list_sessions(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Query(q): Query<ListSessionsQuery>,
) -> impl IntoResponse {
    let config = state.config.lock().clone();
    if !config.huanxing.enabled {
        return (StatusCode::OK, Json(serde_json::json!({ "sessions": [] }))).into_response();
    }
    let config_dir = config.config_path.parent().unwrap_or(&config.workspace_dir);

    let tenant_dir =
        crate::huanxing::api_agents::extract_tenant_dir(&headers, config_dir, &config.huanxing)
            .await;
    let agents_dir = config
        .huanxing
        .resolve_tenant_root(config_dir, tenant_dir.as_deref())
        .join("agents");

    let mut all_sessions: Vec<SessionInfo> = Vec::new();

    if let Some(ref agent_id) = q.agent_id {
        // 只查指定 agent
        let workspace =
            resolve_agent_session_workspace(&config, config_dir, tenant_dir.as_deref(), agent_id);
        if workspace.exists() {
            all_sessions = list_agent_sessions(agent_id, &workspace);
        }
    } else {
        // 扫描所有 agent 目录
        match tokio::fs::read_dir(&agents_dir).await {
            Ok(mut entries) => {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let path = entry.path();
                    if !path.is_dir() {
                        continue;
                    }
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with('.') {
                        continue;
                    }
                    let workspace = resolve_agent_session_workspace(
                        &config,
                        config_dir,
                        tenant_dir.as_deref(),
                        &name,
                    );
                    if !workspace.exists() {
                        continue;
                    }
                    let sessions =
                        tokio::task::block_in_place(|| list_agent_sessions(&name, &workspace));
                    all_sessions.extend(sessions);
                }
            }
            Err(e) => {
                tracing::warn!(
                    agents_dir = %agents_dir.display(),
                    "扫描 agents 目录失败: {e}"
                );
            }
        }
        // 按更新时间降序
        all_sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({ "sessions": all_sessions })),
    )
        .into_response()
}

/// POST /api/sessions — 创建新会话
async fn create_session(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<CreateSessionRequest>,
) -> impl IntoResponse {
    let config = state.config.lock().clone();
    if !config.huanxing.enabled {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error": "HuanXing 未启用"})),
        )
            .into_response();
    }
    let config_dir = config.config_path.parent().unwrap_or(&config.workspace_dir);

    let tenant_dir =
        crate::huanxing::api_agents::extract_tenant_dir(&headers, config_dir, &config.huanxing)
            .await;

    // agent_id 默认为 "default"（兼容单 agent 模式）
    let agent_id = req
        .agent_id
        .clone()
        .unwrap_or_else(|| "default".to_string());

    let workspace =
        resolve_agent_session_workspace(&config, config_dir, tenant_dir.as_deref(), &agent_id);
    if !workspace.exists() {
        // agent 目录不存在，尝试创建（兼容 legacy 单 agent）
        if agent_id == "default" {
            if let Err(e) = tokio::fs::create_dir_all(&workspace).await {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": format!("创建工作区失败: {e}")})),
                )
                    .into_response();
            }
        } else {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": format!("agent '{}' 不存在", agent_id)})),
            )
                .into_response();
        }
    }

    let session_id = uuid::Uuid::new_v4().to_string();
    let title = req.title.unwrap_or_else(|| "新会话".to_string());
    let now = Utc::now().to_rfc3339();
    let agent_id_clone = agent_id.clone();
    let session_id_clone = session_id.clone();
    let title_clone = title.clone();
    let now_clone = now.clone();

    let result = tokio::task::block_in_place(move || {
        let conn = open_agent_sessions_db(&workspace)?;
        conn.execute(
            "INSERT INTO desktop_sessions (session_id, agent_id, title, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                session_id_clone,
                agent_id_clone,
                title_clone,
                now_clone,
                now_clone
            ],
        )?;
        Ok::<(), rusqlite::Error>(())
    });

    match result {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "session_id": session_id,
                "title": title,
                "agent_id": agent_id,
                "created_at": now,
                "updated_at": now,
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("创建会话失败: {e}")})),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_agent_session_workspace;
    use crate::config::Config;
    use tempfile::tempdir;

    #[test]
    fn resolve_agent_session_workspace_uses_inner_workspace() {
        let temp = tempdir().unwrap();
        let config_dir = temp.path();
        let mut config = Config::default();
        config.huanxing.enabled = true;
        config.config_path = config_dir.join("config.toml");
        config.workspace_dir = config_dir.join("workspace");

        let workspace =
            resolve_agent_session_workspace(&config, config_dir, Some("001-tenant-a"), "default");

        assert_eq!(
            workspace,
            config_dir
                .join("users")
                .join("001-tenant-a")
                .join("agents")
                .join("default")
                .join("workspace")
        );
    }
}

/// GET /api/sessions/{id} — 获取会话详情（含分页消息）
async fn get_session(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Path(session_id): Path<String>,
    Query(q): Query<GetSessionQuery>,
) -> impl IntoResponse {
    let config = state.config.lock().clone();
    let config_dir = config.config_path.parent().unwrap_or(&config.workspace_dir);
    let tenant_dir =
        crate::huanxing::api_agents::extract_tenant_dir(&headers, config_dir, &config.huanxing)
            .await;

    let limit = q.limit.unwrap_or(50).min(200).max(1);

    // 找到该 session 所属的 agent workspace
    let agent_id_and_workspace = if let Some(ref aid) = q.agent_id {
        let ws = config
            .huanxing
            .resolve_agent_workspace(config_dir, tenant_dir.as_deref(), aid);
        if ws.exists() {
            Some((aid.clone(), ws))
        } else {
            None
        }
    } else {
        // 扫描所有 agent 查找 session
        find_session_owner(&config, config_dir, &session_id, tenant_dir.as_deref()).await
    };

    let Some((agent_id, workspace)) = agent_id_and_workspace else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "会话不存在"})),
        )
            .into_response();
    };

    let session_id_clone = session_id.clone();
    let before = q.before;

    let result: Result<SessionDetail, rusqlite::Error> = tokio::task::block_in_place(move || {
        let conn = open_agent_sessions_db(&workspace)?;

        // 查 desktop_sessions 元数据
        let meta: Option<(String, String, String)> = conn
            .query_row(
                "SELECT title, created_at, updated_at FROM desktop_sessions WHERE session_id = ?1",
                params![session_id_clone],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .ok();

        let (title, created_at, updated_at) = meta.unwrap_or_else(|| {
            let now = Utc::now().to_rfc3339();
            ("未命名会话".to_string(), now.clone(), now)
        });

        // 总消息数
        let total_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sessions WHERE session_key = ?1",
                params![session_id_clone],
                |row| row.get(0),
            )
            .unwrap_or(0);

        // 分页查询消息（倒序，before cursor）
        let msgs: Vec<SessionMessage> = if let Some(before_id) = before {
            let mut stmt = conn.prepare(
                "SELECT id, role, content, created_at, metadata FROM sessions
                 WHERE session_key = ?1 AND id < ?2
                 ORDER BY id DESC LIMIT ?3",
            )?;
            let rows = stmt.query_map(params![session_id_clone, before_id, limit], |row| {
                let metadata: Option<String> = row.get(4)?;
                let mut progress_lines = None;
                if let Some(meta_str) = metadata {
                    if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&meta_str) {
                        if let Some(pl) = meta.get("progress_lines") {
                            progress_lines = serde_json::from_value(pl.clone()).ok();
                        }
                    }
                }
                Ok(SessionMessage {
                    id: row.get(0)?,
                    role: row.get(1)?,
                    content: row.get(2)?,
                    timestamp: row.get(3)?,
                    progress_lines,
                })
            })?;
            rows.filter_map(|r| r.ok()).collect()
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, role, content, created_at, metadata FROM sessions
                 WHERE session_key = ?1
                 ORDER BY id DESC LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![session_id_clone, limit], |row| {
                let metadata: Option<String> = row.get(4)?;
                let mut progress_lines = None;
                if let Some(meta_str) = metadata {
                    if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&meta_str) {
                        if let Some(pl) = meta.get("progress_lines") {
                            progress_lines = serde_json::from_value(pl.clone()).ok();
                        }
                    }
                }
                Ok(SessionMessage {
                    id: row.get(0)?,
                    role: row.get(1)?,
                    content: row.get(2)?,
                    timestamp: row.get(3)?,
                    progress_lines,
                })
            })?;
            rows.filter_map(|r| r.ok()).collect()
        };

        // 反转为时间正序
        let mut msgs = msgs;
        msgs.reverse();

        let oldest_id = msgs.first().map(|m| m.id);
        let returned = msgs.len() as i64;
        let has_more = if let Some(before_id) = before {
            conn.query_row(
                "SELECT COUNT(*) FROM sessions WHERE session_key = ?1 AND id < ?2",
                params![session_id_clone, before_id - returned],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0)
                > 0
        } else {
            total_count > limit
        };

        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        Ok(SessionDetail {
            id: session_id_clone,
            title,
            agent_id: agent_id.clone(),
            created_at,
            updated_at,
            messages: msgs,
            has_more,
            oldest_id,
            total_count: total_count as u32,
        })
    });

    match result {
        Ok(detail) => (StatusCode::OK, Json(detail)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("查询失败: {e}")})),
        )
            .into_response(),
    }
}

/// PUT /api/sessions/{id} — 重命名会话
async fn rename_session(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Path(session_id): Path<String>,
    Json(req): Json<RenameSessionRequest>,
) -> impl IntoResponse {
    let config = state.config.lock().clone();
    let config_dir = config.config_path.parent().unwrap_or(&config.workspace_dir);
    let tenant_dir =
        crate::huanxing::api_agents::extract_tenant_dir(&headers, config_dir, &config.huanxing)
            .await;

    let Some((_agent_id, workspace)) =
        find_session_owner(&config, config_dir, &session_id, tenant_dir.as_deref()).await
    else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "会话不存在"})),
        )
            .into_response();
    };

    let now = Utc::now().to_rfc3339();
    let result = tokio::task::block_in_place(move || {
        let conn = open_agent_sessions_db(&workspace)?;
        conn.execute(
            "UPDATE desktop_sessions SET title = ?1, updated_at = ?2 WHERE session_id = ?3",
            params![req.title, now, session_id],
        )?;
        Ok::<(), rusqlite::Error>(())
    });

    match result {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"status": "ok"}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("重命名失败: {e}")})),
        )
            .into_response(),
    }
}

/// DELETE /api/sessions/{id} — 删除会话及所有消息
async fn delete_session(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    let config = state.config.lock().clone();
    let config_dir = config.config_path.parent().unwrap_or(&config.workspace_dir);

    let tenant_dir =
        crate::huanxing::api_agents::extract_tenant_dir(&headers, config_dir, &config.huanxing)
            .await;
    let Some((_agent_id, workspace)) =
        find_session_owner(&config, config_dir, &session_id, tenant_dir.as_deref()).await
    else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "会话不存在"})),
        )
            .into_response();
    };

    let result = tokio::task::block_in_place(move || {
        let conn = open_agent_sessions_db(&workspace)?;
        conn.execute(
            "DELETE FROM sessions WHERE session_key = ?1",
            params![session_id],
        )?;
        conn.execute(
            "DELETE FROM session_metadata WHERE session_key = ?1",
            params![session_id],
        )?;
        conn.execute(
            "DELETE FROM desktop_sessions WHERE session_id = ?1",
            params![session_id],
        )?;
        Ok::<(), rusqlite::Error>(())
    });

    match result {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"status": "ok"}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("删除失败: {e}")})),
        )
            .into_response(),
    }
}

/// DELETE /api/sessions/{id}/messages — 清空会话消息（保留元数据）
async fn clear_messages(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    let config = state.config.lock().clone();
    let config_dir = config.config_path.parent().unwrap_or(&config.workspace_dir);

    let tenant_dir =
        crate::huanxing::api_agents::extract_tenant_dir(&headers, config_dir, &config.huanxing)
            .await;
    let Some((_agent_id, workspace)) =
        find_session_owner(&config, config_dir, &session_id, tenant_dir.as_deref()).await
    else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "会话不存在"})),
        )
            .into_response();
    };

    let now = Utc::now().to_rfc3339();
    let result = tokio::task::block_in_place(move || {
        let conn = open_agent_sessions_db(&workspace)?;
        conn.execute(
            "DELETE FROM sessions WHERE session_key = ?1",
            params![session_id],
        )?;
        conn.execute(
            "UPDATE session_metadata SET message_count = 0, last_activity = ?1
             WHERE session_key = ?2",
            params![now, session_id],
        )?;
        Ok::<(), rusqlite::Error>(())
    });

    match result {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"status": "ok"}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("清空失败: {e}")})),
        )
            .into_response(),
    }
}

/// POST /api/sessions/{id}/generate-title — LLM 自动生成标题
///
/// 读取最近几条消息，调用 Provider 生成简短标题，更新到 desktop_sessions。
async fn generate_title(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    let config = state.config.lock().clone();
    let config_dir = config.config_path.parent().unwrap_or(&config.workspace_dir);

    let tenant_dir =
        crate::huanxing::api_agents::extract_tenant_dir(&headers, config_dir, &config.huanxing)
            .await;
    let Some((_agent_id, workspace)) =
        find_session_owner(&config, config_dir, &session_id, tenant_dir.as_deref()).await
    else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "会话不存在"})),
        )
            .into_response();
    };

    // 读取最近 6 条消息用于生成标题
    let messages: Vec<(String, String)> = tokio::task::block_in_place(|| {
        let conn = match open_agent_sessions_db(&workspace) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        let mut stmt = match conn.prepare(
            "SELECT role, content FROM sessions WHERE session_key = ?1 ORDER BY id DESC LIMIT 6",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let rows = match stmt.query_map(params![session_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        }) {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };
        let mut msgs: Vec<(String, String)> = rows.filter_map(|r| r.ok()).collect();
        msgs.reverse();
        msgs
    });

    if messages.is_empty() {
        return (
            StatusCode::OK,
            Json(serde_json::json!({ "title": "新会话" })),
        )
            .into_response();
    }

    // 构造 prompt
    let context: String = messages
        .iter()
        .take(6)
        .map(|(role, content)| {
            let c = content.chars().take(200).collect::<String>();
            format!("{}: {}", role, c)
        })
        .collect::<Vec<_>>()
        .join("\n");

    let prompt =
        format!("请为以下对话生成一个简短的标题（不超过20个字，不要加引号）：\n\n{context}");

    // 调用 LLM
    let provider = state.provider.clone();
    let title_model = config
        .title_model
        .clone()
        .or_else(|| config.default_model.clone())
        .unwrap_or_else(|| "claude-haiku-4-5".to_string());

    let title = match provider
        .chat_with_system(None, &prompt, &title_model, config.default_temperature)
        .await
    {
        Ok(response) => {
            let t = response.trim().trim_matches('"').to_string();
            if t.is_empty() {
                "新会话".to_string()
            } else {
                t
            }
        }
        Err(e) => {
            tracing::warn!("生成标题失败: {e}");
            // 降级：取第一条用户消息前 20 字
            messages
                .iter()
                .find(|(role, _)| role == "user")
                .map(|(_, content)| content.chars().take(20).collect())
                .unwrap_or_else(|| "新会话".to_string())
        }
    };

    // 更新 DB
    let title_clone = title.clone();
    let now = Utc::now().to_rfc3339();
    let sid = session_id.clone();
    let update_result = tokio::task::block_in_place(move || {
        let conn = open_agent_sessions_db(&workspace)?;
        conn.execute(
            "UPDATE desktop_sessions SET title = ?1, updated_at = ?2 WHERE session_id = ?3",
            params![title_clone, now, sid],
        )?;
        Ok::<(), rusqlite::Error>(())
    });

    if let Err(e) = update_result {
        tracing::warn!("更新标题到 DB 失败: {e}");
    }

    (StatusCode::OK, Json(serde_json::json!({ "title": title }))).into_response()
}

// ── 辅助函数 ──────────────────────────────────────────────

/// 扫描所有 agent 目录，找到包含指定 session_id 的 agent workspace。
async fn find_session_owner(
    config: &crate::config::schema::Config,
    config_dir: &FsPath,
    session_id: &str,
    tenant_dir: Option<&str>,
) -> Option<(String, std::path::PathBuf)> {
    let tenant_root = config.huanxing.resolve_tenant_root(config_dir, tenant_dir);
    let agents_dir = tenant_root.join("agents");

    let mut entries = tokio::fs::read_dir(&agents_dir).await.ok()?;
    let sid = session_id.to_string();

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }

        let sid_clone = sid.clone();

        // Find the inner workspace directly
        let workspace = config
            .huanxing
            .resolve_agent_workspace(config_dir, tenant_dir, &name);
        if !workspace.exists() {
            continue;
        }

        let path_clone = workspace.clone();
        let found = tokio::task::block_in_place(move || {
            let conn = match open_agent_sessions_db(&path_clone) {
                Ok(c) => c,
                Err(_) => return false,
            };
            conn.query_row(
                "SELECT COUNT(*) FROM desktop_sessions WHERE session_id = ?1",
                params![sid_clone],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0)
                > 0
        });

        if found {
            return Some((name, workspace));
        }
    }
    None
}
