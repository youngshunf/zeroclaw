//! 唤星桌面端 — Tauri 命令注册 + Sidecar 生命周期管理
//!
//! 注册所有 Tauri IPC 命令，供前端 invoke() 调用。
//! 唤星使用独立的配置目录 (~/.huanxing/) 和端口 (42620)，
//! 与用户可能自装的 ZeroClaw 完全隔离。
//! 登录后由前端触发 onboard → 生成配置 → 启动 sidecar。

mod commands;
mod sidecar;
mod utils;
mod services;

use commands::{auth, files, hasn, marketplace, zeroclaw};
use hasn::HasnClientState;
use sidecar::SidecarManager;
use std::sync::Arc;
use tauri::Emitter;

/// 默认 HASN API 地址（配置中未设置时使用）
const HASN_API_BASE_DEFAULT: &str = "https://api.huanxing.dcfuture.cn";

/// 从 ~/.huanxing/config.toml 读取 [huanxing] api_base_url
fn read_hasn_api_base() -> String {
    let config_path = dirs::home_dir()
        .unwrap_or_default()
        .join(".huanxing")
        .join("config.toml");

    if let Ok(content) = std::fs::read_to_string(&config_path) {
        // 解析 TOML 获取 huanxing.api_base_url
        if let Ok(table) = content.parse::<toml::Table>() {
            if let Some(huanxing) = table.get("huanxing").and_then(|v| v.as_table()) {
                if let Some(url) = huanxing.get("api_base_url").and_then(|v| v.as_str()) {
                    let url = url.trim().trim_end_matches('/');
                    if !url.is_empty() {
                        eprintln!("[huanxing-desktop] HASN API: {} (from config.toml)", url);
                        return url.to_string();
                    }
                }
            }
        }
    }

    eprintln!("[huanxing-desktop] HASN API: {} (default)", HASN_API_BASE_DEFAULT);
    HASN_API_BASE_DEFAULT.to_string()
}

/// HASN 本地数据库路径
fn hasn_db_path() -> String {
    let dir = dirs::home_dir()
        .unwrap_or_default()
        .join(".huanxing")
        .join("hasn");
    std::fs::create_dir_all(&dir).ok();
    dir.join("hasn.db").to_string_lossy().to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let manager = Arc::new(SidecarManager::new());

    // 从配置文件读取 HASN API 地址
    let hasn_api_base = read_hasn_api_base();

    // 初始化 HASN 客户端状态
    let hasn_state = Arc::new(
        HasnClientState::new(&hasn_api_base, &hasn_db_path())
            .expect("初始化 HASN 客户端失败"),
    );

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_notification::init())
        .manage(manager.clone())
        .manage(hasn_state.clone())
        .setup({
            let mgr = manager.clone();
            let hasn_st = hasn_state.clone();
            move |app| {
                let handle = app.handle().clone();

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

                        // Sidecar 就绪后，尝试自动连接 HASN
                        hasn::hasn_auto_connect(hasn_st, handle.clone()).await;

                        // 异步更新后台缓存应用市场数据
                        tauri::async_runtime::spawn(async move {
                            marketplace::sync_marketplace_data().await;
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
            // HASN 连接管理
            hasn::hasn_connect,
            hasn::hasn_disconnect,
            hasn::hasn_status,
            hasn::hasn_provide_token,
            hasn::hasn_get_client_id,
            // HASN IM
            hasn::get_conversations,
            hasn::get_messages,
            hasn::send_message,
            hasn::mark_conversation_read,
            // HASN 联系人
            hasn::get_contacts,
            hasn::send_friend_request,
            hasn::get_friend_requests,
            hasn::respond_friend_request,
            // HASN Agent
            hasn::get_my_agents,
            hasn::set_hasn_sidecar_port,
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
