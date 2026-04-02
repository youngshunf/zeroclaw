//! Agent 管理 HTTP API。
//!
//! 提供桌面端专用的 Agent CRUD 接口和工作区文件读写接口。
//! 路由通过 `huanxing_routes()` 注册到 gateway。
//!
//! # 端点
//!
//! ```text
//! GET    /api/agents                          → 列出所有 Agent
//! POST   /api/agents                          → 从模板创建 Agent
//! DELETE /api/agents/:name                    → 删除 Agent
//! GET    /api/agents/:name/files              → 列出工作区文件
//! GET    /api/agents/:name/files/:filename    → 读取工作区文件
//! PUT    /api/agents/:name/files/:filename    → 写入工作区文件
//! ```

use axum::{
    Json, Router,
    body::Bytes,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
};
use serde::{Deserialize, Serialize};

use crate::gateway::AppState;

pub async fn extract_tenant_dir(
    headers: &axum::http::HeaderMap,
    config_dir: &std::path::Path,
    config: &crate::huanxing::config::HuanXingConfig,
) -> Option<String> {
    if let Some(tenant) = headers.get("x-tenant-dir").and_then(|v| v.to_str().ok()) {
        return Some(tenant.to_string());
    }
    if let Some(tenant) = headers.get("x-tenant-id").and_then(|v| v.to_str().ok()) {
        return Some(tenant.to_string());
    }

    let db_path = config.resolve_db_path(config_dir);
    if let Ok(db) = crate::huanxing::db::TenantDb::open(&db_path) {
        if let Ok(Some(tenant)) = db.get_first_tenant_dir().await {
            return Some(tenant);
        }
    }
    None
}

async fn require_tenant_dir(
    headers: &axum::http::HeaderMap,
    config_dir: &std::path::Path,
    config: &crate::huanxing::config::HuanXingConfig,
) -> Result<String, String> {
    extract_tenant_dir(headers, config_dir, config)
        .await
        .filter(|tenant| !tenant.trim().is_empty())
        .ok_or_else(|| "未解析到 tenant_dir，拒绝创建 Agent 以避免写入 users/default".to_string())
}

fn build_create_agent_params(
    req: &CreateAgentRequest,
    tenant_dir: &str,
) -> huanxing_agent_factory::CreateAgentParams {
    let template_id = req.template.as_deref().unwrap_or("_base");
    let display_name = req.display_name.as_deref().unwrap_or(&req.name);

    huanxing_agent_factory::CreateAgentParams {
        tenant_id: tenant_dir.to_string(),
        template_id: template_id.to_string(),
        agent_name: req.name.clone(),
        display_name: display_name.to_string(),
        is_desktop: req.is_desktop.unwrap_or(false),
        user_nickname: display_name.to_string(),
        provider: None,
        model: None,
        api_key: req.api_key.clone(),
        hasn_id: None,
        fallback_provider: None,
        embedding_provider: None,
        llm_gateway: None,
    }
}

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
    /// 代理图标 URL
    pub icon_url: Option<String>,
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
    /// 是否使用桌面端覆盖层
    pub is_desktop: Option<bool>,
}

/// 创建 Agent 响应
#[derive(Debug, Serialize)]
pub struct CreateAgentResponse {
    pub status: String,
    pub name: String,
    pub config_dir: String,
}

#[derive(Debug, Deserialize)]
struct UpdateAgentHasnIdRequest {
    hasn_id: String,
}

/// 工作区 config.toml 的部分字段（用于读取 display_name / model）

// ── 路由 ──────────────────────────────────────────────────

/// 返回唤星桌面端 Agent 管理路由集合。
pub fn agent_routes() -> Router<AppState> {
    Router::new()
        .route("/api/agents", get(list_agents).post(create_agent))
        .route("/api/agents/{name}", delete(delete_agent))
        .route("/api/agents/{name}/hasn-id", post(update_agent_hasn_id))
        .route("/api/agents/{name}/files", get(list_files))
        .route(
            "/api/agents/{name}/files/{filename}",
            get(read_file).put(write_file),
        )
        .route("/api/audio/transcribe", post(handle_audio_transcribe))
        .route("/api/upload", post(handle_file_upload))
}

