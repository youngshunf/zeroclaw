use std::path::PathBuf;
use crate::sidecar::manager::SidecarManager;
use crate::sidecar::constants::HEALTH_TIMEOUT;

impl SidecarManager {
    /// 配置目录路径
    pub fn config_dir(&self) -> &PathBuf {
        &self.config_dir
    }

    /// 是否有有效的唤星配置文件（包含 [huanxing] enabled = true）
    pub fn has_valid_huanxing_config(&self) -> bool {
        let config_path = self.config_dir.join("config.toml");
        if !config_path.exists() { return false; }
        let content = std::fs::read_to_string(&config_path).unwrap_or_default();
        // 必须同时有 [huanxing] 段且 enabled = true
        content.contains("[huanxing]") && content.contains("enabled = true")
    }

    /// 是否有任何配置文件（兼容旧检查）
    pub fn has_config(&self) -> bool {
        self.config_dir.join("config.toml").exists()
    }

    /// 检查配置状态：(config_exists, config_valid)
    /// - (true, true): config.toml 存在且有效 → 可自动修复 sidecar
    /// - (true, false): config.toml 存在但无效 → 需重新登录
    /// - (false, false): 目录/文件不存在 → 需重新登录
    pub fn check_config_status(&self) -> (bool, bool) {
        let config_path = self.config_dir.join("config.toml");
        if !config_path.exists() {
            return (false, false);
        }
        let valid = self.has_valid_huanxing_config();
        (true, valid)
    }

    /// 杀掉可能残留的 zeroclaw 孤儿进程（配置已删除但进程还在跑）
    pub async fn kill_orphan_sidecar(&self, port: u16) {
        // 1. 尝试通过 HTTP 优雅关闭
        let client = reqwest::Client::builder()
            .timeout(HEALTH_TIMEOUT)
            .build()
            .unwrap_or_default();

        let health_ok = client
            .get(format!("http://127.0.0.1:{port}/health"))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false);

        if !health_ok {
            eprintln!("[huanxing-desktop] No orphan sidecar found on port {port}");
            return;
        }

        eprintln!("[huanxing-desktop] Found orphan sidecar on port {port}, killing...");

        // 2. 读 PID 文件
        let pid_path = self.config_dir.join(".sidecar.pid");
        if let Ok(pid_str) = std::fs::read_to_string(&pid_path) {
            if let Ok(pid) = pid_str.trim().parse::<i32>() {
                #[cfg(unix)]
                unsafe {
                    libc::kill(pid, libc::SIGTERM);
                }
                eprintln!("[huanxing-desktop] Sent SIGTERM to PID {pid}");
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                // 如果还在运行，强杀
                #[cfg(unix)]
                unsafe {
                    libc::kill(pid, libc::SIGKILL);
                }
                let _ = std::fs::remove_file(&pid_path);
                return;
            }
        }

        // 3. PID 文件不存在，用 pkill 按端口关联杀
        let _ = std::process::Command::new("pkill")
            .args(["-f", &format!("zeroclaw daemon.*--port {port}")])
            .status();
        eprintln!("[huanxing-desktop] Killed orphan sidecar via pkill");
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }

    /// 读取 config.toml 中的快捷配置
    pub fn read_config(&self) -> Result<crate::sidecar::models::QuickConfig, String> {
        let config_path = self.config_dir.join("config.toml");
        let content =
            std::fs::read_to_string(&config_path).map_err(|e| format!("读取配置文件失败: {e}"))?;

        let table: toml::Table = content
            .parse()
            .map_err(|e| format!("解析 TOML 失败: {e}"))?;

        Ok(crate::sidecar::models::QuickConfig {
            default_model: table
                .get("default_model")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            default_temperature: table.get("default_temperature").and_then(|v| v.as_float()),
            autonomy_level: table
                .get("autonomy")
                .and_then(|v| v.as_table())
                .and_then(|t| t.get("level"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            gateway_port: table
                .get("gateway")
                .and_then(|v| v.as_table())
                .and_then(|t| t.get("port"))
                .and_then(|v| v.as_integer())
                .map(|v| v as u16),
        })
    }

    /// 更新 config.toml 中的快捷配置
    pub fn update_config(&self, updates: crate::sidecar::models::QuickConfig) -> Result<(), String> {
        let config_path = self.config_dir.join("config.toml");
        let content =
            std::fs::read_to_string(&config_path).map_err(|e| format!("读取配置文件失败: {e}"))?;

        let mut table: toml::Table = content
            .parse()
            .map_err(|e| format!("解析 TOML 失败: {e}"))?;

        if let Some(model) = updates.default_model {
            table.insert("default_model".to_string(), toml::Value::String(model));
        }
        if let Some(temp) = updates.default_temperature {
            table.insert("default_temperature".to_string(), toml::Value::Float(temp));
        }
        if let Some(level) = updates.autonomy_level {
            if let Some(autonomy) = table
                .entry("autonomy")
                .or_insert_with(|| toml::Value::Table(toml::Table::new()))
                .as_table_mut()
            {
                autonomy.insert("level".to_string(), toml::Value::String(level));
            }
        }

        let new_content =
            toml::to_string_pretty(&table).map_err(|e| format!("序列化 TOML 失败: {e}"))?;

        std::fs::write(&config_path, new_content).map_err(|e| format!("写入配置文件失败: {e}"))?;

        tracing::info!("Config updated: {}", config_path.display());
        Ok(())
    }
}

// ── 内部辅助方法 (从 Manager 提取避免双向依赖导致编译错误) ──

/// 清理 PID 文件
pub(crate) fn cleanup_pid_file_helper(config_dir: &PathBuf) {
    let pid_path = config_dir.join(".sidecar.pid");
    let _ = std::fs::remove_file(&pid_path);
}

/// 通过 PID 文件杀残留进程
pub(crate) fn kill_by_pid_file_helper(config_dir: &PathBuf) {
    let pid_path = config_dir.join(".sidecar.pid");
    if let Ok(content) = std::fs::read_to_string(&pid_path) {
        if let Ok(pid) = content.trim().parse::<i32>() {
            tracing::info!("Killing leftover sidecar process: PID {pid}");
            #[cfg(unix)]
            unsafe {
                libc::kill(pid, libc::SIGTERM);
            }
        }
    }
}
