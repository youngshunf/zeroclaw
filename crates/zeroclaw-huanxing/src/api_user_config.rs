//! 用户级配置 HTTP API。
//!
//! 提供用户级配置的读写接口，配置保存到 `users/{tenant_dir}/config.toml`，
//! 而非全局 `config.toml`。遵循三级配置级联规则（Global → User → Agent）。
//!
//! # 端点
//!
//! ```text
//! GET  /api/user-config  → 返回合并后的配置（全局 + 用户级覆盖）
//! PUT  /api/user-config  → 仅保存到用户级 config.toml
//! ```

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, put},
};
use serde::Deserialize;

use zeroclaw_gateway::AppState;

/// 返回用户级配置路由集合。
pub fn user_config_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/user-config",
            get(handle_user_config_get).put(handle_user_config_put),
        )
        .route(
            "/api/huanxing/user/hasn_id",
            put(handle_user_hasn_id_put),
        )
}

#[derive(Debug, Deserialize)]
struct UpdateUserHasnIdRequest {
    hasn_id: String,
}

/// PUT /api/huanxing/user/hasn_id
///
/// 桌面端完成云端 HASN 注册后调用；把云端返回的 `hasn_id` 落进本地
/// `users.db.users.hasn_id` 列。幂等：相同 hasn_id 多次写入均返回 200。
///
/// tenant_dir 解析顺序：`x-tenant-dir` header → `users.db` 第一条 tenant_dir
/// （单用户桌面端场景的自然默认）。
async fn handle_user_hasn_id_put(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<UpdateUserHasnIdRequest>,
) -> impl IntoResponse {
    if let Err(e) = zeroclaw_gateway::api::require_auth(&state, &headers) {
        return e.into_response();
    }

    let hasn_id = req.hasn_id.trim();
    if hasn_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "hasn_id 不能为空"})),
        )
            .into_response();
    }

    let config = state.config.lock().clone();
    let config_dir = config
        .config_path
        .parent()
        .unwrap_or(&config.workspace_dir)
        .to_path_buf();

    let tenant_dir = match crate::api_agents::extract_tenant_dir(
        &headers,
        &config_dir,
        &config.huanxing,
    )
    .await
    .filter(|t| !t.trim().is_empty())
    {
        Some(t) => t,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "未解析到 tenant_dir（缺 x-tenant-dir header 且 users.db 为空）"
                })),
            )
                .into_response();
        }
    };

    let db_path = config.huanxing.resolve_db_path(&config_dir);
    let db = match crate::db::TenantDb::open(&db_path) {
        Ok(db) => db,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("打开 users.db 失败: {e}")})),
            )
                .into_response();
        }
    };

    match db
        .update_user_hasn_id_by_tenant_dir(&tenant_dir, hasn_id)
        .await
    {
        Ok(true) => {
            tracing::info!(
                tenant_dir = %tenant_dir,
                hasn_id,
                "User hasn_id persisted to users.db"
            );
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "status": "ok",
                    "tenant_dir": tenant_dir,
                    "hasn_id": hasn_id,
                })),
            )
                .into_response()
        }
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": format!("tenant_dir={tenant_dir} 在 users.db 中无匹配行")
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("更新 users.db 失败: {e}")})),
        )
            .into_response(),
    }
}