// ── 处理函数 ──────────────────────────────────────────────

/// GET /api/agents — 扫描 agents_dir，列出所有 Agent
async fn list_agents(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let config = state.config.lock().clone();
    if !config.huanxing.enabled {
        return (
            StatusCode::OK,
            Json(AgentListResponse {
                agents: vec![],
                current: String::new(),
            }),
        )
            .into_response();
    }

    let config_dir = config.config_path.parent().unwrap_or(&config.workspace_dir);
    let tenant_dir =
        crate::huanxing::api_agents::extract_tenant_dir(&headers, config_dir, &config.huanxing)
            .await;
    // Desktop list: we list all agents in the desktop tenant_root.
    // However, the physical structure for Desktop is `users/{tenant_dir}/agents/{agent_id}/`.
    // We can just read `agents/` to find them.
    let agents_dir = config
        .huanxing
        .resolve_tenant_root(config_dir, tenant_dir.as_deref())
        .join("agents");

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
                let icon_url = if path.join("icon.svg").exists() {
                    Some(format!(
                        "/api/agents/{}/files/icon.svg?raw=true",
                        urlencoding::encode(&name)
                    ))
                } else if path.join("icon.png").exists() {
                    Some(format!(
                        "/api/agents/{}/files/icon.png?raw=true",
                        urlencoding::encode(&name)
                    ))
                } else {
                    None
                };

                let display_name = ws_cfg
                    .display_name
                    .or_else(|| ws_cfg.name.clone())
                    .or_else(|| ws_cfg.identity.as_ref().and_then(|id| id.name.clone()))
                    .or_else(|| config.display_name.clone());

                agents.push(AgentInfo {
                    config_dir: path.join("workspace").to_string_lossy().to_string(), // point to the inner workspace
                    model: ws_cfg.default_model,
                    display_name,
                    hasn_id: ws_cfg.hasn_id,
                    active: true,
                    is_default: false,
                    icon_url,
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
    headers: axum::http::HeaderMap,
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
    let tenant_dir = match require_tenant_dir(&headers, config_dir, &config.huanxing).await {
        Ok(tenant_dir) => tenant_dir,
        Err(err) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": err })),
            )
                .into_response();
        }
    };
    let agent_wrapper =
        config
            .huanxing
            .resolve_agent_wrapper_dir(config_dir, Some(&tenant_dir), &req.name);

    // 检查是否已存在
    if agent_wrapper.exists() {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error": format!("agent '{}' 已存在", req.name)})),
        )
            .into_response();
    }

    // 确定 hub 模板目录
    let hub_dir = config
        .huanxing
        .resolve_hub_dir()
        .unwrap_or_else(|| config.workspace_dir.join("hub"));
    let templates_dir = hub_dir.join("templates");

    let template_id = req.template.as_deref().unwrap_or("_base");

    let factory = huanxing_agent_factory::AgentFactory::new(config_dir.to_path_buf(), None);
    let params = build_create_agent_params(&req, &tenant_dir);

    struct ApiProgress;
    impl huanxing_agent_factory::ProgressSink for ApiProgress {
        fn on_progress(&self, step: &str, detail: &str) {
            tracing::debug!("Agent API create progress: {} - {}", step, detail);
        }
        fn on_error(&self, step: &str, error: &str) {
            tracing::warn!("Agent API create error: {} - {}", step, error);
        }
    }

    match factory
        .create_local_agent(&templates_dir, &params, &ApiProgress)
        .await
    {
        Ok(res) => {
            tracing::info!(
                name = req.name,
                workspace = %res.workspace_dir.display(),
                template = template_id,
                "API: Agent 工作区创建完成"
            );
            (
                StatusCode::CREATED,
                Json(CreateAgentResponse {
                    status: "ok".to_string(),
                    name: req.name.clone(),
                    config_dir: res.workspace_dir.to_string_lossy().to_string(),
                }),
            )
                .into_response()
        }
        Err(e) => {
            // 创建失败时清理已创建的目录
            let _ = tokio::fs::remove_dir_all(&agent_wrapper).await;
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
    headers: axum::http::HeaderMap,
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

    let config_dir = config.config_path.parent().unwrap_or(&config.workspace_dir);
    let tenant_dir =
        crate::huanxing::api_agents::extract_tenant_dir(&headers, config_dir, &config.huanxing)
            .await;
    let agent_wrapper =
        config
            .huanxing
            .resolve_agent_wrapper_dir(config_dir, tenant_dir.as_deref(), &name);

    if !agent_wrapper.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": format!("agent '{}' 不存在", name)})),
        )
            .into_response();
    }

    if let Err(e) = tokio::fs::remove_dir_all(&agent_wrapper).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("删除工作区失败: {e}")})),
        )
            .into_response();
    }

    tracing::info!(name, "Agent 工作区已删除");

    (
        StatusCode::OK,
        Json(serde_json::json!({"status": "ok", "name": name})),
    )
        .into_response()
}

