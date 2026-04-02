use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use crate::sidecar::config::{cleanup_pid_file_helper, kill_by_pid_file_helper};
use crate::sidecar::constants::*;
use crate::sidecar::models::*;
use crate::sidecar::monitor::{monitor_loop, SidecarMonitorHandle};

pub struct SidecarManager {
    /// 子进程句柄
    pub(crate) child: Arc<Mutex<Option<Child>>>,
    /// 当前端口
    pub(crate) port: AtomicU32, // u16 stored as u32 for atomic
    /// 配置目录（~/.zeroclaw）
    pub(crate) config_dir: PathBuf,
    /// 自动重启计数
    pub(crate) restart_count: AtomicU32,
    /// 上次重启时间
    pub(crate) last_restart: Arc<Mutex<Option<Instant>>>,
    /// 日志环形缓冲
    pub(crate) log_buffer: Arc<Mutex<VecDeque<String>>>,
    /// 监控循环是否运行中
    pub(crate) monitoring: AtomicBool,
    /// 正在停止中（避免自动重启）
    pub(crate) stopping: AtomicBool,
}

impl SidecarManager {
    /// 创建新的 SidecarManager
    pub fn new() -> Self {
        let config_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(HUANXING_DIR_NAME);

        Self {
            child: Arc::new(Mutex::new(None)),
            port: AtomicU32::new(HUANXING_PORT as u32),
            config_dir,
            restart_count: AtomicU32::new(0),
            last_restart: Arc::new(Mutex::new(None)),
            log_buffer: Arc::new(Mutex::new(VecDeque::with_capacity(LOG_BUFFER_SIZE))),
            monitoring: AtomicBool::new(false),
            stopping: AtomicBool::new(false),
        }
    }

    /// 查找 zeroclaw binary，按优先级：
    /// 1. $ZEROCLAW_BIN 环境变量
    /// 2. 开发模式：项目目录下 target/release/zeroclaw
    /// 3. PATH 中的 zeroclaw
    pub(crate) fn find_binary(&self) -> Result<PathBuf, String> {
        // 1. 环境变量
        if let Ok(bin) = std::env::var("ZEROCLAW_BIN") {
            let path = PathBuf::from(&bin);
            if path.exists() {
                tracing::info!("Using zeroclaw from $ZEROCLAW_BIN: {}", path.display());
                return Ok(path);
            }
            tracing::warn!("$ZEROCLAW_BIN set but not found: {bin}");
        }

        // 2. 开发模式：相对于 Tauri 项目位置
        // clients/desktop/src-tauri/ → 上三级就是 zeroclaw 根目录
        let dev_paths = [
            // 从 src-tauri 出发
            PathBuf::from("../../../target/release/zeroclaw"),
            // 从工作目录出发（可能是项目根）
            PathBuf::from("target/release/zeroclaw"),
            // 绝对路径 fallback
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../target/release/zeroclaw"),
        ];
        for path in &dev_paths {
            if path.exists() {
                let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
                tracing::info!("Using zeroclaw from dev build: {}", canonical.display());
                return Ok(canonical);
            }
        }

        // 3. PATH 查找
        if let Ok(path) = which::which("zeroclaw") {
            tracing::info!("Using zeroclaw from PATH: {}", path.display());
            return Ok(path);
        }

        Err("找不到 zeroclaw 可执行文件。\n\
             请先编译: cargo build --release\n\
             或设置环境变量: ZEROCLAW_BIN=/path/to/zeroclaw"
            .to_string())
    }

    /// 检查端口是否可用
    fn is_port_available(port: u16) -> bool {
        std::net::TcpListener::bind(("127.0.0.1", port)).is_ok()
    }

    /// 找到一个可用端口
    fn find_available_port(&self) -> u16 {
        let preferred = self.port.load(Ordering::Relaxed) as u16;
        if Self::is_port_available(preferred) {
            return preferred;
        }
        // 端口被占用，尝试从唤星端口范围开始找
        for port in HUANXING_PORT..HUANXING_PORT + 80 {
            if Self::is_port_available(port) {
                tracing::warn!("Port {preferred} occupied, using {port}");
                return port;
            }
        }
        // fallback: 让 OS 分配
        0
    }

    /// 写 PID 文件
    pub(crate) fn write_pid_file(&self, pid: u32) {
        let pid_path = self.config_dir.join(".sidecar.pid");
        if let Err(e) = std::fs::write(&pid_path, pid.to_string()) {
            tracing::warn!("Failed to write PID file: {e}");
        }
    }