/// GET /api/user-config — 返回全局 + 用户级合并后的配置（TOML 格式）
async fn handle_user_config_get(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(e) = zeroclaw_gateway::api::require_auth(&state, &headers) {
        return e.into_response();
    }

    let config = state.config.lock().clone();
    let config_dir = config
        .config_path
        .parent()
        .unwrap_or(&config.workspace_dir);

    let tenant_dir =
        crate::api_agents::extract_tenant_dir(&headers, config_dir, &config.huanxing)
            .await;

    // 1. 序列化全局配置为 TOML Value
    let global_toml: toml::Value = match toml::to_string_pretty(&config) {
        Ok(s) => match s.parse::<toml::Value>() {
            Ok(v) => v,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": format!("序列化全局配置失败: {e}")})),
                )
                    .into_response();
            }
        },
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("序列化全局配置失败: {e}")})),
            )
                .into_response();
        }
    };

    // 2. 读取用户级配置（如果存在）
    let user_config_path = match &tenant_dir {
        Some(td) => config
            .huanxing
            .resolve_tenant_root(config_dir, Some(td))
            .join("config.toml"),
        None => {
            // 无 tenant_dir 时直接返回全局配置
            return return_masked_toml(&global_toml);
        }
    };

    let user_toml: Option<toml::Value> = match tokio::fs::read_to_string(&user_config_path).await {
        Ok(content) => match content.parse::<toml::Value>() {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::warn!(
                    path = %user_config_path.display(),
                    "用户级配置解析失败，忽略: {e}"
                );
                None
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            tracing::warn!(
                path = %user_config_path.display(),
                "读取用户级配置失败: {e}"
            );
            None
        }
    };

    // 3. 合并：用户级覆盖全局
    let merged = match user_toml {
        Some(user) => merge_toml_values(global_toml, user),
        None => global_toml,
    };

    return_masked_toml(&merged)
}

/// PUT /api/user-config — 将配置保存到用户级 config.toml
async fn handle_user_config_put(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> impl IntoResponse {
    if let Err(e) = zeroclaw_gateway::api::require_auth(&state, &headers) {
        return e.into_response();
    }

    // 解析提交的 TOML
    let incoming: toml::Value = match body.parse::<toml::Value>() {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("无效 TOML: {e}")})),
            )
                .into_response();
        }
    };

    let config = state.config.lock().clone();
    let config_dir = config
        .config_path
        .parent()
        .unwrap_or(&config.workspace_dir);

    let tenant_dir =
        crate::api_agents::extract_tenant_dir(&headers, config_dir, &config.huanxing)
            .await;

    let tenant_dir = match tenant_dir {
        Some(td) if !td.trim().is_empty() => td,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "未解析到 tenant_dir，无法保存用户级配置"})),
            )
                .into_response();
        }
    };

    let user_config_path = config
        .huanxing
        .resolve_tenant_root(config_dir, Some(&tenant_dir))
        .join("config.toml");

    // 1. 序列化全局配置为 TOML Value（用于 diff 比较）
    let global_toml: toml::Value = match toml::to_string_pretty(&config) {
        Ok(s) => match s.parse::<toml::Value>() {
            Ok(v) => v,
            Err(_) => toml::Value::Table(toml::map::Map::new()),
        },
        Err(_) => toml::Value::Table(toml::map::Map::new()),
    };

    // 2. 读取已有的用户级配置（用于保留 MASKED 密钥的原值）
    let existing_user_toml: toml::Value =
        match tokio::fs::read_to_string(&user_config_path).await {
            Ok(content) => content
                .parse::<toml::Value>()
                .unwrap_or_else(|_| toml::Value::Table(toml::map::Map::new())),
            Err(_) => toml::Value::Table(toml::map::Map::new()),
        };

    // 3. 处理 MASKED 值：从已有用户配置或全局配置恢复
    let incoming = restore_masked_values(incoming, &existing_user_toml, &global_toml);

    // 4. 计算 diff：只保留与全局配置不同的字段
    let diff = diff_toml_values(&incoming, &global_toml);

    // 5. 写入用户级 config.toml
    let toml_str = match diff {
        Some(diff_value) => {
            match toml::to_string_pretty(&diff_value) {
                Ok(s) => s,
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": format!("序列化配置失败: {e}")})),
                    )
                        .into_response();
                }
            }
        }
        None => {
            // diff 为空，说明用户配置与全局完全相同，清空用户配置文件
            String::new()
        }
    };

    // 确保目录存在
    if let Some(parent) = user_config_path.parent() {
        if let Err(e) = tokio::fs::create_dir_all(parent).await {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("创建目录失败: {e}")})),
            )
                .into_response();
        }
    }

    // 添加文件头注释
    let content = if toml_str.is_empty() {
        "# 用户级配置（与全局配置相同，无覆盖项）\n".to_string()
    } else {
        format!(
            "# 用户级配置 — 仅包含与全局配置不同的覆盖项\n\
             # 全局配置: {}/config.toml\n\
             # 修改时间: {}\n\n{}",
            config_dir.display(),
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            toml_str
        )
    };

    if let Err(e) = tokio::fs::write(&user_config_path, &content).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("写入用户配置失败: {e}")})),
        )
            .into_response();
    }

    tracing::info!(
        path = %user_config_path.display(),
        tenant = %tenant_dir,
        "用户级配置已保存"
    );

    // 6. 同时更新内存中的全局运行时配置（合并用户覆盖到运行时）
    // 这样当前进程后续使用的配置也是最新的
    let merged_config_str = match toml::to_string_pretty(&incoming) {
        Ok(s) => s,
        Err(_) => {
            return Json(serde_json::json!({"status": "ok", "path": user_config_path.to_string_lossy()}))
                .into_response();
        }
    };
    if let Ok(merged_config) = toml::from_str::<zeroclaw_config::schema::Config>(&merged_config_str) {
        let mut runtime_config = merged_config;
        // 保留运行时不可覆盖的字段
        let current = state.config.lock().clone();
        runtime_config.config_path = current.config_path.clone();
        runtime_config.workspace_dir = current.workspace_dir.clone();
        *state.config.lock() = runtime_config;
    }

    Json(serde_json::json!({
        "status": "ok",
        "path": user_config_path.to_string_lossy(),
    }))
    .into_response()
}