async fn update_agent_hasn_id(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Path(name): Path<String>,
    Json(req): Json<UpdateAgentHasnIdRequest>,
) -> impl IntoResponse {
    if !is_valid_agent_name(&name) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "非法 agent name"})),
        )
            .into_response();
    }

    if req.hasn_id.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "hasn_id 不能为空"})),
        )
            .into_response();
    }

    let config = state.config.lock().clone();
    let config_dir = config.config_path.parent().unwrap_or(&config.workspace_dir);
    let tenant_dir = match require_tenant_dir(&headers, config_dir, &config.huanxing).await {
        Ok(tenant_dir) => tenant_dir,
        Err(err) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": err })),
            )
                .into_response();
        }
    };
    let workspace = config
        .huanxing
        .resolve_agent_workspace(config_dir, Some(&tenant_dir), &name);
    let _ = crate::huanxing::config::promote_legacy_agent_config_from_workspace(&workspace);
    let config_path =
        config
            .huanxing
            .resolve_agent_config_path(config_dir, Some(&tenant_dir), &name);

    let content = match tokio::fs::read_to_string(&config_path).await {
        Ok(content) => content,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "agent config.toml 不存在"})),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("读取 agent config 失败: {e}")})),
            )
                .into_response();
        }
    };

    let updated = upsert_hasn_id_in_config(&content, &req.hasn_id);
    if let Err(e) = tokio::fs::write(&config_path, updated).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("写回 agent config 失败: {e}")})),
        )
            .into_response();
    }

    let db_path = config.huanxing.resolve_db_path(config_dir);
    let db = match crate::huanxing::db::TenantDb::open(&db_path) {
        Ok(db) => db,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("打开 users.db 失败: {e}")})),
            )
                .into_response();
        }
    };

    match db.update_agent_hasn_id(&name, &req.hasn_id).await {
        Ok(true) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "ok", "name": name, "hasn_id": req.hasn_id})),
        )
            .into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": format!("agent '{}' 不存在于 users.db", name)})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("更新 users.db 失败: {e}")})),
        )
            .into_response(),
    }
}