    /// 启动 sidecar
    pub async fn start(&self, app: AppHandle) -> Result<SidecarStatus, String> {
        // 检查是否已在运行
        {
            let child = self.child.lock().await;
            if child.is_some() {
                return Err("Sidecar 已在运行中".to_string());
            }
        }

        self.stopping.store(false, Ordering::Relaxed);

        let bin = self.find_binary()?;
        let port = self.find_available_port();

        tracing::info!(
            "Starting zeroclaw sidecar: {} daemon --port {}",
            bin.display(),
            port
        );

        // 清理可能的残留 PID 文件
        cleanup_pid_file_helper(&self.config_dir);

        let mut cmd = Command::new(&bin);
        cmd.arg("daemon")
            .arg("--port")
            .arg(port.to_string())
            .arg("--config-dir")
            .arg(self.config_dir.to_str().unwrap_or("~/.huanxing"))
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .env("ZEROCLAW_BUILD_VERSION", "huanxing-desktop");

        if !self.config_dir.join("config.toml").exists() {
            tracing::warn!(
                "No config.toml found at {}. ZeroClaw will create a default one.",
                self.config_dir.display()
            );
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("启动 zeroclaw 失败: {e}\n路径: {}", bin.display()))?;

        let pid = child.id();
        tracing::info!("Sidecar spawned, PID: {:?}", pid);

        if let Some(pid) = pid {
            self.write_pid_file(pid);
        }

        if let Some(stdout) = child.stdout.take() {
            let log_buf = self.log_buffer.clone();
            let app_clone = app.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let mut buf = log_buf.lock().await;
                    if buf.len() >= LOG_BUFFER_SIZE {
                        buf.pop_front();
                    }
                    buf.push_back(line.clone());
                    drop(buf);
                    let _ = app_clone.emit("sidecar://log", &line);
                }
            });
        }

        if let Some(stderr) = child.stderr.take() {
            let log_buf = self.log_buffer.clone();
            let app_clone = app.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let mut buf = log_buf.lock().await;
                    if buf.len() >= LOG_BUFFER_SIZE {
                        buf.pop_front();
                    }
                    buf.push_back(format!("[stderr] {line}"));
                    drop(buf);
                    let _ = app_clone.emit("sidecar://log", &format!("[stderr] {line}"));
                }
            });
        }

        self.port.store(port as u32, Ordering::Relaxed);
        {
            let mut guard = self.child.lock().await;
            *guard = Some(child);
        }

        let healthy = self.wait_for_healthy(port).await;
        if !healthy {
            tracing::warn!("Sidecar started but health check didn't pass within timeout");
        }

        if !self.monitoring.swap(true, Ordering::Relaxed) {
            let manager = SidecarMonitorHandle {
                child: self.child.clone(),
                port,
                restart_count: &self.restart_count as *const AtomicU32,
                last_restart: self.last_restart.clone(),
                log_buffer: self.log_buffer.clone(),
                monitoring: &self.monitoring as *const AtomicBool,
                stopping: &self.stopping as *const AtomicBool,
                bin_path: bin.clone(),
                config_dir: self.config_dir.to_str().unwrap_or("").to_string(),
            };
            let app_clone = app.clone();
            tokio::spawn(async move {
                unsafe {
                    monitor_loop(manager, app_clone).await;
                }
            });
        }

        let status = self.build_status(port).await;

        let _ = app.emit(
            "sidecar://status-changed",
            SidecarEvent {
                running: true,
                pid,
                port,
                model: status.model.clone(),
            },
        );

        Ok(status)
    }

    /// 停止 sidecar
    pub async fn stop(&self, app: &AppHandle) -> Result<(), String> {
        self.stopping.store(true, Ordering::Relaxed);

        let mut guard = self.child.lock().await;
        if let Some(ref mut child) = *guard {
            let pid = child.id();
            tracing::info!("Stopping sidecar (PID: {:?})", pid);

            #[cfg(unix)]
            if let Some(pid) = pid {
                unsafe {
                    libc::kill(pid as i32, libc::SIGTERM);
                }
            }
            #[cfg(not(unix))]
            {
                let _ = child.kill().await;
            }

            let wait_result = tokio::time::timeout(GRACEFUL_SHUTDOWN_TIMEOUT, child.wait()).await;

            match wait_result {
                Ok(Ok(status)) => {
                    tracing::info!("Sidecar exited with: {status}");
                }
                Ok(Err(e)) => {
                    tracing::warn!("Error waiting for sidecar: {e}");
                }
                Err(_) => {
                    tracing::warn!("Sidecar didn't exit gracefully, killing");
                    let _ = child.kill().await;
                }
            }

            *guard = None;
        } else {
            kill_by_pid_file_helper(&self.config_dir);
        }

        cleanup_pid_file_helper(&self.config_dir);
        self.monitoring.store(false, Ordering::Relaxed);
        self.restart_count.store(0, Ordering::Relaxed);

        let port = self.port.load(Ordering::Relaxed) as u16;
        let _ = app.emit(
            "sidecar://status-changed",
            SidecarEvent {
                running: false,
                pid: None,
                port,
                model: None,
            },
        );

        Ok(())
    }

    /// 取名为 adopt_existing 并且支持尝试重连（来自 lib.rs 的调用）
    pub async fn adopt_existing(&self, port: u16) -> bool {
        let client = reqwest::Client::builder()
            .timeout(HEALTH_TIMEOUT)
            .build()
            .unwrap_or_default();

        if client
            .get(format!("http://127.0.0.1:{port}/health"))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
        {
            self.port.store(port as u32, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    /// 获取当前状态
    pub async fn status(&self) -> SidecarStatus {
        let port = self.port.load(Ordering::Relaxed) as u16;
        self.build_status(port).await
    }

    /// 获取日志
    pub async fn logs(&self, lines: usize) -> Vec<String> {
        let buf = self.log_buffer.lock().await;
        let start = if buf.len() > lines {
            buf.len() - lines
        } else {
            0
        };
        buf.iter().skip(start).cloned().collect()
    }

    // ── 内部辅助方法 ──

    async fn wait_for_healthy(&self, port: u16) -> bool {
        let start = Instant::now();
        let client = reqwest::Client::builder()
            .timeout(HEALTH_TIMEOUT)
            .build()
            .unwrap_or_default();

        while start.elapsed() < STARTUP_TIMEOUT {
            tokio::time::sleep(Duration::from_millis(500)).await;

            match client
                .get(format!("http://127.0.0.1:{port}/health"))
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    tracing::info!("Sidecar health check passed after {:?}", start.elapsed());
                    return true;
                }
                Ok(resp) => {
                    tracing::debug!("Health check returned {}", resp.status());
                }
                Err(e) => {
                    tracing::debug!("Health check not ready: {e}");
                }
            }
        }
        false
    }

    async fn fetch_api_status(&self, port: u16) -> Option<StatusResponse> {
        let client = reqwest::Client::builder()
            .timeout(HEALTH_TIMEOUT)
            .build()
            .ok()?;

        let health = client
            .get(format!("http://127.0.0.1:{port}/health"))
            .send()
            .await
            .ok()
            .and_then(|r| {
                if r.status().is_success() {
                    Some(r)
                } else {
                    None
                }
            });

        let health_data: Option<HealthResponse> = match health {
            Some(r) => r.json().await.ok(),
            None => return None,
        };

        let pid = health_data
            .as_ref()
            .and_then(|h| h.runtime.as_ref())
            .and_then(|r| r.pid);

        let uptime = health_data
            .as_ref()
            .and_then(|h| h.runtime.as_ref())
            .and_then(|r| r.uptime_seconds);

        let api_resp = client
            .get(format!("http://127.0.0.1:{port}/api/status"))
            .send()
            .await
            .ok()
            .and_then(|r| {
                if r.status().is_success() {
                    Some(r)
                } else {
                    None
                }
            });

        let mut status = match api_resp {
            Some(r) => r.json::<StatusResponse>().await.unwrap_or(StatusResponse {
                model: None,
                provider: None,
                uptime_seconds: uptime,
                memory_backend: None,
                gateway_port: None,
                pid,
            }),
            None => StatusResponse {
                model: None,
                provider: None,
                uptime_seconds: uptime,
                memory_backend: None,
                gateway_port: None,
                pid,
            },
        };

        if status.pid.is_none() {
            status.pid = pid;
        }
        if status.uptime_seconds.is_none() {
            status.uptime_seconds = uptime;
        }

        Some(status)
    }

    async fn build_status(&self, port: u16) -> SidecarStatus {
        let child = self.child.lock().await;
        let has_child = child.is_some();
        let pid = child.as_ref().and_then(|c| c.id());
        let restart_count = self.restart_count.load(Ordering::Relaxed);
        drop(child);

        let api_status = self.fetch_api_status(port).await;
        let actually_running = has_child || api_status.is_some();

        let (model, provider, uptime, memory_backend, remote_pid) = match &api_status {
            Some(s) => (
                s.model.clone(),
                s.provider.clone(),
                s.uptime_seconds,
                s.memory_backend.clone(),
                s.pid,
            ),
            None => (None, None, None, None, None),
        };

        let effective_pid = pid.or(remote_pid);

        SidecarStatus {
            running: actually_running,
            pid: effective_pid,
            port,
            model,
            provider,
            uptime_seconds: uptime,
            memory_backend,
            restart_count,
            version: None,
        }
    }
}
