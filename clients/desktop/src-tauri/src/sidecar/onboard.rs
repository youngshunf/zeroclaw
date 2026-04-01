use tauri::AppHandle;
use tauri::Manager;
use crate::sidecar::constants::HUANXING_PORT;
use crate::sidecar::manager::SidecarManager;
use crate::sidecar::models::{OnboardRequest, OnboardResult};

impl SidecarManager {
    /// 执行 onboard 流程：
    /// 1. 创建 ~/.huanxing/ 目录结构
    /// 2. 从模板生成 config.toml
    /// 3. 创建默认 agent 配置
    /// 4. 创建完整 workspace（从 workspace-scaffold/ 复制 + 占位符替换）
    /// 5. 生成 secret key
    /// 6. 启动 sidecar
    pub async fn onboard(
        &self,
        req: OnboardRequest,
        app: AppHandle,
    ) -> Result<OnboardResult, String> {
        let mut result = OnboardResult {
            success: false,
            config_created: false,
            agent_created: false,
            sidecar_started: false,
            error: None,
        };

        let star_name = req.user_nickname.as_deref().unwrap_or("小星");
        let nickname = req.user_nickname.as_deref().unwrap_or("主人");
        let user_uuid = req.user_uuid.as_deref().unwrap_or("unknown");
        let user_phone = req.user_phone.as_deref().unwrap_or("（未提供）");
        let agent_key = req.agent_key.as_deref().unwrap_or("");

        // 1. 创建目录结构
        std::fs::create_dir_all(&self.config_dir).map_err(|e| format!("创建配置目录失败: {e}"))?;
        let user_tenant_dir = self.config_dir.join("users").join("default");
        let default_agent_dir = user_tenant_dir.join("agents").join("default");
        let workspace_dir = default_agent_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).ok();

        // 2. 生成 config.toml
        let api_base = req
            .api_base_url
            .as_deref()
            .unwrap_or("http://127.0.0.1:8020");
        let llm_gateway = req
            .llm_gateway_url
            .as_deref()
            .unwrap_or_else(|| "")
            .to_string();
        let llm_gateway = if llm_gateway.is_empty() {
            format!("{api_base}/api/v1/llm/proxy/v1")
        } else {
            llm_gateway
        };

        let config_content = generate_config_toml(
            &app,
            &req.llm_token,
            &llm_gateway,
            api_base,
            star_name,
            agent_key,
            user_uuid,
            HUANXING_PORT,
        );

        let config_path = self.config_dir.join("config.toml");
        std::fs::write(&config_path, &config_content)
            .map_err(|e| format!("写入配置文件失败: {e}"))?;
        result.config_created = true;
        tracing::info!("Config created: {}", config_path.display());

        // 3. 创建默认 agent 配置
        let agent_dir = self.config_dir.join("users").join("default").join("agents").join("default").join("workspace");
        std::fs::create_dir_all(&agent_dir).ok();
        let (default_model, title_model) = extract_models_from_template(&app);
        let fallback_agent_config = load_scaffold_file(&app, "agent-scaffold/config.toml.template")
            .unwrap_or_else(|| {
                tracing::warn!("agent-scaffold/config.toml.template not found! Using empty fallback.");
                String::new()
            });

        let agent_config = fallback_agent_config
            .replace("{{star_name}}", star_name)
            .replace("{{default_model}}", &default_model)
            .replace("{{title_model}}", &title_model);
        let agent_config_path = agent_dir.join("config.toml");
        if !agent_config_path.exists() {
            std::fs::write(&agent_config_path, &agent_config).ok();
            tracing::info!("Agent config created: {}", agent_config_path.display());
        }
        result.agent_created = true;

        // 4. 创建完整 workspace
        let now = chrono_now_pretty();
        let comm_style = "温暖、自然、简洁。适当使用 emoji（最多 1-2 个），避免机械化措辞。";
        let placeholders: &[(&str, &str)] = &[
            ("{{nickname}}", nickname),
            ("{{star_name}}", star_name),
            ("{{user_id}}", user_uuid),
            ("{{created_at}}", &now),
            ("{{createdAt}}", &now),
            ("{{timestamp}}", &now),
            ("{{phone}}", user_phone),
            ("{{agent_key}}", agent_key),
            ("{{comm_style}}", comm_style),
        ];

        let owner_dir = self.config_dir.join("users").join("default").join("workspace");
        std::fs::create_dir_all(&owner_dir).ok();

        let scaffold_result = scaffold_workspace(&app, &owner_dir, &workspace_dir, placeholders);
        match scaffold_result {
            Ok(count) => {
                tracing::info!(
                    "Workspace scaffolded: {count} files created in {}",
                    workspace_dir.display()
                );
            }
            Err(e) => {
                tracing::warn!("Workspace scaffold partial failure: {e}");
            }
        }



        // 4.6. 初始化 data/users.db
        let data_dir = self.config_dir.join("data");
        std::fs::create_dir_all(&data_dir).ok();
        let users_db_path = data_dir.join("users.db");
        if !users_db_path.exists() {
            match init_users_db(&users_db_path, user_uuid, user_phone, nickname, star_name) {
                Ok(_) => tracing::info!("users.db initialized: {}", users_db_path.display()),
                Err(e) => tracing::warn!("users.db init failed (non-fatal): {e}"),
            }
        } else {
            tracing::info!("users.db already exists, skipping init");
        }