/// GET /api/agents/:name/files — 列出工作区文件
async fn list_files(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let config = state.config.lock().clone();
    let config_dir = config.config_path.parent().unwrap_or(&config.workspace_dir);
    let tenant_dir =
        crate::huanxing::api_agents::extract_tenant_dir(&headers, config_dir, &config.huanxing)
            .await;
    let workspace =
        config
            .huanxing
            .resolve_agent_workspace(config_dir, tenant_dir.as_deref(), &name);

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

#[derive(Deserialize)]
struct ReadFileQuery {
    raw: Option<bool>,
}

/// GET /api/agents/:name/files/:filename — 读取工作区文件
async fn read_file(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Path((name, filename)): Path<(String, String)>,
    axum::extract::Query(query): axum::extract::Query<ReadFileQuery>,
) -> impl IntoResponse {
    let config = state.config.lock().clone();
    let config_dir = config.config_path.parent().unwrap_or(&config.workspace_dir);
    let tenant_dir =
        crate::huanxing::api_agents::extract_tenant_dir(&headers, config_dir, &config.huanxing)
            .await;
    let workspace =
        config
            .huanxing
            .resolve_agent_workspace(config_dir, tenant_dir.as_deref(), &name);
    let file_path = workspace.join(&filename);

    // 防止路径遍历
    if filename.contains("..") || filename.contains('/') {
        return (StatusCode::BAD_REQUEST, "非法文件名".to_string()).into_response();
    }

    if query.raw.unwrap_or(false) {
        match tokio::fs::read(&file_path).await {
            Ok(bytes) => {
                let content_type = if filename.ends_with(".svg") {
                    "image/svg+xml"
                } else if filename.ends_with(".png") {
                    "image/png"
                } else if filename.ends_with(".jpg") || filename.ends_with(".jpeg") {
                    "image/jpeg"
                } else {
                    "application/octet-stream"
                };
                return (
                    StatusCode::OK,
                    [(axum::http::header::CONTENT_TYPE, content_type)],
                    bytes,
                )
                    .into_response();
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return (StatusCode::NOT_FOUND, "文件不存在").into_response();
            }
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("读取文件失败: {e}"),
                )
                    .into_response();
            }
        }
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
    headers: axum::http::HeaderMap,
    Path((name, filename)): Path<(String, String)>,
    body: Bytes,
) -> impl IntoResponse {
    let config = state.config.lock().clone();
    let config_dir = config.config_path.parent().unwrap_or(&config.workspace_dir);
    let tenant_dir =
        crate::huanxing::api_agents::extract_tenant_dir(&headers, config_dir, &config.huanxing)
            .await;
    let workspace =
        config
            .huanxing
            .resolve_agent_workspace(config_dir, tenant_dir.as_deref(), &name);

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
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("写入失败: {e}")).into_response(),
    }
}

// ── 音频转录 API ──────────────────────────────────────────

/// STT 转录响应
#[derive(Debug, Serialize)]
struct TranscribeResponse {
    text: String,
}

/// POST /api/audio/transcribe — 接收音频文件（multipart/form-data），返回转录文本
///
/// 前端通过 MediaRecorder 录音后，将 WebM/OGG Blob 上传到此端点。
/// 使用配置中的 `[transcription]` 提供商进行语音识别。
async fn handle_audio_transcribe(
    State(state): State<AppState>,
    _headers: axum::http::HeaderMap,
    mut multipart: axum::extract::Multipart,
) -> impl IntoResponse {
    // 1. 从 multipart 中提取音频文件
    let mut audio_data: Option<Vec<u8>> = None;
    let mut file_name = String::from("voice.webm");

    while let Ok(Some(field)) = multipart.next_field().await {
        let field_name = field.name().unwrap_or("").to_string();
        if field_name == "file" {
            let fname = field.file_name().map(|s| s.to_string());
            if let Some(f) = fname {
                file_name = f;
            }
            match field.bytes().await {
                Ok(bytes) => {
                    audio_data = Some(bytes.to_vec());
                }
                Err(e) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({"error": format!("读取音频数据失败: {e}")})),
                    )
                        .into_response();
                }
            }
        }
    }

    let Some(data) = audio_data else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "请提供音频文件 (field name: file)"})),
        )
            .into_response();
    };

    if data.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "音频文件为空"})),
        )
            .into_response();
    }

    tracing::info!(
        file_name = %file_name,
        size_bytes = data.len(),
        "Audio transcribe: received audio file"
    );

    // 2. 使用配置中的转录提供商
    let config = state.config.lock().clone();

    match crate::channels::transcription::transcribe_audio(data, &file_name, &config.transcription)
        .await
    {
        Ok(text) => {
            tracing::info!(text_len = text.len(), "Audio transcribe: success");
            (StatusCode::OK, Json(TranscribeResponse { text })).into_response()
        }
        Err(e) => {
            tracing::warn!("Audio transcribe failed: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("语音识别失败: {e}")})),
            )
                .into_response()
        }
    }
}

// ── 文件上传 API ──────────────────────────────────────────

/// 文件上传响应
#[derive(Debug, Serialize)]
struct UploadResponse {
    path: String,
}