// ── TOML 合并/Diff 工具函数 ──────────────────────────────────────

const MASKED_SECRET: &str = "***MASKED***";

/// 递归合并两个 TOML Value（base + overlay）。
/// overlay 的字段覆盖 base。
fn merge_toml_values(base: toml::Value, overlay: toml::Value) -> toml::Value {
    match (base, overlay) {
        (toml::Value::Table(mut base_map), toml::Value::Table(overlay_map)) => {
            for (key, overlay_val) in overlay_map {
                let merged = match base_map.remove(&key) {
                    Some(base_val) => merge_toml_values(base_val, overlay_val),
                    None => overlay_val,
                };
                base_map.insert(key, merged);
            }
            toml::Value::Table(base_map)
        }
        // 非 table 类型直接用 overlay 覆盖
        (_base, overlay) => overlay,
    }
}

/// 计算 incoming 与 global 之间的差异。
/// 返回仅包含差异字段的 TOML Value，如果完全相同则返回 None。
fn diff_toml_values(incoming: &toml::Value, global: &toml::Value) -> Option<toml::Value> {
    match (incoming, global) {
        (toml::Value::Table(inc_map), toml::Value::Table(glob_map)) => {
            let mut diff_map = toml::map::Map::new();

            for (key, inc_val) in inc_map {
                match glob_map.get(key) {
                    Some(glob_val) => {
                        // 递归比较嵌套 table
                        if let Some(sub_diff) = diff_toml_values(inc_val, glob_val) {
                            diff_map.insert(key.clone(), sub_diff);
                        }
                        // 如果 sub_diff 为 None，说明该子树完全一致，跳过
                    }
                    None => {
                        // 全局没有此字段，这是用户新增的
                        diff_map.insert(key.clone(), inc_val.clone());
                    }
                }
            }

            if diff_map.is_empty() {
                None
            } else {
                Some(toml::Value::Table(diff_map))
            }
        }
        // 叶子节点：值不同才保留
        (inc, glob) if inc != glob => Some(inc.clone()),
        _ => None,
    }
}

