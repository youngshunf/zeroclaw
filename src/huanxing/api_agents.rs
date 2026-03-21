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

// ── 数据结构 ──────────────────────────────────────────────

/// 单个 Agent 信息
#[derive(Debug, Serialize)]
pub struct AgentInfo {
    /// Agent 目录名（即 agent_id）
    pub name: String,
    /// 显示名称（从 config.toml 的 display_name 读取，没有则为空）
    pub display_name: Option<String>,
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
    /// 显示名称
    pub display_name: Option<String>,
    /// 模型名称
    pub model: Option<String>,
    /// 温度
    pub temperature: Option<f64>,
    /// Hub 模板 ID（从 hub/templates/ 加载）
    pub template: Option<String>,
    /// 覆盖写入 SOUL.md（优先于 template）
    pub soul_md: Option<String>,
    /// 覆盖写入 IDENTITY.md
    pub identity_md: Option<String>,
    /// 覆盖写入 AGENTS.md
    pub agents_md: Option<String>,
    /// 覆盖写入 USER.md
    pub user_md: Option<String>,
    /// 覆盖写入 TOOLS.md
    pub tools_md: Option<String>,
    /// Provider API key（来自用户登录 session 的 llm_token）
    pub api_key: Option<String>,
    /// Provider base URL
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
    pub default_model: Option<String>,
    pub default_provider: Option<String>,
    pub default_temperature: Option<f64>,
}

// ── 路由 ──────────────────────────────────────────────────