        // 5. 生成 secret key（如果不存在）
        let secret_path = self.config_dir.join(".secret_key");
        if !secret_path.exists() {
            use std::io::Write;
            let key: [u8; 32] = rand_bytes();
            let hex: String = key.iter().map(|b| format!("{b:02x}")).collect();
            if let Ok(mut f) = std::fs::File::create(&secret_path) {
                let _ = f.write_all(hex.as_bytes());
            }
        }

        // 6. 启动 sidecar
        match self.start(app).await {
            Ok(status) => {
                result.sidecar_started = true;
                tracing::info!(
                    "Sidecar started after onboard: PID={:?}, port={}",
                    status.pid,
                    status.port
                );
            }
            Err(e) => {
                tracing::warn!("Sidecar start failed after onboard: {e}");
                result.error = Some(format!("配置已创建，但引擎启动失败: {e}"));
            }
        }

        result.success = true;
        Ok(result)
    }
}

// ── 内部辅助方法 ──

fn load_scaffold_file(app: &AppHandle, filename: &str) -> Option<String> {
    let scaffold_dir = app.path().resource_dir().ok()?.join("workspace-scaffold");

    let scaffold_dir = if scaffold_dir.exists() {
        scaffold_dir
    } else {
        let dev_path =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../workspace-scaffold");
        if dev_path.exists() {
            dev_path
        } else {
            return None;
        }
    };

    std::fs::read_to_string(scaffold_dir.join(filename)).ok()
}

fn scaffold_workspace(
    app: &AppHandle,
    owner_dir: &std::path::Path,
    agent_dir: &std::path::Path,
    placeholders: &[(&str, &str)],
) -> Result<usize, String> {
    let scaffold_base = app
        .path()
        .resource_dir()
        .map_err(|e| format!("获取资源目录失败: {e}"))?
        .join("workspace-scaffold");

    let scaffold_base = if scaffold_base.exists() {
        scaffold_base
    } else {
        let dev_path =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../workspace-scaffold");
        if dev_path.exists() {
            dev_path
        } else {
            return Err(format!(
                "workspace-scaffold 目录不存在: {}",
                scaffold_base.display()
            ));
        }
    };

    let mut count = 0;

    // Helper closure to process a directory
    let mut process_dir = |src_dir: &std::path::Path, dest_dir: &std::path::Path| -> Result<(), String> {
        if !src_dir.exists() {
            return Ok(()); // tolerate missing parts
        }
        let entries = std::fs::read_dir(src_dir).map_err(|e| format!("读取 scaffold 目录失败: {e}"))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("读取目录条目失败: {e}"))?;
            let file_name = entry.file_name().to_string_lossy().to_string();

            if !file_name.ends_with(".md") || file_name == "README.md" {
                continue;
            }

            let dest = dest_dir.join(&file_name);
            if dest.exists() {
                continue;
            }

            let content = std::fs::read_to_string(entry.path())
                .map_err(|e| format!("读取模板 {file_name} 失败: {e}"))?;

            let mut content = content;
            for (placeholder, value) in placeholders {
                content = content.replace(placeholder, value);
            }

            std::fs::write(&dest, &content).map_err(|e| format!("写入 {file_name} 失败: {e}"))?;
            count += 1;
        }
        Ok(())
    };

    // 1. Owner files -> owner_dir
    process_dir(&scaffold_base.join("owner-scaffold"), owner_dir)?;
    
    // 2. Agent files -> agent_dir
    process_dir(&scaffold_base.join("agent-scaffold"), agent_dir)?;

    Ok(count)
}


fn init_users_db(
    db_path: &std::path::Path,
    user_uuid: &str,
    phone: &str,
    nickname: &str,
    star_name: &str,
) -> Result<(), String> {
    use std::io::Write;

    let sql = format!(
        r#"
CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id TEXT NOT NULL UNIQUE,
    phone TEXT,
    nickname TEXT,
    tenant_dir TEXT,
    plan TEXT,
    plan_expires DATETIME,
    status TEXT DEFAULT 'active',
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS agents (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_id TEXT NOT NULL UNIQUE,
    user_id TEXT NOT NULL,
    template TEXT NOT NULL DEFAULT 'finance',
    star_name TEXT,
    hasn_id TEXT UNIQUE,
    status TEXT DEFAULT 'active',
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(user_id)
);

CREATE TABLE IF NOT EXISTS routing (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    channel_type TEXT NOT NULL,
    sender_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(channel_type, sender_id),
    FOREIGN KEY (agent_id) REFERENCES agents(agent_id),
    FOREIGN KEY (user_id) REFERENCES users(user_id)
);

INSERT OR IGNORE INTO users (user_id, phone, nickname, tenant_dir)
VALUES ('{user_uuid}', '{phone}', '{nickname}', 'default');

INSERT OR IGNORE INTO agents (agent_id, user_id, template, star_name)
VALUES ('default', '{user_uuid}', 'default', '{star_name}');
"#,
        user_uuid = user_uuid,
        phone = phone,
        nickname = nickname,
        star_name = star_name,
    );

    let sql_path = db_path.with_extension("init.sql");
    {
        let mut f = std::fs::File::create(&sql_path)
            .map_err(|e| format!("创建 SQL 文件失败: {e}"))?;
        f.write_all(sql.as_bytes())
            .map_err(|e| format!("写入 SQL 文件失败: {e}"))?;
    }

    let output = std::process::Command::new("sqlite3")
        .arg(db_path.to_string_lossy().as_ref())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .arg(&format!(".read {}", sql_path.to_string_lossy()))
        .output();

    std::fs::remove_file(&sql_path).ok();

    match output {
        Ok(out) => {
            if out.status.success() {
                tracing::info!("users.db initialized successfully");
                Ok(())
            } else {
                init_users_db_pipe(db_path, &sql)
            }
        }
        Err(_) => {
            init_users_db_pipe(db_path, &sql)
        }
    }
}