/// 恢复 MASKED 值：如果 incoming 中的字段是 MASKED，
/// 优先从已有用户配置中恢复，其次从全局配置中恢复。
fn restore_masked_values(
    incoming: toml::Value,
    existing_user: &toml::Value,
    global: &toml::Value,
) -> toml::Value {
    match incoming {
        toml::Value::Table(mut inc_map) => {
            let user_table = match existing_user {
                toml::Value::Table(t) => Some(t),
                _ => None,
            };
            let global_table = match global {
                toml::Value::Table(t) => Some(t),
                _ => None,
            };

            for (key, val) in inc_map.iter_mut() {
                match val {
                    toml::Value::String(s) if s == MASKED_SECRET => {
                        // 优先从用户配置恢复，其次从全局配置恢复
                        if let Some(user_val) = user_table.and_then(|t| t.get(key)) {
                            *val = user_val.clone();
                        } else if let Some(glob_val) = global_table.and_then(|t| t.get(key)) {
                            *val = glob_val.clone();
                        }
                    }
                    toml::Value::Table(_) => {
                        let user_sub = user_table
                            .and_then(|t| t.get(key))
                            .cloned()
                            .unwrap_or_else(|| toml::Value::Table(toml::map::Map::new()));
                        let global_sub = global_table
                            .and_then(|t| t.get(key))
                            .cloned()
                            .unwrap_or_else(|| toml::Value::Table(toml::map::Map::new()));
                        *val = restore_masked_values(val.clone(), &user_sub, &global_sub);
                    }
                    _ => {}
                }
            }

            toml::Value::Table(inc_map)
        }
        other => other,
    }
}

/// 将 TOML Value 中的敏感字段 mask 后返回。
/// 使用简单的关键字匹配来识别敏感字段。
fn return_masked_toml(value: &toml::Value) -> axum::response::Response {
    let masked = mask_toml_secrets(value.clone());
    let toml_str = match toml::to_string_pretty(&masked) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("序列化配置失败: {e}")})),
            )
                .into_response();
        }
    };

    Json(serde_json::json!({
        "format": "toml",
        "content": toml_str,
    }))
    .into_response()
}

/// 敏感字段关键字列表
const SENSITIVE_KEYS: &[&str] = &[
    "api_key",
    "api_keys",
    "bot_token",
    "app_token",
    "app_secret",
    "access_token",
    "client_secret",
    "private_key",
    "encrypt_key",
    "verification_token",
    "signing_secret",
    "webhook_secret",
    "webhook_key",
    "server_password",
    "nickserv_password",
    "sasl_password",
    "password",
    "oauth_token",
    "recovery_key",
    "verify_token",
    "bearer_token",
    "paired_tokens",
    "auth_token",
    "db_url",
    "clawhub_token",
    "token",
    "agent_key",
    "owner_key",
    "node_key",
];

/// 递归 mask TOML Value 中的敏感字段。
fn mask_toml_secrets(value: toml::Value) -> toml::Value {
    match value {
        toml::Value::Table(mut map) => {
            for (key, val) in map.iter_mut() {
                let key_lower = key.to_lowercase();
                let is_sensitive = SENSITIVE_KEYS.iter().any(|k| key_lower == *k);

                if is_sensitive {
                    match val {
                        toml::Value::String(s) if !s.is_empty() => {
                            *s = MASKED_SECRET.to_string();
                        }
                        toml::Value::Array(arr) => {
                            for item in arr.iter_mut() {
                                if let toml::Value::String(s) = item {
                                    if !s.is_empty() {
                                        *s = MASKED_SECRET.to_string();
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                } else {
                    *val = mask_toml_secrets(val.clone());
                }
            }
            toml::Value::Table(map)
        }
        toml::Value::Array(arr) => {
            toml::Value::Array(arr.into_iter().map(mask_toml_secrets).collect())
        }
        other => other,
    }
}
