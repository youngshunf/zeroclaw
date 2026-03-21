//! Hub Gitee 同步模块。
//!
//! 从 Gitee 仓库下载 huanxing-hub（tarball），解压后原子替换本地 hub 目录，
//! 使桌面端用户无需手动维护 hub 仓库即可获取最新模板和技能。
//!
//! # 触发时机
//!
//! - Sidecar 启动时：`hub/registry.json` 修改时间超过 `sync_interval_hours`
//! - Hub 目录不存在时（首次安装）
//! - 用户手动触发：`POST /api/hub/sync`
//! - 创建 Agent 时 hub 未初始化
//!
//! # 下载策略
//!
//! 使用 Gitee tarball 接口（一次 HTTP 请求，无需 git 命令）：
//! `GET https://gitee.com/{owner}/{repo}/repository/archive/{branch}.tar.gz`

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use crate::gateway::AppState;

// ── 数据结构 ──────────────────────────────────────────────

/// 同步结果
#[derive(Debug, Clone, Serialize)]
pub struct SyncResult {
    /// registry.json 中的版本号（如有）
    pub version: Option<String>,
    /// 模板数量
    pub templates: usize,
    /// 技能数量
    pub skills: usize,
    /// 本次是否有更新（重新下载了 tarball）
    pub updated: bool,
    /// 同步完成时间（UTC）
    pub synced_at: DateTime<Utc>,
}

/// Hub 同步状态（返回给前端）
#[derive(Debug, Clone, Serialize)]
pub struct SyncStatus {
    pub syncing: bool,
    pub last_result: Option<SyncResult>,
    pub error: Option<String>,
}

/// Hub 模板信息
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HubTemplate {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub emoji: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub pricing_tier: String,
}

/// hub/registry.json 的部分结构
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RegistryJson {
    version: Option<String>,
    templates: Vec<HubTemplate>,
    skills: Vec<serde_json::Value>,
}

/// 全局同步状态（注入 AppState Extension）
pub type SyncStateExt = Arc<Mutex<SyncStatus>>;

// ── 路由 ──────────────────────────────────────────────────

/// Hub 路由集合
pub fn hub_routes() -> Router<AppState> {
    Router::new()
        .route("/api/hub/templates", get(list_templates))
        .route("/api/hub/sync", post(trigger_sync))
        .route("/api/hub/sync/status", get(sync_status))
}

// ── 处理函数 ──────────────────────────────────────────────

/// GET /api/hub/templates — 列出 hub 中所有模板
async fn list_templates(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.config.lock().clone();
    let hub_dir = resolve_hub_dir(&config);

    let registry_path = hub_dir.join("registry.json");

    // 如果 hub 目录不存在或 registry 不存在，触发一次同步
    if !registry_path.exists() {
        if let Err(e) = run_sync(&hub_dir, &config.huanxing.hub_sync).await {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": format!("hub 同步失败: {e}"), "templates": []})  ),
            )
                .into_response();
        }
    }

    let templates = read_templates_from_registry(&registry_path).await;
    (StatusCode::OK, Json(serde_json::json!({"templates": templates}))).into_response()
}

/// POST /api/hub/sync — 手动触发 hub 同步
async fn trigger_sync(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.config.lock().clone();
    let hub_dir = resolve_hub_dir(&config);

    match run_sync(&hub_dir, &config.huanxing.hub_sync).await {
        Ok(result) => (StatusCode::OK, Json(serde_json::json!({
            "status": "ok",
            "updated": result.updated,
            "version": result.version,
            "templates": result.templates,
            "skills": result.skills,
            "synced_at": result.synced_at.to_rfc3339(),
        })))
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("{e}")})),
        )
            .into_response(),
    }
}

/// GET /api/hub/sync/status — 查询同步状态（当前无后台状态跟踪，返回 hub 目录信息）
async fn sync_status(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.config.lock().clone();
    let hub_dir = resolve_hub_dir(&config);
    let registry_path = hub_dir.join("registry.json");

    let (last_sync, version, templates_count) = if registry_path.exists() {
        let mtime = tokio::fs::metadata(&registry_path)
            .await
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| DateTime::<Utc>::from_timestamp(d.as_secs() as i64, 0))
            .flatten();

        let registry = read_registry(&registry_path).await;
        (mtime, registry.version, registry.templates.len())
    } else {
        (None, None, 0)
    };

    (StatusCode::OK, Json(serde_json::json!({
        "initialized": registry_path.exists(),
        "hub_dir": hub_dir.to_string_lossy(),
        "last_sync": last_sync.map(|t| t.to_rfc3339()),
        "version": version,
        "templates_count": templates_count,
    })))
        .into_response()
}

// ── 核心同步逻辑 ──────────────────────────────────────────