/// POST /api/upload — 上传文件到当前 Agent 工作区的 files/ 目录
///
/// 前端通过选择或粘贴文件后上传到此端点。
/// 文件保存到 agents/{current_agent}/files/{filename}。
/// 返回绝对路径，前端可用于 [IMAGE:path] 标记。
async fn handle_file_upload(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    mut multipart: axum::extract::Multipart,
) -> impl IntoResponse {
    let mut file_data: Option<Vec<u8>> = None;
    let mut file_name = String::from("upload");

    while let Ok(Some(field)) = multipart.next_field().await {
        let field_name = field.name().unwrap_or("").to_string();
        if field_name == "file" {
            let fname = field.file_name().map(|s| s.to_string());
            if let Some(f) = fname {
                file_name = f;
            }
            match field.bytes().await {
                Ok(bytes) => {
                    file_data = Some(bytes.to_vec());
                }
                Err(e) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({"error": format!("读取文件数据失败: {e}")})),
                    )
                        .into_response();
                }
            }
        }
    }

    let Some(data) = file_data else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "请提供文件 (field name: file)"})),
        )
            .into_response();
    };

    // 确定保存目录
    let config = state.config.lock().clone();
    let config_dir = config.config_path.parent().unwrap_or(&config.workspace_dir);
    let tenant_dir = match require_tenant_dir(&headers, config_dir, &config.huanxing).await {
        Ok(tenant_dir) => tenant_dir,
        Err(err) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": err })),
            )
                .into_response();
        }
    };
    let default_workspace =
        config
            .huanxing
            .resolve_agent_workspace(config_dir, Some(&tenant_dir), "default");

    // 桌面端默认使用 default agent
    let files_dir = default_workspace.join("files");

    // 创建 files/ 目录
    if let Err(e) = tokio::fs::create_dir_all(&files_dir).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("创建目录失败: {e}")})),
        )
            .into_response();
    }

    // 保存文件
    let dest_path = files_dir.join(&file_name);
    match tokio::fs::write(&dest_path, &data).await {
        Ok(_) => {
            let abs_path = dest_path.to_string_lossy().to_string();
            tracing::info!(
                path = %abs_path,
                size = data.len(),
                "File uploaded successfully"
            );
            (StatusCode::OK, Json(UploadResponse { path: abs_path })).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("保存文件失败: {e}")})),
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

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PartialIdentity {
    pub name: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct WorkspaceConfig {
    pub display_name: Option<String>,
    pub name: Option<String>,
    pub hasn_id: Option<String>,
    pub default_model: Option<String>,
    pub default_provider: Option<String>,
    pub default_temperature: Option<f64>,
    pub identity: Option<PartialIdentity>,
}

/// 从工作区目录加载 config.toml 的部分字段
async fn load_workspace_config(workspace: &std::path::Path) -> WorkspaceConfig {
    let _ = crate::huanxing::config::promote_legacy_agent_config_from_workspace(workspace);
    let path = crate::huanxing::config::agent_config_path_from_workspace(workspace);
    let Ok(content) = tokio::fs::read_to_string(&path).await else {
        return WorkspaceConfig::default();
    };
    toml::from_str(&content).unwrap_or_default()
}

fn upsert_hasn_id_in_config(content: &str, hasn_id: &str) -> String {
    if content.contains("hasn_id =") {
        return regex::Regex::new(r#"hasn_id\s*=\s*"[^"]*""#)
            .expect("valid hasn_id regex")
            .replace(content, format!(r#"hasn_id = "{hasn_id}""#))
            .into_owned();
    }

    if content.contains("[agent]") {
        return content.replacen("[agent]", &format!("[agent]\nhasn_id = \"{hasn_id}\""), 1);
    }

    format!("{content}\nhasn_id = \"{hasn_id}\"\n")
}

#[cfg(test)]
mod tests {
    use super::{
        CreateAgentRequest, build_create_agent_params, load_workspace_config, require_tenant_dir,
        upsert_hasn_id_in_config,
    };
    use crate::huanxing::config::HuanXingConfig;
    use crate::huanxing::db::TenantDb;
    use axum::http::HeaderMap;
    use tempfile::tempdir;

    async fn seed_users_db(config_dir: &std::path::Path, tenant_dir: &str) {
        let data_dir = config_dir.join("data");
        std::fs::create_dir_all(&data_dir).unwrap();
        let db_path = data_dir.join("users.db");
        let db = TenantDb::open(&db_path).unwrap();
        db.save_user_full(
            "user-001",
            "13800138000",
            "default",
            Some("Tester"),
            "assistant",
            Some("Tester"),
            None,
            Some(tenant_dir),
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn require_tenant_dir_uses_db_fallback() {
        let temp = tempdir().unwrap();
        let config_dir = temp.path().join(".huanxing");
        seed_users_db(&config_dir, "001-tenant-a").await;

        let tenant_dir =
            require_tenant_dir(&HeaderMap::new(), &config_dir, &HuanXingConfig::default())
                .await
                .unwrap();

        assert_eq!(tenant_dir, "001-tenant-a");
    }

    #[tokio::test]
    async fn require_tenant_dir_errors_when_missing() {
        let temp = tempdir().unwrap();
        let config_dir = temp.path().join(".huanxing");
        std::fs::create_dir_all(config_dir.join("data")).unwrap();

        let err = require_tenant_dir(&HeaderMap::new(), &config_dir, &HuanXingConfig::default())
            .await
            .unwrap_err();

        assert!(err.contains("tenant"));
    }

    #[test]
    fn build_create_agent_params_uses_resolved_tenant_dir() {
        let request = CreateAgentRequest {
            name: "assistant-130".to_string(),
            display_name: Some("Assistant 130".to_string()),
            template: Some("_base".to_string()),
            api_key: Some("session-token".to_string()),
            base_url: None,
            is_desktop: Some(true),
        };

        let params = build_create_agent_params(&request, "001-tenant-a");

        assert_eq!(params.tenant_id, "001-tenant-a");
        assert_eq!(params.agent_name, "assistant-130");
        assert_eq!(params.display_name, "Assistant 130");
        assert_eq!(params.template_id, "_base");
        assert!(params.is_desktop);
        assert_eq!(params.api_key.as_deref(), Some("session-token"));
    }

    #[test]
    fn upsert_hasn_id_in_config_replaces_existing_value() {
        let content = "[agent]\nhasn_id = \"old_id\"\nname = \"default\"\n";

        let updated = upsert_hasn_id_in_config(content, "new_id");

        assert!(updated.contains("hasn_id = \"new_id\""));
        assert!(!updated.contains("hasn_id = \"old_id\""));
    }

    #[test]
    fn upsert_hasn_id_in_config_inserts_into_agent_section() {
        let content = "[agent]\nname = \"default\"\n";

        let updated = upsert_hasn_id_in_config(content, "new_id");

        assert!(updated.contains("[agent]\nhasn_id = \"new_id\""));
    }

    #[tokio::test]
    async fn load_workspace_config_prefers_wrapper_config() {
        let temp = tempdir().unwrap();
        let wrapper = temp.path().join("agents").join("default");
        let workspace = wrapper.join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();

        tokio::fs::write(
            wrapper.join("config.toml"),
            "display_name = \"wrapper-name\"\ndefault_model = \"wrapper-model\"\n",
        )
        .await
        .unwrap();
        tokio::fs::write(
            workspace.join("config.toml"),
            "display_name = \"workspace-name\"\ndefault_model = \"workspace-model\"\n",
        )
        .await
        .unwrap();

        let config = load_workspace_config(&workspace).await;

        assert_eq!(config.display_name.as_deref(), Some("wrapper-name"));
        assert_eq!(config.default_model.as_deref(), Some("wrapper-model"));
    }

    #[tokio::test]
    async fn load_workspace_config_promotes_legacy_workspace_config() {
        let temp = tempdir().unwrap();
        let wrapper = temp.path().join("agents").join("default");
        let workspace = wrapper.join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();

        tokio::fs::write(
            workspace.join("config.toml"),
            "display_name = \"legacy-name\"\ndefault_model = \"legacy-model\"\n",
        )
        .await
        .unwrap();

        let config = load_workspace_config(&workspace).await;

        assert_eq!(config.display_name.as_deref(), Some("legacy-name"));
        assert_eq!(config.default_model.as_deref(), Some("legacy-model"));
        assert!(wrapper.join("config.toml").exists());
        assert!(!workspace.join("config.toml").exists());
    }
}
