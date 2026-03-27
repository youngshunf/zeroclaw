//! Agent 管理 HTTP API。
//!
//! 提供桌面端专用的 Agent CRUD 接口和工作区文件读写接口。
//! 路由通过 `huanxing_routes()` 注册到 gateway。
//!
//! # 端点
//!
//! ```
//! GET    /api/agents                          → 列出所有 Agent
//! POST   /api/agents                          → 从模板创建 Agent
//! DELETE /api/agents/:name                    → 删除 Agent
//! GET    /api/agents/:name/files              → 列出工作区文件
//! GET    /api/agents/:name/files/:filename    → 读取工作区文件
//! PUT    /api/agents/:name/files/:filename    → 写入工作区文件
//! ```

use axum::{
    body::Bytes,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::gateway::AppState;
use crate::huanxing::templates::{TemplateEngine, UserInfo, WorkspaceVariant};

// ── 数据结构 ──────────────────────────────────────────────

/// 单个 Agent 信息
#[derive(Debug, Serialize)]
pub struct AgentInfo {
    /// Agent 目录名（即 agent_name，支持中文）
    pub name: String,
    /// 显示名称（从 config.toml 的 display_name 读取，没有则为空）
    pub display_name: Option<String>,
    /// HASN 身份 ID（从 config.toml 的 hasn_id 读取，未注册时为空）
    pub hasn_id: Option<String>,
    /// 工作区路径
    pub config_dir: String,
    /// 使用的模型
    pub model: Option<String>,
    /// 是否存在（目录存在即为 true）
    pub active: bool,
    /// 是否为默认 Agent（预留，当前始终 false）
    pub is_default: bool,
}

/// 列出 Agent 响应
#[derive(Debug, Serialize)]
pub struct AgentListResponse {
    pub agents: Vec<AgentInfo>,
    pub current: String,
}

/// 创建 Agent 请求体
#[derive(Debug, Deserialize)]
pub struct CreateAgentRequest {
    /// Agent 目录名（只允许 [a-zA-Z0-9_-]，长度 ≤ 32）
    pub name: String,
    /// 显示名称（用于 config.toml display_name 和 {{star_name}} 占位符）
    pub display_name: Option<String>,
    /// Hub 模板 ID（从 hub/templates/ 加载），默认 "_base"
    pub template: Option<String>,
    /// Provider API key（来自用户登录 session 的 llm_token）
    /// 仅云端多租户使用；桌面端此字段忽略（继承全局配置）
    pub api_key: Option<String>,
    /// Provider base URL（仅云端多租户使用）
    pub base_url: Option<String>,
}

/// 创建 Agent 响应
#[derive(Debug, Serialize)]
pub struct CreateAgentResponse {
    pub status: String,
    pub name: String,
    pub config_dir: String,
}

/// 工作区 config.toml 的部分字段（用于读取 display_name / model）
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct WorkspaceConfig {
    pub display_name: Option<String>,
    pub hasn_id: Option<String>,
    pub default_model: Option<String>,
    pub default_provider: Option<String>,
    pub default_temperature: Option<f64>,
}

// ── 路由 ──────────────────────────────────────────────────

/// 返回唤星桌面端 Agent 管理路由集合。
pub fn agent_routes() -> Router<AppState> {
    Router::new()
        .route("/api/agents", get(list_agents).post(create_agent))
        .route("/api/agents/{name}", delete(delete_agent))
        .route("/api/agents/{name}/files", get(list_files))
        .route(
            "/api/agents/{name}/files/{filename}",
            get(read_file).put(write_file),
        )
}

// ── 处理函数 ──────────────────────────────────────────────

/// GET /api/agents — 扫描 agents_dir，列出所有 Agent
async fn list_agents(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.config.lock().clone();
    if !config.huanxing.enabled {
        return (StatusCode::OK, Json(AgentListResponse { agents: vec![], current: String::new() })).into_response();
    }

    let agents_dir = config.huanxing.resolve_agents_dir(config.config_path.parent().unwrap_or(&config.workspace_dir));

    let mut agents: Vec<AgentInfo> = Vec::new();

    match tokio::fs::read_dir(&agents_dir).await {
        Ok(mut entries) => {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().to_string();
                // 跳过隐藏目录
                if name.starts_with('.') {
                    continue;
                }

                let ws_cfg = load_workspace_config(&path).await;
                agents.push(AgentInfo {
                    config_dir: path.to_string_lossy().to_string(),
                    model: ws_cfg.default_model,
                    display_name: ws_cfg.display_name,
                    hasn_id: ws_cfg.hasn_id,
                    active: true,
                    is_default: false,
                    name,
                });
            }
        }
        Err(e) => {
            tracing::warn!(agents_dir = %agents_dir.display(), "列出 agents 目录失败: {e}");
        }
    }

    // 按名称排序
    agents.sort_by(|a, b| a.name.cmp(&b.name));

    let response = AgentListResponse {
        agents,
        current: String::new(),
    };
    (StatusCode::OK, Json(response)).into_response()
}