/// 执行 hub 同步：下载 tarball → 解压 → 原子替换
pub async fn run_sync(
    hub_dir: &PathBuf,
    sync_config: &crate::huanxing::config::HubSyncConfig,
) -> anyhow::Result<SyncResult> {
    let repo = &sync_config.gitee_repo;
    let branch = &sync_config.gitee_branch;

    // 下载 tarball
    let url = format!(
        "https://gitee.com/{repo}/repository/archive/{branch}.tar.gz"
    );

    tracing::info!(%url, "开始下载 hub tarball");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;

    let response = client
        .get(&url)
        .header("User-Agent", "huanxing-desktop/1.0")
        .send()
        .await?;

    if !response.status().is_success() {
        anyhow::bail!(
            "Gitee tarball 下载失败: HTTP {}",
            response.status()
        );
    }

    let bytes = response.bytes().await?;
    tracing::info!(size = bytes.len(), "tarball 下载完成");

    // 解压到临时目录
    let tmp_dir = hub_dir.with_extension("tmp");
    if tmp_dir.exists() {
        tokio::fs::remove_dir_all(&tmp_dir).await?;
    }
    tokio::fs::create_dir_all(&tmp_dir).await?;

    // 在阻塞线程中执行 tar 解压（CPU 密集型）
    let tmp_dir_clone = tmp_dir.clone();
    let bytes_clone = bytes.clone();
    tokio::task::spawn_blocking(move || {
        extract_tarball(&bytes_clone, &tmp_dir_clone)
    })
    .await??;

    // tarball 解压后通常有一级子目录（repo-branch/）
    // 找到 registry.json 所在的实际目录
    let actual_dir = find_registry_root(&tmp_dir).await?;

    // 读取 registry.json 获取元数据
    let registry_path = actual_dir.join("registry.json");
    let registry = read_registry(&registry_path).await;

    let templates_count = registry.templates.len();
    let skills_count = registry.skills.len();
    let version = registry.version.clone();

    // 原子替换：先备份旧 hub，再替换
    let backup_dir = hub_dir.with_extension("bak");
    if hub_dir.exists() {
        if backup_dir.exists() {
            let _ = tokio::fs::remove_dir_all(&backup_dir).await;
        }
        tokio::fs::rename(hub_dir, &backup_dir).await?;
    }

    // 将解压目录移动到 hub_dir
    tokio::fs::rename(&actual_dir, hub_dir).await?;

    // 清理临时目录
    let _ = tokio::fs::remove_dir_all(&tmp_dir).await;
    // 清理备份（成功后删除）
    if backup_dir.exists() {
        let _ = tokio::fs::remove_dir_all(&backup_dir).await;
    }

    tracing::info!(
        templates = templates_count,
        skills = skills_count,
        version = version.as_deref().unwrap_or("unknown"),
        hub_dir = %hub_dir.display(),
        "hub 同步完成"
    );

    Ok(SyncResult {
        version,
        templates: templates_count,
        skills: skills_count,
        updated: true,
        synced_at: Utc::now(),
    })
}

/// 检查是否需要同步（hub 不存在或超过 sync_interval_hours）
pub fn needs_sync(
    hub_dir: &PathBuf,
    sync_config: &crate::huanxing::config::HubSyncConfig,
) -> bool {
    let registry_path = hub_dir.join("registry.json");
    if !registry_path.exists() {
        return true;
    }

    let Ok(meta) = std::fs::metadata(&registry_path) else {
        return true;
    };
    let Ok(mtime) = meta.modified() else {
        return true;
    };
    let Ok(elapsed) = mtime.elapsed() else {
        return false;
    };

    elapsed.as_secs() > sync_config.sync_interval_hours * 3600
}

/// 启动时检查并自动同步（异步后台任务）
pub fn auto_sync_on_startup(hub_dir: PathBuf, sync_config: crate::huanxing::config::HubSyncConfig) {
    if !sync_config.auto_sync_on_startup {
        return;
    }
    if !needs_sync(&hub_dir, &sync_config) {
        tracing::debug!(hub_dir = %hub_dir.display(), "hub 无需同步");
        return;
    }

    tokio::spawn(async move {
        tracing::info!(hub_dir = %hub_dir.display(), "后台自动同步 hub...");
        match run_sync(&hub_dir, &sync_config).await {
            Ok(r) => tracing::info!(
                version = r.version.as_deref().unwrap_or("?"),
                templates = r.templates,
                "hub 自动同步完成"
            ),
            Err(e) => tracing::warn!("hub 自动同步失败（非致命）: {e}"),
        }
    });
}

// ── 辅助函数 ──────────────────────────────────────────────

/// 解析 hub 目录路径
fn resolve_hub_dir(config: &crate::config::Config) -> PathBuf {
    config
        .huanxing
        .resolve_hub_dir()
        .unwrap_or_else(|| config.workspace_dir.join("hub"))
}

/// 从 registry.json 读取模板列表
async fn read_templates_from_registry(registry_path: &std::path::Path) -> Vec<HubTemplate> {
    read_registry(registry_path).await.templates
}

/// 读取并解析 registry.json
async fn read_registry(registry_path: &std::path::Path) -> RegistryJson {
    let Ok(content) = tokio::fs::read_to_string(registry_path).await else {
        return RegistryJson::default();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

/// 在解压目录中找到包含 registry.json 的根目录
async fn find_registry_root(base: &std::path::Path) -> anyhow::Result<PathBuf> {
    // 先检查 base 本身
    if base.join("registry.json").exists() {
        return Ok(base.to_path_buf());
    }

    // 检查第一级子目录
    if let Ok(mut entries) = tokio::fs::read_dir(base).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.is_dir() && path.join("registry.json").exists() {
                return Ok(path);
            }
        }
    }

    // 即使没有 registry.json，返回 base 本身（允许空 hub）
    Ok(base.to_path_buf())
}

/// 解压 tar.gz 到目标目录（同步，在 spawn_blocking 中调用）
fn extract_tarball(bytes: &[u8], dest: &std::path::Path) -> anyhow::Result<()> {
    use flate2::read::GzDecoder;
    use tar::Archive;

    let gz = GzDecoder::new(bytes);
    let mut archive = Archive::new(gz);

    archive.unpack(dest)?;
    Ok(())
}