/// 返回唤星桌面端 Agent 管理路由集合。
pub fn agent_routes() -> Router<AppState> {
    Router::new()
        .route("/api/agents", get(list_agents).post(create_agent))
        .route("/api/agents/:name", delete(delete_agent))
        .route("/api/agents/:name/files", get(list_files))
        .route(
            "/api/agents/:name/files/:filename",
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

    let agents_dir = config.huanxing.resolve_agents_dir(&config.workspace_dir);

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

/// POST /api/agents — 创建新 Agent
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

    let agents_dir = config.huanxing.resolve_agents_dir(&config.workspace_dir);
    let workspace = agents_dir.join(&req.name);

    // 检查是否已存在
    if workspace.exists() {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error": format!("agent '{}' 已存在", req.name)})),
        )
            .into_response();
    }

    // 创建工作区目录
    if let Err(e) = tokio::fs::create_dir_all(&workspace).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("创建工作区目录失败: {e}")})),
        )
            .into_response();
    }

    // 确定各工作区文件内容：优先 request 直传 > hub 模板 > 内置默认
    let template_files = if let Some(ref tmpl) = req.template {
        load_hub_template_files(&config, tmpl).await
    } else {
        TemplateFiles::default()
    };

    // 写入工作区文件
    let files: &[(&str, Option<&str>, &str)] = &[
        ("SOUL.md", req.soul_md.as_deref().or(template_files.soul_md.as_deref()), DEFAULT_SOUL_MD),
        ("IDENTITY.md", req.identity_md.as_deref().or(template_files.identity_md.as_deref()), DEFAULT_IDENTITY_MD),
        ("AGENTS.md", req.agents_md.as_deref().or(template_files.agents_md.as_deref()), DEFAULT_AGENTS_MD),
        ("USER.md", req.user_md.as_deref().or(template_files.user_md.as_deref()), DEFAULT_USER_MD),
        ("TOOLS.md", req.tools_md.as_deref().or(template_files.tools_md.as_deref()), DEFAULT_TOOLS_MD),
        ("MEMORY.md", None, DEFAULT_MEMORY_MD),
    ];

    for (filename, custom, default) in files {
        let content = custom.unwrap_or(default);
        let path = workspace.join(filename);
        if let Err(e) = tokio::fs::write(&path, content).await {
            tracing::warn!(path = %path.display(), "写入工作区文件失败: {e}");
        }
    }

    // 写入 config.toml
    let config_content = build_workspace_config_toml(&req);
    if let Err(e) = tokio::fs::write(workspace.join("config.toml"), &config_content).await {
        tracing::warn!(workspace = %workspace.display(), "写入 config.toml 失败: {e}");
    }

    tracing::info!(
        name = req.name,
        workspace = %workspace.display(),
        template = req.template.as_deref().unwrap_or("default"),
        "Agent 工作区创建完成"
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

    let agents_dir = config.huanxing.resolve_agents_dir(&config.workspace_dir);
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
    let agents_dir = config.huanxing.resolve_agents_dir(&config.workspace_dir);
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
    let agents_dir = config.huanxing.resolve_agents_dir(&config.workspace_dir);
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
    let agents_dir = config.huanxing.resolve_agents_dir(&config.workspace_dir);
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

/// 校验 agent name 只包含 [a-zA-Z0-9_-] 且长度 ≤ 32
fn is_valid_agent_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 32
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// 从工作区目录加载 config.toml 的部分字段
async fn load_workspace_config(workspace: &std::path::Path) -> WorkspaceConfig {
    let path = workspace.join("config.toml");
    let Ok(content) = tokio::fs::read_to_string(&path).await else {
        return WorkspaceConfig::default();
    };
    toml::from_str(&content).unwrap_or_default()
}

/// 从 hub 模板目录加载工作区文件内容
async fn load_hub_template_files(
    config: &crate::config::Config,
    template_id: &str,
) -> TemplateFiles {
    let hub_dir = match config.huanxing.resolve_hub_dir() {
        Some(d) => d,
        None => {
            // hub_dir 未配置，尝试默认路径
            config.workspace_dir.join("hub")
        }
    };

    let template_dir = hub_dir.join("templates").join(template_id);
    if !template_dir.exists() {
        // 尝试 _base 基础模板
        let base_dir = hub_dir.join("templates").join("_base");
        if !base_dir.exists() {
            return TemplateFiles::default();
        }
        return load_template_dir(&base_dir).await;
    }

    // 先加载 _base，再用 template 文件覆盖
    let base_dir = hub_dir.join("templates").join("_base");
    let mut files = if base_dir.exists() {
        load_template_dir(&base_dir).await
    } else {
        TemplateFiles::default()
    };

    let tmpl_files = load_template_dir(&template_dir).await;
    if tmpl_files.soul_md.is_some() { files.soul_md = tmpl_files.soul_md; }
    if tmpl_files.identity_md.is_some() { files.identity_md = tmpl_files.identity_md; }
    if tmpl_files.agents_md.is_some() { files.agents_md = tmpl_files.agents_md; }
    if tmpl_files.user_md.is_some() { files.user_md = tmpl_files.user_md; }
    if tmpl_files.tools_md.is_some() { files.tools_md = tmpl_files.tools_md; }

    files
}

/// 从目录加载模板文件
async fn load_template_dir(dir: &std::path::Path) -> TemplateFiles {
    let read = |name: &str| {
        let path = dir.join(name);
        async move { tokio::fs::read_to_string(path).await.ok() }
    };

    TemplateFiles {
        soul_md: read("SOUL.md").await,
        identity_md: read("IDENTITY.md").await,
        agents_md: read("AGENTS.md").await,
        user_md: read("USER.md").await,
        tools_md: read("TOOLS.md").await,
    }
}

/// 从 CreateAgentRequest 生成 config.toml 内容
fn build_workspace_config_toml(req: &CreateAgentRequest) -> String {
    let mut lines: Vec<String> = Vec::new();

    if let Some(ref name) = req.display_name {
        lines.push(format!("display_name = {:?}", name));
    }
    if let Some(ref model) = req.model {
        lines.push(format!("default_model = {:?}", model));
    }
    if let Some(temp) = req.temperature {
        lines.push(format!("default_temperature = {temp}"));
    }
    if let Some(ref key) = req.api_key {
        lines.push(format!("api_key = {:?}", key));
    }
    if let Some(ref url) = req.base_url {
        // base_url 存到 [llm] section
        lines.push(String::new());
        lines.push("[llm]".to_string());
        lines.push(format!("base_url = {:?}", url));
    }

    lines.join("\n")
}

// ── 模板文件持有者 ──────────────────────────────────────

#[derive(Default)]
struct TemplateFiles {
    soul_md: Option<String>,
    identity_md: Option<String>,
    agents_md: Option<String>,
    user_md: Option<String>,
    tools_md: Option<String>,
}

// ── 内置默认工作区文件 ────────────────────────────────────

const DEFAULT_SOUL_MD: &str = r#"# SOUL.md

你是一个友好、高效的 AI 助手。你善于倾听，回答清晰，能处理各种问题。

## 性格特点
- 真诚、有耐心、乐于助人
- 回答简洁，有条理
- 善于总结和组织信息

## 行为边界
- 保护用户隐私
- 诚实表达不确定性
- 拒绝有害请求
"#;

const DEFAULT_IDENTITY_MD: &str = r#"# IDENTITY.md

- **Platform:** 唤星 AI
- **Version:** 1.0
"#;

const DEFAULT_AGENTS_MD: &str = r#"# AGENTS.md

## 基本原则

1. 始终以用户利益为优先
2. 保持诚实，不夸大能力
3. 遇到不确定的问题，主动说明
4. 保护用户隐私，不泄露敏感信息
"#;

const DEFAULT_USER_MD: &str = r#"# USER.md

## 用户信息

暂无用户信息记录。
"#;

const DEFAULT_TOOLS_MD: &str = r#"# TOOLS.md

## 工具使用规范

- 只在必要时使用工具
- 使用工具前说明用途
- 工具执行失败时优雅降级
"#;

const DEFAULT_MEMORY_MD: &str = r#"# MEMORY.md

## 长期记忆

暂无记忆记录。
"#;
