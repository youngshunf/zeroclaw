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

        // 1. 创建全局配置目录结构
        std::fs::create_dir_all(&self.config_dir).map_err(|e| format!("创建配置目录失败: {e}"))?;

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

        let hasn_api_key = req.hasn_api_key.as_deref().unwrap_or("");
        let config_content = generate_config_toml(
            &app,
            &req.llm_token,
            &llm_gateway,
            api_base,
            star_name,
            agent_key,
            user_uuid,
            hasn_api_key,
            HUANXING_PORT,
        );

        let config_path = self.config_dir.join("config.toml");
        std::fs::write(&config_path, &config_content)
            .map_err(|e| format!("写入配置文件失败: {e}"))?;
        result.config_created = true;
        tracing::info!("Config created: {}", config_path.display());

        // 3. 执行 CLI agent-create 建立工作区与初始化 DB
        let bin = self.find_binary().map_err(|e| format!("找不到 sidecar 二进制: {e}"))?;
        
        let mut create_cmd = tokio::process::Command::new(&bin);
        create_cmd
            .arg("--config-dir")
            .arg(self.config_dir.to_string_lossy().as_ref())
            .arg("agent-create")
            .arg(user_phone) // phone as tenant_id
            .arg("default")  // agent_name
            .arg("assistant") // template
            .arg("--is-desktop")
            .arg("--display-name")
            .arg(star_name)
            .arg("--user-nickname")
            .arg(nickname);
            
        if !hasn_api_key.is_empty() {
            // 在这使用 hasn_api_key 或 user_uuid 派生绑定的 hasn_id
            let derived_hasn_id = format!("desktop_{user_uuid}");
            create_cmd.arg("--hasn-id").arg(&derived_hasn_id);
        }

        create_cmd.env("ZEROCLAW_BUILD_VERSION", "huanxing-desktop");

        tracing::info!("Running zeroClaw agent-create: {:?}", create_cmd);
        match create_cmd.output().await {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                if !stdout.is_empty() {
                    tracing::info!("agent-create stdout: {stdout}");
                }
                if !output.status.success() {
                    let msg = format!("agent-create 失败 (exit={}): {}", output.status, stderr);
                    tracing::error!("{msg}");
                    result.error = Some(msg);
                    // Don't set agent_created = true on failure
                } else {
                    tracing::info!("agent-create finished successfully.");
                    if !stderr.is_empty() {
                        tracing::debug!("agent-create stderr (success): {stderr}");
                    }
                    result.agent_created = true;
                }
            }
            Err(e) => {
                let msg = format!("无法执行 agent-create: {e}");
                tracing::error!("{msg}");
                result.error = Some(msg);
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


fn generate_config_toml(
    _app: &tauri::AppHandle,
    _llm_token: &str,
    llm_gateway: &str,
    api_base: &str,
    agent_name: &str,
    agent_key: &str,
    user_uuid: &str,
    hasn_api_key: &str,
    port: u16,
) -> String {
    tracing::info!("Generating inline fallback config.toml for desktop");
    let llm_gateway_base = llm_gateway.trim_end_matches("/v1");
    format!(
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

[huanxing.hasn]
enabled = true
auto_connect = true
api_key = "{hasn_api_key}"

[runtime]
kind = "native"
"#,
        )
}

#[allow(dead_code)]
fn extract_models_from_template(_app: &AppHandle) -> (String, String) {
    let default_model = "MiniMax-M2.7".to_string();
    let title_model = "MiniMax-M2.5".to_string();

    (default_model, title_model)
}

#[allow(dead_code)]
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

#[allow(dead_code)]
fn chrono_now_pretty() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}
