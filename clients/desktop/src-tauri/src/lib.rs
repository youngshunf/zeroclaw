//! 唤星桌面端/移动端 — Tauri 命令注册 + 引擎生命周期管理
//!
//! 注册所有 Tauri IPC 命令，供前端 invoke() 调用。
//! 唤星使用独立的配置目录 (~/.huanxing/) 和端口 (42620)，
//! 与用户可能自装的 ZeroClaw 完全隔离。
//!
//! ## 双模式架构
//!
//! - **桌面端** `cfg(not(mobile))`: ZeroClaw 以独立 sidecar 进程运行，
//!   通过 SidecarManager 管理子进程生命周期。
//!
//! - **移动端** `cfg(mobile)`: ZeroClaw 以 in-process library 运行，
//!   EmbeddedEngine 创建独立 Tokio Runtime，调用 zeroclaw::daemon::run()。
//!
//! 登录后由前端触发 onboard → 生成配置 → 启动引擎。

mod commands;

// 桌面端：sidecar 进程管理 + 系统托盘
#[cfg(not(mobile))]
mod sidecar;
#[cfg(not(mobile))]
mod tray;

// 移动端：in-process 引擎（仅在 mobile feature 启用时编译，需要 zeroclaw 依赖）
#[cfg(feature = "mobile")]
mod engine;

#[cfg(not(mobile))]
use commands::{auth, channels, files, marketplace, zeroclaw};
#[cfg(not(mobile))]
use sidecar::SidecarManager;

#[cfg(all(mobile, not(feature = "mobile")))]
use commands::{auth, channels, files, marketplace};

#[cfg(feature = "mobile")]
use commands::{auth, channels, files, marketplace};

