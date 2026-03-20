//! 唤星桌面端 — Tauri 命令注册 + Sidecar 生命周期管理
//!
//! 注册所有 Tauri IPC 命令，供前端 invoke() 调用。
//! 唤星使用独立的配置目录 (~/.huanxing/) 和端口 (42620)，
//! 与用户可能自装的 ZeroClaw 完全隔离。
//! 登录后由前端触发 onboard → 生成配置 → 启动 sidecar。

mod commands;
mod sidecar;

use commands::{auth, hasn, zeroclaw};
use sidecar::SidecarManager;
use std::sync::Arc;
use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let manager = Arc::new(SidecarManager::new());

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_notification::init())
        .manage(manager.clone())
        .setup({
            let mgr = manager.clone();
            move |app| {
                let handle = app.handle().clone();

                // 后台检查是否有已在运行的唤星 sidecar
                tauri::async_runtime::spawn(async move {
                    let port = sidecar::HUANXING_PORT;
                    eprintln!("[huanxing-desktop] setup: checking sidecar on port {port}...");

                    // 尝试连接已有的唤星 sidecar（上次 App 关闭后常驻的）
                    if mgr.adopt_existing(port).await {
                        eprintln!("[huanxing-desktop] Adopted existing sidecar on port {port}");
                        let _ = handle.emit(
                            "sidecar://status-changed",
                            serde_json::json!({
                                "running": true,
                                "port": port,
                            }),
                        );
                        return;
                    }

                    eprintln!(
                        "[huanxing-desktop] No existing sidecar. has_config={}",
                        mgr.has_config()
                    );

                    // 检查是否有配置文件（说明之前登录过）
                    if mgr.has_config() {
                        eprintln!("[huanxing-desktop] Config found, starting sidecar...");
                        match mgr.start(handle).await {
                            Ok(status) => {
                                eprintln!(
                                    "[huanxing-desktop] Sidecar started: PID={:?}, port={}",
                                    status.pid, status.port
                                );
                            }
                            Err(e) => {
                                eprintln!("[huanxing-desktop] Sidecar start FAILED: {e}");
                            }
                        }
                    } else {
                        eprintln!(
                            "[huanxing-desktop] No config at {}, waiting for login",
                            mgr.config_dir().display()
                        );
                    }
                });

                Ok(())
            }
        })
        .invoke_handler(tauri::generate_handler![
            // 认证
            auth::login,
            auth::logout,
            auth::get_auth_state,
            // HASN IM
            hasn::get_conversations,
            hasn::get_messages,
            hasn::send_message,
            hasn::mark_conversation_read,
            hasn::get_contacts,
            hasn::send_friend_request,
            hasn::respond_friend_request,
            // ZeroClaw sidecar
            zeroclaw::start_zeroclaw,
            zeroclaw::stop_zeroclaw,
            zeroclaw::restart_zeroclaw,
            zeroclaw::get_zeroclaw_status,
            zeroclaw::get_zeroclaw_logs,
            zeroclaw::get_zeroclaw_config,
            zeroclaw::update_zeroclaw_config,
            // Onboard（登录后创建配置+启动）
            zeroclaw::onboard_zeroclaw,
        ])
        .build(tauri::generate_context!())
        .expect("error while building huanxing desktop");

    // App 事件循环 — sidecar 常驻后台，App 退出不关闭它
    app.run(|_app_handle, event| {
        if let tauri::RunEvent::Exit = event {
            tracing::info!("App exiting, huanxing sidecar continues running in background");
        }
    });
}
