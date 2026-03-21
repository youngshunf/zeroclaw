//! 认证命令 — 手机号验证码登录

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthState {
    pub is_logged_in: bool,
    pub phone: Option<String>,
    pub hasn_uuid: Option<String>,
    pub nickname: Option<String>,
}

#[tauri::command]
pub async fn login(phone: String, _code: String) -> Result<AuthState, String> {
    // TODO: Phase 1 实现
    // 1. 验证手机号+验证码
    // 2. 获取 JWT + HASN token
    // 3. 存储到本地
    tracing::info!("login attempt: phone={}", phone);
    Err("登录功能待实现 (Phase 1)".into())
}

#[tauri::command]
pub async fn logout() -> Result<(), String> {
    // TODO: Phase 1 实现
    tracing::info!("logout");
    Ok(())
}

#[tauri::command]
pub async fn get_auth_state() -> Result<AuthState, String> {
    Ok(AuthState {
        is_logged_in: false,
        phone: None,
        hasn_uuid: None,
        nickname: None,
    })
}
