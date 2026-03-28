//! ZeroClaw Sidecar 管理 — 启动/停止/重启/状态/日志/配置/Onboard

use crate::sidecar::{OnboardRequest, OnboardResult, QuickConfig, SidecarManager, SidecarStatus};
use std::sync::Arc;
use tauri::{AppHandle, State};

/// 启动 sidecar
#[tauri::command]
pub async fn start_zeroclaw(
    state: State<'_, Arc<SidecarManager>>,
    app: AppHandle,
) -> Result<SidecarStatus, String> {
    state.start(app).await
}

/// 停止 sidecar
#[tauri::command]
pub async fn stop_zeroclaw(
    state: State<'_, Arc<SidecarManager>>,
    app: AppHandle,
) -> Result<(), String> {
    state.stop(&app).await
}

/// 重启 sidecar
#[tauri::command]
pub async fn restart_zeroclaw(
    state: State<'_, Arc<SidecarManager>>,
    app: AppHandle,
) -> Result<SidecarStatus, String> {
    state.stop(&app).await?;
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    state.start(app).await
}

/// 获取 sidecar 状态
#[tauri::command]
pub async fn get_zeroclaw_status(
    state: State<'_, Arc<SidecarManager>>,
) -> Result<SidecarStatus, String> {
    Ok(state.status().await)
}

/// 获取 sidecar 日志
#[tauri::command]
pub async fn get_zeroclaw_logs(
    state: State<'_, Arc<SidecarManager>>,
    lines: Option<usize>,
) -> Result<Vec<String>, String> {
    Ok(state.logs(lines.unwrap_or(100)).await)
}

/// 读取快捷配置
#[tauri::command]
pub async fn get_zeroclaw_config(
    state: State<'_, Arc<SidecarManager>>,
) -> Result<QuickConfig, String> {
    state.read_config()
}

/// 更新快捷配置并重启
#[tauri::command]
pub async fn update_zeroclaw_config(
    state: State<'_, Arc<SidecarManager>>,
    app: AppHandle,
    config: QuickConfig,
) -> Result<SidecarStatus, String> {
    state.update_config(config)?;

    let current = state.status().await;
    if current.running {
        state.stop(&app).await?;
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        state.start(app).await
    } else {
        Ok(current)
    }
}

/// Onboard：登录成功后，生成配置 + 创建默认 agent + 启动 sidecar
#[tauri::command]
pub async fn onboard_zeroclaw(
    state: State<'_, Arc<SidecarManager>>,
    app: AppHandle,
    request: OnboardRequest,
) -> Result<OnboardResult, String> {
    state.onboard(request, app).await
}

/// 配置检查结果
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConfigCheckResult {
    /// config.toml 文件是否存在
    pub config_exists: bool,
    /// config.toml 存在且包含有效的 [huanxing] enabled = true
    pub config_valid: bool,
}

/// 检查唤星配置状态
/// - config_valid=true: 配置完好，可自动修复 sidecar
/// - config_exists=true, config_valid=false: 配置损坏，需重新登录
/// - config_exists=false: 目录被删，需重新登录
#[tauri::command]
pub async fn check_huanxing_config(
    state: State<'_, Arc<SidecarManager>>,
) -> Result<ConfigCheckResult, String> {
    let (exists, valid) = state.check_config_status();
    Ok(ConfigCheckResult {
        config_exists: exists,
        config_valid: valid,
    })
}
