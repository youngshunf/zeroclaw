//! ZeroClaw In-Process Engine — 移动端引擎嵌入层
//!
//! 在移动端（iOS/Android），ZeroClaw 无法作为独立进程运行。
//! 此模块创建一个独立的 Tokio Runtime（与 Tauri UI Runtime 完全隔离），
//! 直接调用 `zeroclaw::daemon::run()` 启动完整引擎服务栈（gateway、channels、heartbeat 等）。
//!
//! 行为与桌面端 sidecar 模式完全一致：
//! - 引擎监听 localhost:42620（与 sidecar 端口相同）
//! - 前端通过 HTTP 或 Tauri invoke → bridge 转发访问引擎
//! - 引擎生命周期不受 UI 线程影响

pub mod bridge;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;

/// 嵌入式引擎默认端口（与 sidecar 一致）
pub const ENGINE_PORT: u16 = 42620;

/// 嵌入式引擎配置目录名
const HUANXING_DIR_NAME: &str = ".huanxing";

/// ZeroClaw 嵌入式引擎
///
/// 持有独立的 Tokio Runtime，与 Tauri UI Runtime 完全隔离。
/// `daemon::run()` 在引擎 Runtime 内运行，包含 gateway、channels、heartbeat 等全部组件。
pub struct EmbeddedEngine {
    /// 独立 Tokio Runtime（4 worker threads）
    runtime: Runtime,
    /// 引擎任务句柄
    engine_handle: Option<JoinHandle<()>>,
    /// 引擎监听端口
    port: u16,
    /// 配置目录
    config_dir: PathBuf,
    /// 是否正在运行
    running: Arc<AtomicBool>,
}

impl EmbeddedEngine {
    /// 创建并启动嵌入式引擎
    ///
    /// - 创建独立 Tokio Runtime（4 worker threads，thread name: zeroclaw-engine）
    /// - 加载 ZeroClaw Config
    /// - 启动 daemon::run()（gateway + channels + heartbeat + scheduler）
    pub fn start(config_dir: &str, port: u16) -> Result<Self, String> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .thread_name("zeroclaw-engine")
            .build()
            .map_err(|e| format!("创建引擎运行时失败: {e}"))?;

        let config_path = PathBuf::from(config_dir).join("config.toml");
        let running = Arc::new(AtomicBool::new(false));
        let running_clone = running.clone();

        // 在独立 Runtime 内启动 daemon
        let engine_handle = runtime.spawn(async move {
            tracing::info!(
                "[embedded-engine] Starting ZeroClaw engine on port {port}, config: {}",
                config_path.display()
            );

            // 加载 ZeroClaw 配置（自动解析默认路径）
            let config = match zeroclaw::Config::load_or_init().await {
                Ok(cfg) => cfg,
                Err(e) => {
                    tracing::error!("[embedded-engine] Failed to load config: {e}");
                    return;
                }
            };

            running_clone.store(true, Ordering::SeqCst);

            let host = {
                let h = config.gateway.host.clone();
                if h.is_empty() { "127.0.0.1".to_string() } else { h }
            };

            // 启动完整 daemon（gateway + channels + heartbeat + scheduler）
            if let Err(e) = zeroclaw::daemon::run(config, host, port).await {
                tracing::error!("[embedded-engine] Engine exited with error: {e}");
            }

            running_clone.store(false, Ordering::SeqCst);
            tracing::info!("[embedded-engine] Engine stopped");
        });

        Ok(Self {
            runtime,
            engine_handle: Some(engine_handle),
            port,
            config_dir: PathBuf::from(config_dir),
            running,
        })
    }

    /// 检查引擎是否正在运行
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// 获取引擎端口
    pub fn port(&self) -> u16 {
        self.port
    }

    /// 获取配置目录
    pub fn config_dir(&self) -> &PathBuf {
        &self.config_dir
    }

    /// 通过 HTTP 健康检查验证引擎是否就绪
    pub async fn wait_for_healthy(&self) -> bool {
        let port = self.port;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap_or_default();

        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(15);

        while start.elapsed() < timeout {
            tokio::time::sleep(Duration::from_millis(500)).await;
            match client
                .get(format!("http://127.0.0.1:{port}/health"))
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    tracing::info!(
                        "[embedded-engine] Health check passed after {:?}",
                        start.elapsed()
                    );
                    return true;
                }
                _ => {}
            }
        }

        tracing::warn!("[embedded-engine] Health check timeout after {timeout:?}");
        false
    }

    /// 停止引擎
    pub fn stop(&mut self) {
        tracing::info!("[embedded-engine] Stopping engine...");
        if let Some(handle) = self.engine_handle.take() {
            handle.abort();
        }
        self.running.store(false, Ordering::SeqCst);
    }

    /// 使用默认参数创建引擎（便捷方法）
    pub fn default_start() -> Result<Self, String> {
        let config_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(HUANXING_DIR_NAME);

        Self::start(
            config_dir
                .to_str()
                .ok_or("config_dir 路径包含非法字符")?,
            ENGINE_PORT,
        )
    }
}

impl Drop for EmbeddedEngine {
    fn drop(&mut self) {
        self.stop();
    }
}