use std::sync::Arc;
use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // ── 公共 Builder 配置 ──
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init());

    // ── 桌面端：SidecarManager + 托盘 ──
    #[cfg(not(mobile))]
    let app = {
        let manager = Arc::new(SidecarManager::new());
        builder
            .manage(manager.clone())
            .setup({
                let mgr = manager.clone();
                move |app| {
                    let handle = app.handle().clone();

                    tray::setup_tray(&handle).expect("初始化托盘失败");

                    // 后台检查唤星配置完整性并启动 sidecar
                    tauri::async_runtime::spawn(async move {
                        let port = sidecar::HUANXING_PORT;
                        eprintln!(
                            "[huanxing-desktop] setup: checking config and sidecar..."
                        );

                        let config_valid = mgr.has_valid_huanxing_config();

                        if config_valid {
                            // 配置有效 → 尝试连接已有 sidecar，或启动新的
                            if mgr.adopt_existing(port).await {
                                eprintln!(
                                    "[huanxing-desktop] Adopted existing sidecar on port {port}"
                                );
                                let _ = handle.emit(
                                    "sidecar://status-changed",
                                    serde_json::json!({
                                        "running": true,
                                        "port": port,
                                    }),
                                );
                            } else {
                                eprintln!(
                                    "[huanxing-desktop] Valid config, starting sidecar..."
                                );
                                match mgr.start(handle.clone()).await {
                                    Ok(status) => {
                                        eprintln!(
                                            "[huanxing-desktop] Sidecar started: PID={:?}, port={}",
                                            status.pid, status.port
                                        );
                                    }
                                    Err(e) => {
                                        eprintln!(
                                            "[huanxing-desktop] Sidecar start FAILED: {e}"
                                        );
                                    }
                                }
                            }

                            // 异步更新后台缓存应用市场数据
                            let market_handle = handle.clone();
                            tauri::async_runtime::spawn(async move {
                                marketplace::sync_marketplace_data(Some(market_handle))
                                    .await;
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
            .expect("error while building huanxing desktop")
    };

    // ── 移动端（带引擎）：EmbeddedEngine ──
    #[cfg(feature = "mobile")]
    let app = {
        use tokio::sync::Mutex;

        builder
            .setup(move |app| {
                let handle = app.handle().clone();

                eprintln!("[huanxing-mobile] setup: initializing embedded engine...");

                // iOS 模拟器共享 Mac 文件系统，但 dirs::home_dir() 和 $HOME 都返回沙箱路径。
                // 策略：优先检查 SIMULATOR_HOST_HOME → 扫描 /Users → 沙箱回退
                let config_dir = {
                    let sandbox_home = dirs::home_dir()
                        .unwrap_or_else(|| std::path::PathBuf::from("."));

                    eprintln!("[huanxing-mobile] sandbox_home = {}", sandbox_home.display());
                    eprintln!("[huanxing-mobile] HOME = {:?}", std::env::var("HOME"));
                    eprintln!("[huanxing-mobile] SIMULATOR_HOST_HOME = {:?}", std::env::var("SIMULATOR_HOST_HOME"));

                    // 1. SIMULATOR_HOST_HOME（Xcode 在模拟器中设置，指向 Mac 真实 home）
                    let mut found: Option<std::path::PathBuf> = None;
                    if let Ok(host_home) = std::env::var("SIMULATOR_HOST_HOME") {
                        let candidate = std::path::PathBuf::from(&host_home).join(".huanxing");
                        if candidate.join("config.toml").exists() {
                            eprintln!("[huanxing-mobile] Using SIMULATOR_HOST_HOME: {}", candidate.display());
                            found = Some(candidate);
                        }
                    }

                    // 2. 扫描 /Users 下的常见路径（模拟器可以访问 Mac 文件系统）
                    if found.is_none() {
                        for entry in std::fs::read_dir("/Users").into_iter().flatten().flatten() {
                            let candidate = entry.path().join(".huanxing");
                            if candidate.join("config.toml").exists() {
                                eprintln!("[huanxing-mobile] Found config at: {}", candidate.display());
                                found = Some(candidate);
                                break;
                            }
                        }
                    }

                    // 3. 回退到沙箱
                    found.unwrap_or_else(|| {
                        eprintln!("[huanxing-mobile] No host config found, using sandbox: {}", sandbox_home.display());
                        sandbox_home.join(".huanxing")
                    })
                };

                let config_dir_str = config_dir.to_string_lossy().to_string();
                let config_valid = config_dir.join("config.toml").exists();

                if config_valid {
                    // 启动嵌入式引擎
                    match engine::EmbeddedEngine::start(
                        &config_dir_str,
                        engine::ENGINE_PORT,
                    ) {
                        Ok(eng) => {
                            eprintln!(
                                "[huanxing-mobile] Engine started on port {}",
                                eng.port()
                            );
                            app.manage(Arc::new(Mutex::new(eng)));

                            // 异步等待健康检查
                            let h = handle.clone();
                            tauri::async_runtime::spawn(async move {
                                // 给引擎启动时间
                                tokio::time::sleep(std::time::Duration::from_secs(2))
                                    .await;
                                let _ = h.emit(
                                    "engine://status-changed",
                                    serde_json::json!({
                                        "running": true,
                                        "port": engine::ENGINE_PORT,
                                    }),
                                );
                            });

                            // 异步更新应用市场缓存
                            let market_handle = handle.clone();
                            tauri::async_runtime::spawn(async move {
                                marketplace::sync_marketplace_data(Some(market_handle))
                                    .await;
                            });
                        }
                        Err(e) => {
                            eprintln!("[huanxing-mobile] Engine start FAILED: {e}");
                            // 仍然 manage 一个空引擎占位，避免 State 不存在 panic
                            // 前端会收到 config-invalid 事件
                        }
                    }
                } else {
                    eprintln!(
                        "[huanxing-mobile] No valid config at {}",
                        config_dir.display()
                    );
                    let _ = handle.emit(
                        "huanxing:config-invalid",
                        serde_json::json!({
                            "config_dir": config_dir.to_string_lossy(),
                            "config_exists": false,
                        }),
                    );
                }

                Ok(())
            })
            .invoke_handler(tauri::generate_handler![
                // 认证
                auth::login,
                auth::logout,
                auth::get_auth_state,
                // 引擎 bridge
                engine::bridge::engine_request,
                engine::bridge::get_engine_status,
                engine::bridge::restart_engine,
                // 市场安装与数据接口
                marketplace::get_market_apps,
                marketplace::get_market_skills,
                marketplace::get_market_sops,
                marketplace::download_and_install_agent,
                marketplace::download_and_install_skill,
                marketplace::download_and_install_sop,
                // 文件操作（不依赖 SidecarManager）
                files::copy_file_to_workspace,
                files::get_config_dir,
                // 通道与绑定
                channels::list_user_agents,
                channels::bind_channel_to_agent,
                channels::generate_weixin_qr,
                channels::poll_weixin_auth_status,
                channels::save_weixin_credentials,
            ])
            .build(tauri::generate_context!())
            .expect("error while building huanxing mobile")
    };

    // ── 事件循环 ──
    app.run(|app_handle, event| {
        match event {
            tauri::RunEvent::Exit => {
                tracing::info!("App exiting");
            }
            // 桌面端：关闭窗口时隐藏到托盘
            #[cfg(not(mobile))]
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
            #[cfg(not(mobile))]
            tauri::RunEvent::ExitRequested { api, .. } => {
                // 防止最后一个窗口关闭时直接退出（桌面端 sidecar 常驻后台）
                api.prevent_exit();
            }
            _ => {}
        }
    });
}