fn init_users_db_pipe(
    db_path: &std::path::Path,
    sql: &str,
) -> Result<(), String> {
    use std::io::Write;

    let mut child = std::process::Command::new("sqlite3")
        .arg(db_path.to_string_lossy().as_ref())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("启动 sqlite3 失败: {e}"))?;

    if let Some(ref mut stdin) = child.stdin {
        stdin.write_all(sql.as_bytes())
            .map_err(|e| format!("写入 SQL 失败: {e}"))?;
    }

    let output = child.wait_with_output()
        .map_err(|e| format!("等待 sqlite3 失败: {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("sqlite3 执行失败: {stderr}"))
    }
}

fn rand_bytes() -> [u8; 32] {
    let mut buf = [0u8; 32];
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id() as u128;
    let mix = seed ^ (pid << 64);
    for (i, b) in buf.iter_mut().enumerate() {
        *b = ((mix >> (i % 16 * 8)) & 0xFF) as u8 ^ (i as u8).wrapping_mul(37);
    }
    buf
}

fn generate_config_toml(
    app: &tauri::AppHandle,
    llm_token: &str,
    llm_gateway: &str,
    api_base: &str,
    agent_name: &str,
    agent_key: &str,
    user_uuid: &str,
    port: u16,
) -> String {
    let template = load_scaffold_file(app, "config.toml.template").unwrap_or_default();

    if template.is_empty() {
        tracing::warn!("config.toml.template not found, using inline fallback");
        return format!(
            r#"# 唤星桌面端配置 — 自动生成（回退模板）
display_name = "{agent_name}"
default_provider = "custom:{llm_gateway_base}/v1"
default_model = "MiniMax-M2.7"
title_model = "MiniMax-M2.5"
default_temperature = 0.7

[memory]
backend = "sqlite"
auto_save = true

[gateway]
port = {port}
host = "127.0.0.1"
require_pairing = false

[huanxing]
enabled = true
api_base_url = "{api_base}"
agent_key = "{agent_key}"
server_id = "desktop-{user_uuid}"

[runtime]
kind = "native"
"#,
            agent_name = agent_name,
            llm_gateway_base = llm_gateway.trim_end_matches("/v1"),
            api_base = api_base,
            agent_key = agent_key,
            user_uuid = user_uuid,
            port = port,
        );
    }

    let llm_gateway_base = llm_gateway.trim_end_matches("/v1");
    template
        .replace("{{timestamp}}", &chrono_now())
        .replace("{{star_name}}", agent_name)
        .replace("{{llm_token}}", llm_token)
        .replace("{{llm_gateway}}", llm_gateway)
        .replace("{{llm_gateway_base}}", llm_gateway_base)
        .replace("{{api_base}}", api_base)
        .replace("{{agent_key}}", agent_key)
        .replace("{{user_id}}", user_uuid)
        .replace("{{port}}", &port.to_string())
}

fn extract_models_from_template(app: &AppHandle) -> (String, String) {
    let default_model = "MiniMax-M2.7".to_string();
    let title_model = "MiniMax-M2.5".to_string();

    let template = match load_scaffold_file(app, "config.toml.template") {
        Some(t) => t,
        None => return (default_model, title_model),
    };

    let mut dm = default_model;
    let mut tm = title_model;
    for line in template.lines() {
        let line = line.trim();
        if line.starts_with("default_model") {
            if let Some(val) = extract_toml_string_value(line) {
                dm = val;
            }
        } else if line.starts_with("title_model") {
            if let Some(val) = extract_toml_string_value(line) {
                tm = val;
            }
        }
    }
    (dm, tm)
}

fn extract_toml_string_value(line: &str) -> Option<String> {
    if let Some((_, val_str)) = line.split_once('=') {
        let val_str = val_str.trim();
        if val_str.starts_with('"') && val_str.ends_with('"') && val_str.len() >= 2 {
            let inner = &val_str[1..val_str.len() - 1];
            return Some(inner.to_string());
        }
    }
    None
}

fn chrono_now() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn chrono_now_pretty() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}
