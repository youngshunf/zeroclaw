use rusqlite::Connection;
use serde_json::Value;
use std::fs::{self, File};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use tauri::{command, AppHandle, Emitter};
use toml::Table;

#[derive(Clone, serde::Serialize)]
struct ProgressPayload {
    message: String,
}

fn emit_progress(app: &AppHandle, msg: &str) {
    let _ = app.emit(
        "agent-install-progress",
        ProgressPayload {
            message: msg.to_string(),
        },
    );
}

/// 获取 Marketplace API Base URL
fn read_marketplace_api_base() -> String {
    let config_path = dirs::home_dir()
        .unwrap_or_default()
        .join(".huanxing")
        .join("config.toml");

    if let Ok(content) = fs::read_to_string(&config_path) {
        if let Ok(table) = content.parse::<Table>() {
            if let Some(huanxing) = table.get("huanxing").and_then(|v| v.as_table()) {
                if let Some(url) = huanxing.get("api_base_url").and_then(|v| v.as_str()) {
                    let url = url.trim().trim_end_matches('/');
                    if !url.is_empty() {
                        return url.to_string();
                    }
                }
            }
        }
    }
    // Default fallback
    "http://127.0.0.1:8020".to_string()
}

/// 读取全局配置中的 LLM 设置（用于替换模板占位符）
#[allow(dead_code)]
fn read_global_llm_config() -> (String, f64) {
    let config_path = dirs::home_dir()
        .unwrap_or_default()
        .join(".huanxing")
        .join("config.toml");

    let mut model = "MiniMax-M2.7".to_string();
    let mut temperature = 0.7;

    if let Ok(content) = fs::read_to_string(&config_path) {
        if let Ok(table) = content.parse::<Table>() {
            if let Some(m) = table.get("default_model").and_then(|v| v.as_str()) {
                model = m.to_string();
            }
            if let Some(t) = table.get("default_temperature").and_then(|v| v.as_float()) {
                temperature = t;
            }
        }
    }
    (model, temperature)
}

/// 缓存目录路径
fn get_cache_dir() -> PathBuf {
    let dir = dirs::home_dir()
        .unwrap_or_default()
        .join(".huanxing")
        .join("cache");
    let _ = fs::create_dir_all(&dir);
    dir
}

/// 从 users.db 查询当前活跃的真实 tenant_dir（如 "001-18611348367"）。
/// 桌面端通常只有一个注册用户，取第一条即可。
fn resolve_first_tenant_dir() -> Result<String, String> {
    let db_path = dirs::home_dir()
        .unwrap_or_default()
        .join(".huanxing")
        .join("data")
        .join("users.db");

    let conn = Connection::open(&db_path)
        .map_err(|e| format!("打开 users.db 失败 ({}): {e}", db_path.display()))?;

    conn.query_row(
        "SELECT tenant_dir
         FROM users
         WHERE tenant_dir IS NOT NULL
           AND TRIM(tenant_dir) != ''
         ORDER BY datetime(COALESCE(created_at, '1970-01-01T00:00:00Z')) ASC, rowid ASC
         LIMIT 1",
        [],
        |row| row.get::<_, String>(0),
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            "未在 users.db 中找到可用 tenant_dir，请先登录".to_string()
        }
        other => format!("读取 tenant_dir 失败: {other}"),
    })
}

// ── SQLite 市场缓存 ─────────────────────────────────────────

/// 缓存新鲜度阈值（秒）。1 小时内的缓存视为新鲜。
const CACHE_FRESH_SECS: i64 = 3600;

/// 打开（或创建）市场缓存数据库
fn open_market_cache_db() -> Result<Connection, String> {
    let db_path = get_cache_dir().join("market_cache.db");
    let conn = Connection::open(&db_path)
        .map_err(|e| format!("打开 market_cache.db 失败: {e}"))?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS market_items (
            item_type  TEXT NOT NULL,
            item_id    TEXT NOT NULL,
            data       TEXT NOT NULL,
            sort_order INTEGER DEFAULT 0,
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            PRIMARY KEY (item_type, item_id)
        );
        CREATE TABLE IF NOT EXISTS market_meta (
            item_type  TEXT PRIMARY KEY,
            total      INTEGER DEFAULT 0,
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_items_type_order ON market_items(item_type, sort_order);",
    )
    .map_err(|e| format!("初始化缓存表失败: {e}"))?;

    Ok(conn)
}

