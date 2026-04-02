//! 唤星桌面端 — Tauri 命令注册 + Sidecar 生命周期管理
//!
//! 注册所有 Tauri IPC 命令，供前端 invoke() 调用。
//! 唤星使用独立的配置目录 (~/.huanxing/) 和端口 (42620)，
//! 与用户可能自装的 ZeroClaw 完全隔离。
//! 登录后由前端触发 onboard → 生成配置 → 启动 sidecar。

mod commands;
mod sidecar;
mod tray;

use commands::{auth, channels, files, marketplace, zeroclaw};
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
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(manager.clone())
        .setup({
            let mgr = manager.clone();
            move |app| {
                let handle = app.handle().clone();

                tray::setup_tray(&handle).expect("初始化托盘失败");

                // 后台检查唤星配置完整性并启动 sidecar
                tauri::async_runtime::spawn(async move {
                    let port = sidecar::HUANXING_PORT;
                    eprintln!("[huanxing-desktop] setup: checking config and sidecar...");

                    let config_valid = mgr.has_valid_huanxing_config();

                    if config_valid {
                        // 配置有效 → 尝试连接已有 sidecar，或启动新的
                        if mgr.adopt_existing(port).await {
                            eprintln!("[huanxing-desktop] Adopted existing sidecar on port {port}");
                            let _ = handle.emit(
                                "sidecar://status-changed",
                                serde_json::json!({
                                    "running": true,
                                    "port": port,
                                }),
                            );
                        } else {
                            eprintln!("[huanxing-desktop] Valid config, starting sidecar...");
                            match mgr.start(handle.clone()).await {
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
                        }

                        // 异步更新后台缓存应用市场数据
                        let market_handle = handle.clone();
                        tauri::async_runtime::spawn(async move {
                            marketplace::sync_marketplace_data(Some(market_handle)).await;
                        });

                        return;
                    }

                    // 配置无效或不存在 → 杀掉可能残留的 sidecar 进程
                    eprintln!(
                        "[huanxing-desktop] No valid huanxing config at {}",
                        mgr.config_dir().display()
                    );
                    mgr.kill_orphan_sidecar(port).await;

                    // 通知前端需要登录
                    let _ = handle.emit(
                        "huanxing:config-invalid",
                        serde_json::json!({
                            "config_dir": mgr.config_dir().to_string_lossy(),
                            "config_exists": mgr.has_config(),
                        }),
                    );
                });

                Ok(())
            }
        })
        .invoke_handler(tauri::generate_handler![
            // 认证
            auth::login,
            auth::logout,
            auth::get_auth_state,
            // ZeroClaw sidecar
            zeroclaw::start_zeroclaw,
            zeroclaw::stop_zeroclaw,
            zeroclaw::restart_zeroclaw,
            zeroclaw::get_zeroclaw_status,
            zeroclaw::get_zeroclaw_logs,
            zeroclaw::get_zeroclaw_config,
            zeroclaw::update_zeroclaw_config,
            // 市场安装与数据接口
            marketplace::get_market_apps,
            marketplace::get_market_skills,
            marketplace::get_market_sops,
            marketplace::download_and_install_agent,
            marketplace::download_and_install_skill,
            marketplace::download_and_install_sop,
            // Onboard（登录后创建配置+启动）
            zeroclaw::onboard_zeroclaw,
            // 配置有效性检查
            zeroclaw::check_huanxing_config,
            // 文件操作
            files::copy_file_to_workspace,
            files::get_workspace_dir,
            files::get_config_dir,
            // 通道与绑定
            channels::list_user_agents,
            channels::bind_channel_to_agent,
            channels::generate_weixin_qr,
            channels::poll_weixin_auth_status,
            channels::save_weixin_credentials,
        ])
        .build(tauri::generate_context!())
        .expect("error while building huanxing desktop");

    // App 事件循环 — sidecar 常驻后台，App 退出不关闭它
    app.run(|app_handle, event| {
        match event {
            tauri::RunEvent::Exit => {
                tracing::info!("App exiting, huanxing sidecar continues running in background");
            }
            tauri::RunEvent::WindowEvent {
                label,
                event: tauri::WindowEvent::CloseRequested { api, .. },
                ..
            } => {
                if label == "main" {
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let _ = window.hide();
                    }
                    api.prevent_close();
                }
            }
            tauri::RunEvent::ExitRequested { api, .. } => {
                // 防止最后一个窗口关闭时（即被 hide 时系统默认行为）直接退出
                api.prevent_exit();
            }
            _ => {}
        }
    });
}
