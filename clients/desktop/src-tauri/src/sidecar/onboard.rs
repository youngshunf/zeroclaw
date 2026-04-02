use tauri::AppHandle;
use crate::sidecar::constants::HUANXING_PORT;
use crate::sidecar::manager::SidecarManager;
use crate::sidecar::models::{OnboardRequest, OnboardResult};

impl SidecarManager {
    /// 执行 onboard 流程：
    /// 1. 创建 ~/.huanxing/ 目录结构
    /// 2. 从模板生成 config.toml（全局级，包含渠道/工具/TTS/HASN 等进程级配置）
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

        // 2. 生成 config.toml（使用 agent-factory 嵌入的完整模板）
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
        let llm_gateway_base = llm_gateway.trim_end_matches("/v1");
        
        let factory = huanxing_agent_factory::AgentFactory::new(
            self.config_dir.clone(),
            None,
        );
        let vars = huanxing_agent_factory::GlobalConfigVars {
            display_name: star_name.to_string(),
            default_provider: format!("custom:{llm_gateway_base}/v1"),
            default_model: "MiniMax-M2.7".to_string(),
            title_model: "MiniMax-M2.5".to_string(),
            gateway_port: HUANXING_PORT,
            llm_gateway: llm_gateway.clone(),
            api_base_url: api_base.to_string(),
            agent_key: agent_key.to_string(),
            user_uuid: user_uuid.to_string(),
            hasn_api_key: hasn_api_key.to_string(),
        };
        let config_content = factory.generate_global_config(&vars);

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