/// POST /api/agents — 从 hub 模板创建桌面端 Agent
async fn create_agent(
    State(state): State<AppState>,
    Json(req): Json<CreateAgentRequest>,
) -> impl IntoResponse {
    let config = state.config.lock().clone();

    // 校验 name 格式
    if !is_valid_agent_name(&req.name) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "agent name 只允许 [a-zA-Z0-9_-]，长度 ≤ 32"})),
        )
            .into_response();
    }

    let config_dir = config.config_path.parent().unwrap_or(&config.workspace_dir);
    let agents_dir = config.huanxing.resolve_agents_dir(config_dir);
    let workspace = agents_dir.join(&req.name);

    // 检查是否已存在
    if workspace.exists() {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error": format!("agent '{}' 已存在", req.name)})),
        )
            .into_response();
    }

    // 确定 hub 模板目录
    let hub_dir = config.huanxing.resolve_hub_dir()
        .unwrap_or_else(|| config.workspace_dir.join("hub"));
    let templates_dir = hub_dir.join("templates");

    let template_id = req.template.as_deref().unwrap_or("_base");
    let display_name = req.display_name.as_deref().unwrap_or(&req.name);

    let user_info = UserInfo {
        nickname: display_name,
        phone: "",
        star_name: display_name,
        user_id: &req.name,
        agent_id: &req.name,
        template: template_id,
    };

    let engine = TemplateEngine::new(templates_dir);

    match engine
        .create_workspace(&workspace, &user_info, None, None, WorkspaceVariant::Desktop)
        .await
    {
        Ok(files) => {
            tracing::info!(
                name = req.name,
                workspace = %workspace.display(),
                template = template_id,
                files = files.len(),
                "桌面端 Agent 工作区创建完成"
            );
            (
                StatusCode::CREATED,
                Json(CreateAgentResponse {
                    status: "ok".to_string(),
                    name: req.name.clone(),
                    config_dir: workspace.to_string_lossy().to_string(),
                }),
            )
                .into_response()
        }
        Err(e) => {
            // 创建失败时清理已创建的目录
            let _ = tokio::fs::remove_dir_all(&workspace).await;
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("创建工作区失败: {e}")})),
            )
                .into_response()
        }
    }
}

/// DELETE /api/agents/:name — 删除 Agent 工作区
async fn delete_agent(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let config = state.config.lock().clone();

    if !is_valid_agent_name(&name) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "非法 agent name"})),
        )
            .into_response();
    }

    let agents_dir = config.huanxing.resolve_agents_dir(config.config_path.parent().unwrap_or(&config.workspace_dir));
    let workspace = agents_dir.join(&name);

    if !workspace.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": format!("agent '{}' 不存在", name)})),
        )
            .into_response();
    }

    if let Err(e) = tokio::fs::remove_dir_all(&workspace).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("删除工作区失败: {e}")})),
        )
            .into_response();
    }

    tracing::info!(name, "Agent 工作区已删除");

    (StatusCode::OK, Json(serde_json::json!({"status": "ok", "name": name}))).into_response()
}

/// GET /api/agents/:name/files — 列出工作区文件
async fn list_files(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let config = state.config.lock().clone();
    let agents_dir = config.huanxing.resolve_agents_dir(config.config_path.parent().unwrap_or(&config.workspace_dir));
    let workspace = agents_dir.join(&name);

    if !workspace.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": format!("agent '{}' 不存在", name)})),
        )
            .into_response();
    }

    let mut files: Vec<String> = Vec::new();
    if let Ok(mut entries) = tokio::fs::read_dir(&workspace).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.path().is_file() {
                files.push(entry.file_name().to_string_lossy().to_string());
            }
        }
    }
    files.sort();

    (StatusCode::OK, Json(serde_json::json!({"files": files}))).into_response()
}

/// GET /api/agents/:name/files/:filename — 读取工作区文件
async fn read_file(
    State(state): State<AppState>,
    Path((name, filename)): Path<(String, String)>,
) -> impl IntoResponse {
    let config = state.config.lock().clone();
    let agents_dir = config.huanxing.resolve_agents_dir(config.config_path.parent().unwrap_or(&config.workspace_dir));
    let file_path = agents_dir.join(&name).join(&filename);

    // 防止路径遍历
    if filename.contains("..") || filename.contains('/') {
        return (StatusCode::BAD_REQUEST, "非法文件名".to_string()).into_response();
    }

    match tokio::fs::read_to_string(&file_path).await {
        Ok(content) => (
            StatusCode::OK,
            Json(serde_json::json!({"content": content})),
        )
            .into_response(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "文件不存在"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("读取失败: {e}")})),
        )
            .into_response(),
    }
}

/// PUT /api/agents/:name/files/:filename — 写入工作区文件（纯文本 body）
async fn write_file(
    State(state): State<AppState>,
    Path((name, filename)): Path<(String, String)>,
    body: Bytes,
) -> impl IntoResponse {
    let config = state.config.lock().clone();
    let agents_dir = config.huanxing.resolve_agents_dir(config.config_path.parent().unwrap_or(&config.workspace_dir));
    let workspace = agents_dir.join(&name);

    // 防止路径遍历
    if filename.contains("..") || filename.contains('/') {
        return (StatusCode::BAD_REQUEST, "非法文件名").into_response();
    }

    if !workspace.exists() {
        return (StatusCode::NOT_FOUND, "agent 不存在").into_response();
    }

    let file_path = workspace.join(&filename);
    match tokio::fs::write(&file_path, &body).await {
        Ok(_) => (StatusCode::OK, "ok").into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("写入失败: {e}"),
        )
            .into_response(),
    }
}

// ── 辅助函数 ──────────────────────────────────────────────

/// 校验 agent name：允许中文、字母、数字、下划线、连字符，长度 ≤ 32 字符（UTF-8）
/// 禁止：空名、以 . 开头、包含 / \ \0
fn is_valid_agent_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().count() <= 32
        && !name.starts_with('.')
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains('\0')
}

/// 从工作区目录加载 config.toml 的部分字段
async fn load_workspace_config(workspace: &std::path::Path) -> WorkspaceConfig {
    let path = workspace.join("config.toml");
    let Ok(content) = tokio::fs::read_to_string(&path).await else {
        return WorkspaceConfig::default();
    };
    toml::from_str(&content).unwrap_or_default()
}