/// 查询缓存是否新鲜（距上次更新不超过 max_age 秒）
fn cache_is_fresh(conn: &Connection, item_type: &str, max_age_secs: i64) -> bool {
    conn.query_row(
        "SELECT 1 FROM market_meta
         WHERE item_type = ?1
           AND (julianday('now') - julianday(updated_at)) * 86400 < ?2",
        rusqlite::params![item_type, max_age_secs],
        |_| Ok(true),
    )
    .unwrap_or(false)
}

/// 从缓存读取指定类型的所有条目
fn cache_get_all(conn: &Connection, item_type: &str) -> Vec<Value> {
    let mut stmt = match conn.prepare(
        "SELECT data FROM market_items WHERE item_type = ?1 ORDER BY sort_order ASC",
    ) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    stmt.query_map(rusqlite::params![item_type], |row| {
        let json_str: String = row.get(0)?;
        Ok(serde_json::from_str::<Value>(&json_str).unwrap_or(Value::Null))
    })
    .map(|rows| rows.filter_map(|r| r.ok()).filter(|v| !v.is_null()).collect())
    .unwrap_or_default()
}

/// 获取缓存中的 total 值
fn cache_get_total(conn: &Connection, item_type: &str) -> i64 {
    conn.query_row(
        "SELECT total FROM market_meta WHERE item_type = ?1",
        rusqlite::params![item_type],
        |row| row.get(0),
    )
    .unwrap_or(0)
}

/// 将在线获取的数据写入缓存（全量替换该类型）
fn cache_upsert(conn: &Connection, item_type: &str, items: &[Value], total: i64) {
    let tx = match conn.unchecked_transaction() {
        Ok(t) => t,
        Err(_) => return,
    };

    // 清除旧数据
    let _ = tx.execute(
        "DELETE FROM market_items WHERE item_type = ?1",
        rusqlite::params![item_type],
    );

    // 插入新数据
    for (idx, item) in items.iter().enumerate() {
        let item_id = item
            .get("id")
            .map(|v| match v {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                _ => format!("{item_type}_{idx}"),
            })
            .unwrap_or_else(|| format!("{item_type}_{idx}"));
        let data_str = serde_json::to_string(item).unwrap_or_default();
        let _ = tx.execute(
            "INSERT OR REPLACE INTO market_items (item_type, item_id, data, sort_order, updated_at)
             VALUES (?1, ?2, ?3, ?4, datetime('now'))",
            rusqlite::params![item_type, item_id, data_str, idx as i64],
        );
    }

    // 更新 meta
    let _ = tx.execute(
        "INSERT OR REPLACE INTO market_meta (item_type, total, updated_at)
         VALUES (?1, ?2, datetime('now'))",
        rusqlite::params![item_type, total],
    );

    let _ = tx.commit();
}

/// 从线上 API 获取市场数据
async fn fetch_market_online(item_type: &str) -> Result<(Vec<Value>, i64), String> {
    let api_base = read_marketplace_api_base();
    // 请求足够大的 page_size 以获取所有条目（API 默认分页较小）
    let url = format!(
        "{}/api/v1/marketplace/client/{}?page=1&page_size=200",
        api_base, item_type
    );

    let res = reqwest::get(&url)
        .await
        .map_err(|e| format!("网络请求失败: {e}"))?;

    if !res.status().is_success() {
        return Err(format!("API 响应错误: {}", res.status()));
    }

    let json: Value = res.json().await.map_err(|e| format!("JSON 解析失败: {e}"))?;

    let data = json.get("data").unwrap_or(&json);
    let items = data
        .get("items")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let total = data
        .get("total")
        .and_then(|v| v.as_i64())
        .unwrap_or(items.len() as i64);

    Ok((items, total))
}

