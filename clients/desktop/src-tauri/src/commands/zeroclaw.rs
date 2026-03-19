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