/// 通用的"缓存优先、在线兜底"获取逻辑
async fn get_market_data(item_type: &str, api_path: &str) -> Result<Value, String> {
    let conn = open_market_cache_db()?;

    // 1. 缓存新鲜 → 秒返回
    if cache_is_fresh(&conn, item_type, CACHE_FRESH_SECS) {
        let items = cache_get_all(&conn, item_type);
        if !items.is_empty() {
            let total = cache_get_total(&conn, item_type);
            return Ok(serde_json::json!({ "items": items, "total": total }));
        }
    }

    // 2. 在线获取
    match fetch_market_online(api_path).await {
        Ok((items, total)) => {
            cache_upsert(&conn, item_type, &items, total);
            Ok(serde_json::json!({ "items": items, "total": total }))
        }
        Err(online_err) => {
            // 3. 在线失败 → 返回过期缓存（有总比没有好）
            let items = cache_get_all(&conn, item_type);
            if !items.is_empty() {
                let total = cache_get_total(&conn, item_type);
                eprintln!(
                    "[huanxing-desktop] Online fetch failed for {item_type}, using stale cache ({} items): {online_err}",
                    items.len()
                );
                Ok(serde_json::json!({ "items": items, "total": total }))
            } else {
                // 4. 都没有 → 空数据
                eprintln!(
                    "[huanxing-desktop] No cache and online failed for {item_type}: {online_err}"
                );
                Ok(serde_json::json!({ "items": [], "total": 0 }))
            }
        }
    }
}

/// 异步静默同步市场数据（启动时后台预热第一页到 DB）
pub async fn sync_marketplace_data(app_handle: Option<tauri::AppHandle>) {
    let conn = match open_market_cache_db() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[huanxing-desktop] Failed to open market cache DB: {e}");
            return;
        }
    };

    // 预热三类数据到 SQLite
    for (item_type, api_path) in [("app", "apps"), ("skill", "skills"), ("sop", "sops")] {
        match fetch_market_online(api_path).await {
            Ok((items, total)) => {
                cache_upsert(&conn, item_type, &items, total);
                eprintln!(
                    "[huanxing-desktop] Pre-warmed {}s cache: {} items",
                    item_type,
                    items.len()
                );
            }
            Err(e) => {
                eprintln!("[huanxing-desktop] Failed to pre-warm {item_type}s: {e}");
            }
        }
    }

    // 同步 common-skills
    let api_base = read_marketplace_api_base();
    let common_skills_url = format!("{}/api/v1/marketplace/client/common-skills", api_base);
    let skills_dir = dirs::home_dir()
        .unwrap_or_default()
        .join(".huanxing")
        .join("skills");
    let _ = fs::create_dir_all(&skills_dir);

    if let Ok(res) = reqwest::get(&common_skills_url).await {
        if let Ok(json) = res.json::<Value>().await {
            if let Some(data) = json.get("data") {
                if let Some(skills_arr) = data.get("skills").and_then(|v| v.as_array()) {
                    for skill_val in skills_arr {
                        if let Some(skill_id) = skill_val.as_str() {
                            let target_skill_dir = skills_dir.join(skill_id);
                            if !target_skill_dir.exists() {
                                eprintln!(
                                    "[huanxing-desktop] Downloading common skill: {}",
                                    skill_id
                                );
                                if let Ok(info) =
                                    get_download_info(&api_base, "skill", skill_id).await
                                {
                                    if let Some(pkg_url) =
                                        info.get("package_url").and_then(|v| v.as_str())
                                    {
                                        if let Ok(bytes) = download_bytes(pkg_url).await {
                                            if unzip_buffer(&bytes, &target_skill_dir).is_ok() {
                                                eprintln!("[huanxing-desktop] Successfully installed common skill: {}", skill_id);
                                            } else {
                                                let _ = fs::remove_dir_all(&target_skill_dir);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    eprintln!(
        "[huanxing-desktop] Marketplace cache synchronized (apps + skills + sops + common-skills)."
    );

    // 通知前端缓存已就绪
    if let Some(handle) = app_handle {
        let _ = handle.emit("marketplace-synced", serde_json::json!({ "success": true }));
    }
}

#[command]
pub async fn get_market_apps() -> Result<Value, String> {
    get_market_data("app", "apps").await
}

#[command]
pub async fn get_market_skills() -> Result<Value, String> {
    get_market_data("skill", "skills").await
}

#[command]
pub async fn get_market_sops() -> Result<Value, String> {
    get_market_data("sop", "sops").await
}

#[command]
pub async fn force_refresh_market_cache() -> Result<(), String> {
    let conn = open_market_cache_db()?;
    conn.execute_batch("DELETE FROM market_meta; DELETE FROM market_items;")
        .map_err(|e| format!("清除缓存失败: {}", e))?;
    Ok(())
}

/// 辅助：解压工具
fn unzip_buffer(buf: &[u8], target_dir: &Path) -> Result<(), String> {
    let cursor = Cursor::new(buf);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| format!("Zip 解析失败: {}", e))?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
        let outpath = match file.enclosed_name() {
            Some(path) => target_dir.join(path),
            None => continue,
        };

        if file.name().ends_with('/') {
            fs::create_dir_all(&outpath).map_err(|e| e.to_string())?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(p).map_err(|e| e.to_string())?;
                }
            }
            let mut outfile = File::create(&outpath).map_err(|e| e.to_string())?;
            std::io::copy(&mut file, &mut outfile).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// 下载文件到字节数组
async fn download_bytes(url: &str) -> Result<Vec<u8>, String> {
    let response = reqwest::get(url)
        .await
        .map_err(|e| format!("下载失败: {}", e))?;
    if !response.status().is_success() {
        return Err(format!("下载响应错误: {}", response.status()));
    }
    let bytes = response.bytes().await.map_err(|e| e.to_string())?;
    Ok(bytes.to_vec())
}

/// 从市场 API 获取最新版本下载信息
async fn get_download_info(
    api_base: &str,
    item_type: &str,
    item_id: &str,
) -> Result<Value, String> {
    let url = format!(
        "{}/api/v1/marketplace/client/download/{}/{}/latest",
        api_base, item_type, item_id
    );
    let response = reqwest::get(&url)
        .await
        .map_err(|e| format!("获取下载信息失败: {}", e))?;
    let json: Value = response.json().await.map_err(|e| e.to_string())?;

    let code = json.get("code").and_then(|c| c.as_i64());
    if code != Some(0) && code != Some(200) {
        return Err(format!(
            "获取下载信息失败: {}",
            json.get("msg")
                .and_then(|m| m.as_str())
                .unwrap_or("未知错误")
        ));
    }

    json.get("data")
        .cloned()
        .ok_or_else(|| "响应缺少 data 字段".to_string())
}

#[command]
pub async fn download_and_install_agent(
    app: tauri::AppHandle,
    _app_id: String,
    agent_name: String,
    display_name: String,
    package_url: String,
) -> Result<(), String> {
    emit_progress(&app, "初始化安装环境...");
    eprintln!(
        "[huanxing-desktop] Installing Agent '{}' (template: {})",
        agent_name, _app_id
    );

    let api_base = read_marketplace_api_base();

    // ── Step 1: 市场下载包获取 ──
    let final_url = if package_url.is_empty() || package_url.contains(":8000") {
        emit_progress(&app, "正在获取下载地址...");
        let info = get_download_info(&api_base, "app", &_app_id)
            .await
            .map_err(|e| format!("无法获取下载地址: {}", e))?;
        info.get("package_url")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    } else {
        package_url
    };

    if final_url.is_empty() {
        emit_progress(&app, "Error: 无效的下载地址");
        return Err("无法解析有效的 package_url".to_string());
    }

    // ── Step 2: 调用 AgentFactory 安装体系 ──
    let config_dir = dirs::home_dir().unwrap_or_default().join(".huanxing");
    let factory = huanxing_agent_factory::AgentFactory::new(config_dir, Some(api_base));

    // 从 users.db 查询真实 tenant_dir，不再硬编码 "default"
    let tenant_dir = resolve_first_tenant_dir()?;
    eprintln!(
        "[huanxing-desktop] Resolved tenant_dir for marketplace install: {}",
        tenant_dir
    );

    let params = huanxing_agent_factory::CreateAgentParams {
        tenant_id: tenant_dir.clone(),
        template_id: _app_id.clone(),
        agent_name: agent_name.clone(),
        display_name: display_name.clone(),
        is_desktop: true, // 触发 Layer2: _base_desktop 特殊覆盖
        user_nickname: String::new(), // 市场安装不需要用户昵称
        user_phone: String::new(),    // 市场安装不涉及手机号模板替换
        owner_dir: {
            let huanxing_dir = dirs::home_dir().unwrap_or_default().join(".huanxing");
            huanxing_dir
                .join("users")
                .join(&tenant_dir)
                .join("workspace")
                .to_string_lossy()
                .to_string()
        },
        provider: None,
        model: None,
        api_key: None,
        hasn_id: None,
        fallback_provider: None,
        embedding_provider: None,
        llm_gateway: None,
    };

    struct TauriProgress {
        pub app: tauri::AppHandle,
    }

    impl huanxing_agent_factory::ProgressSink for TauriProgress {
        fn on_progress(&self, step: &str, detail: &str) {
            emit_progress(&self.app, &format!("{} - {}", step, detail));
        }
        fn on_error(&self, step: &str, error: &str) {
            emit_progress(&self.app, &format!("⚠️错误: {} ({})", step, error));
        }
    }

    match factory
        .install_from_market(&params, &final_url, &TauriProgress { app: app.clone() })
        .await
    {
        Ok(_) => {
            // ── Register agent in users.db so TenantContext can resolve it ──
            // Without this record, load_by_agent_id() returns None and the
            // runtime falls back to the global config (which has no api_key),
            // causing "API key not set" errors.
            emit_progress(&app, "正在注册 Agent 到本地数据库...");
            let db_path = dirs::home_dir()
                .unwrap_or_default()
                .join(".huanxing")
                .join("data")
                .join("users.db");

            match Connection::open(&db_path) {
                Ok(conn) => {
                    // Resolve user_id from tenant_dir
                    let user_id_result: Result<String, _> = conn.query_row(
                        "SELECT user_id FROM users WHERE tenant_dir = ?1 LIMIT 1",
                        rusqlite::params![&tenant_dir],
                        |row| row.get(0),
                    );

                    match user_id_result {
                        Ok(user_id) => {
                            match conn.execute(
                                "INSERT OR IGNORE INTO agents (agent_id, user_id, template, star_name, status)
                                 VALUES (?1, ?2, ?3, ?4, 'active')",
                                rusqlite::params![&agent_name, &user_id, &_app_id, &display_name],
                            ) {
                                Ok(_) => {
                                    eprintln!(
                                        "[huanxing-desktop] Agent '{}' registered in DB (user={})",
                                        agent_name, user_id
                                    );
                                }
                                Err(e) => {
                                    eprintln!(
                                        "[huanxing-desktop] ⚠ Failed to register agent in DB: {}",
                                        e
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!(
                                "[huanxing-desktop] ⚠ Could not resolve user_id for tenant_dir '{}': {}",
                                tenant_dir, e
                            );
                        }
                    }
                }
                Err(e) => {
                    eprintln!(
                        "[huanxing-desktop] ⚠ Could not open users.db for agent registration: {}",
                        e
                    );
                }
            }

            eprintln!(
                "[huanxing-desktop] Agent '{}' installed successfully",
                agent_name
            );
            emit_progress(&app, "Agent 赋能安装全部完成！");
            Ok(())
        }
        Err(e) => {
            let msg = format!("Agent 安装失败: {}", e);
            eprintln!("[huanxing-desktop] {}", msg);
            Err(msg)
        }
    }
}

#[command]
pub async fn download_and_install_skill(
    app: tauri::AppHandle,
    agent_name: String,
    skill_id: String,
    package_url: String,
    install_scope: Option<String>,
) -> Result<(), String> {
    let scope = install_scope.as_deref().unwrap_or("agent");
    let scope_label = if scope == "user" { "用户公共" } else { "Agent" };

    emit_progress(&app, &format!("准备获取技能 '{}' ({}级)...", skill_id, scope_label));
    eprintln!(
        "[huanxing-desktop] Downloading Skill {} for Agent {} (scope={})",
        skill_id, agent_name, scope
    );

    let api_base = read_marketplace_api_base();

    // Resolve package URL
    let final_url = if package_url.is_empty() || package_url.contains(":8000") {
        let info = get_download_info(&api_base, "skill", &skill_id)
            .await
            .map_err(|e| format!("无法获取 Skill 下载地址: {}", e))?;
        info.get("package_url")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    } else {
        package_url
    };
    if final_url.is_empty() {
        emit_progress(&app, "Error: 无法解析有效的 package_url");
        return Err("无法解析有效的 package_url".to_string());
    }

    emit_progress(&app, "正在下载技能包...");
    // 1. Download
    let response = reqwest::get(&final_url)
        .await
        .map_err(|e| format!("下载失败: {}", e))?;

    let bytes = response.bytes().await.map_err(|e| e.to_string())?;

    // 2. 从 users.db 查询真实 tenant_dir
    let tenant_dir = resolve_first_tenant_dir()?;
    let huanxing_dir = dirs::home_dir().unwrap_or_default().join(".huanxing");

    // 3. Determine target directory based on install scope
    let target_dir = match scope {
        "user" => {
            // 用户级: users/{tenant}/workspace/skills/{skill_id}/
            huanxing_dir
                .join("users")
                .join(&tenant_dir)
                .join("workspace")
                .join("skills")
                .join(&skill_id)
        }
        _ => {
            // Agent 级 (默认): users/{tenant}/agents/{agent}/workspace/skills/{skill_id}/
            huanxing_dir
                .join("users")
                .join(&tenant_dir)
                .join("agents")
                .join(&agent_name)
                .join("workspace")
                .join("skills")
                .join(&skill_id)
        }
    };

    if target_dir.exists() {
        let _ = fs::remove_dir_all(&target_dir); // clean old
    }
    fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;

    // 4. Extract
    emit_progress(&app, "正在安装和解压...");
    unzip_buffer(&bytes, &target_dir)?;

    // 5. Update the agent's config.toml skills list (only for agent-scope installs)
    if scope != "user" {
        emit_progress(&app, "更新 Agent 配置依赖...");
        let config_path = huanxing_dir
            .join("users")
            .join(&tenant_dir)
            .join("agents")
            .join(&agent_name)
            .join("config.toml");

        if config_path.exists() {
            if let Ok(content) = fs::read_to_string(&config_path) {
                if let Ok(mut doc) = content.parse::<toml_edit::DocumentMut>() {
                    let plugins = doc
                        .entry("plugins")
                        .or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
                    if let Some(plugins_table) = plugins.as_table_mut() {
                        let skills = plugins_table
                            .entry("skills")
                            .or_insert(toml_edit::Item::Value(toml_edit::Value::Array(
                                toml_edit::Array::new(),
                            )));

                        if let Some(arr) = skills.as_array_mut() {
                            let mut exists = false;
                            for v in arr.iter() {
                                if let Some(s) = v.as_str() {
                                    if s == skill_id {
                                        exists = true;
                                        break;
                                    }
                                }
                            }
                            if !exists {
                                arr.push(skill_id.clone());
                                fs::write(&config_path, doc.to_string()).ok();
                            }
                        }
                    }
                }
            }
        }
    }

    emit_progress(&app, &format!("✅ 技能安装成功！（{}级别）", scope_label));
    Ok(())
}

#[command]
pub async fn download_and_install_sop(
    app: tauri::AppHandle,
    agent_name: String,
    sop_id: String,
    package_url: String,
) -> Result<(), String> {
    emit_progress(&app, &format!("准备获取 SOP 工作流 '{}' ...", sop_id));

    let api_base = read_marketplace_api_base();

    // Resolve package URL
    let final_url = if package_url.is_empty() || package_url.contains(":8000") {
        let info = get_download_info(&api_base, "sop", &sop_id)
            .await
            .map_err(|e| format!("无法获取 SOP 下载地址: {}", e))?;
        info.get("package_url")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    } else {
        package_url
    };
    if final_url.is_empty() {
        emit_progress(&app, "Error: 无法解析有效的 package_url");
        return Err("无法解析有效的 package_url".to_string());
    }

    emit_progress(&app, "正在下载 SOP 工作流包...");
    let bytes = download_bytes(&final_url).await?;

    emit_progress(&app, "正在初始化安装目录...");

    // 从 users.db 查询真实 tenant_dir
    let tenant_dir = resolve_first_tenant_dir()?;
    let huanxing_dir = dirs::home_dir().unwrap_or_default().join(".huanxing");

    // SOP 安装路径: users/{tenant}/agents/{agent}/workspace/sops/{sop_id}/
    let target_dir = huanxing_dir
        .join("users")
        .join(&tenant_dir)
        .join("agents")
        .join(&agent_name)
        .join("workspace")
        .join("sops")
        .join(&sop_id);

    if target_dir.exists() {
        let _ = fs::remove_dir_all(&target_dir);
    }
    fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;

    emit_progress(&app, "正在解压资产...");
    unzip_buffer(&bytes, &target_dir)?;

    // 解析 SOP.md 中引用的技能，检查是否已安装
    emit_progress(&app, "解析并确认能力依赖 (Requirements)...");
    let sop_md_path = target_dir.join("SOP.md");
    if sop_md_path.exists() {
        if let Ok(md_content) = fs::read_to_string(&sop_md_path) {
            let api_base = read_marketplace_api_base();
            // 技能依赖路径: users/{tenant}/agents/{agent}/workspace/skills/
            let skills_dir = huanxing_dir
                .join("users")
                .join(&tenant_dir)
                .join("agents")
                .join(&agent_name)
                .join("workspace")
                .join("skills");

            // 提取 `- tools: xxx, yyy` 行中的技能
            for line in md_content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("- tools:") {
                    let tools_part = trimmed.trim_start_matches("- tools:").trim();
                    for tool in tools_part.split(',') {
                        let tool = tool.trim();
                        if tool.is_empty() {
                            continue;
                        }
                        // 跳过内置工具
                        if tool.starts_with("memory_")
                            || tool.starts_with("web_")
                            || tool.starts_with("hx_")
                            || tool == "shell"
                            || tool == "file_read"
                            || tool == "file_write"
                            || tool == "delegate"
                        {
                            continue;
                        }
                        // 检查是否已安装
                        if !skills_dir.join(tool).exists() {
                            emit_progress(
                                &app,
                                &format!("缺少能力依赖 '{}'，开始自动安装...", tool),
                            );
                            eprintln!(
                                "[huanxing-desktop]   SOP 依赖技能 '{}' 未安装，尝试自动安装...",
                                tool
                            );
                            match get_download_info(&api_base, "skill", tool).await {
                                Ok(info) => {
                                    if let Some(pkg_url) =
                                        info.get("package_url").and_then(|v| v.as_str())
                                    {
                                        match download_bytes(pkg_url).await {
                                            Ok(skill_bytes) => {
                                                let skill_dir = skills_dir.join(tool);
                                                let _ = fs::create_dir_all(&skill_dir);
                                                if let Err(e) =
                                                    unzip_buffer(&skill_bytes, &skill_dir)
                                                {
                                                    emit_progress(
                                                        &app,
                                                        &format!(
                                                            "Error: 依赖 '{}' 安装失败: {}",
                                                            tool, e
                                                        ),
                                                    );
                                                    eprintln!("[huanxing-desktop]   ⚠ 技能 '{}' 安装失败: {}", tool, e);
                                                } else {
                                                    emit_progress(
                                                        &app,
                                                        &format!("✓ 依赖 '{}' 安装完备", tool),
                                                    );
                                                    eprintln!("[huanxing-desktop]   ✓ 技能 '{}' 自动安装成功", tool);
                                                }
                                            }
                                            Err(e) => eprintln!(
                                                "[huanxing-desktop]   ⚠ 技能 '{}' 下载失败: {}",
                                                tool, e
                                            ),
                                        }
                                    }
                                }
                                Err(_) => eprintln!(
                                    "[huanxing-desktop]   [i] 技能 '{}' 可能是内置工具，跳过",
                                    tool
                                ),
                            }
                        }
                    }
                }
            }
        }
    }

    eprintln!(
        "[huanxing-desktop] SOP '{}' installed for Agent '{}'",
        sop_id, agent_name
    );
    emit_progress(&app, "✅ SOP 工作流安装成功！");
    Ok(())
}
