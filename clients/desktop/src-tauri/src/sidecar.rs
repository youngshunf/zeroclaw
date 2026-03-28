//! 唤星 Sidecar 进程管理器
//!
//! 管理唤星专属的 zeroclaw daemon 子进程。
//! 使用独立的配置目录 (~/.huanxing/) 和端口 (42620)，
//! 与用户可能自装的 ZeroClaw 完全隔离。
//!
//! 生命周期：
//! - 用户登录 → onboard 生成配置 → 启动 sidecar
//! - App 退出 → sidecar 常驻后台
//! - App 再次打开 → adopt 已有 sidecar

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

/// 唤星专属端口（不与 ZeroClaw 默认 42617 冲突）
pub const HUANXING_PORT: u16 = 42620;
/// 唤星配置目录名
const HUANXING_DIR_NAME: &str = ".huanxing";
/// 最大自动重启次数
const MAX_AUTO_RESTARTS: u32 = 3;
/// 自动重启计数重置窗口
const RESTART_WINDOW: Duration = Duration::from_secs(300); // 5 分钟
/// 健康检查超时
const HEALTH_TIMEOUT: Duration = Duration::from_secs(3);
/// 启动后等待健康检查最长时间
const STARTUP_TIMEOUT: Duration = Duration::from_secs(15);
/// 日志缓冲区最大行数
const LOG_BUFFER_SIZE: usize = 500;
/// SIGTERM 后等待退出的时间
const GRACEFUL_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

// ── 数据结构 ──────────────────────────────────────────────

/// Sidecar 运行状态（返回给前端）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidecarStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub port: u16,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub uptime_seconds: Option<u64>,
    pub memory_backend: Option<String>,
    pub restart_count: u32,
    pub version: Option<String>,
}

/// 健康检查 API 响应
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct HealthResponse {
    status: Option<String>,
    #[serde(default)]
    paired: bool,
    #[serde(default)]
    runtime: Option<HealthRuntime>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct HealthRuntime {
    #[serde(default)]
    pid: Option<u32>,
    #[serde(default)]
    uptime_seconds: Option<u64>,
}

/// 状态 API 响应
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct StatusResponse {
    model: Option<String>,
    provider: Option<String>,
    uptime_seconds: Option<u64>,
    memory_backend: Option<String>,
    #[serde(default)]
    gateway_port: Option<u16>,
    #[serde(default)]
    pid: Option<u32>,
}

/// 日志条目
#[derive(Debug, Clone, Serialize)]
#[allow(dead_code)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

/// Sidecar 事件（emit 到前端）
#[derive(Debug, Clone, Serialize)]
struct SidecarEvent {
    running: bool,
    pid: Option<u32>,
    port: u16,
    model: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct CrashEvent {
    exit_code: Option<i32>,
    restart_count: u32,
    will_restart: bool,
}

// ── SidecarManager ───────────────────────────────────────

pub struct SidecarManager {
    /// 子进程句柄
    child: Arc<Mutex<Option<Child>>>,
    /// 当前端口
    port: AtomicU32, // u16 stored as u32 for atomic
    /// 配置目录（~/.zeroclaw）
    config_dir: PathBuf,
    /// 自动重启计数
    restart_count: AtomicU32,
    /// 上次重启时间
    last_restart: Arc<Mutex<Option<Instant>>>,
    /// 日志环形缓冲
    log_buffer: Arc<Mutex<VecDeque<String>>>,
    /// 监控循环是否运行中
    monitoring: AtomicBool,
    /// 正在停止中（避免自动重启）
    stopping: AtomicBool,
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
    fn find_binary(&self) -> Result<PathBuf, String> {
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
        self.cleanup_pid_file();

        let mut cmd = Command::new(&bin);
        cmd.arg("daemon")
            .arg("--port")
            .arg(port.to_string())
            .arg("--config-dir")
            .arg(self.config_dir.to_str().unwrap_or("~/.huanxing"))
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .env("ZEROCLAW_BUILD_VERSION", "huanxing-desktop");

        // 如果配置目录不存在，zeroclaw 会自动创建
        // 但我们提前检查，给用户更好的提示
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

        // 写 PID 文件
        if let Some(pid) = pid {
            self.write_pid_file(pid);
        }

        // 收集 stdout
        if let Some(stdout) = child.stdout.take() {
            let log_buf = self.log_buffer.clone();
            let app_clone = app.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    // 追加到环形缓冲
                    let mut buf = log_buf.lock().await;
                    if buf.len() >= LOG_BUFFER_SIZE {
                        buf.pop_front();
                    }
                    buf.push_back(line.clone());
                    drop(buf);
                    // emit 到前端
                    let _ = app_clone.emit("sidecar://log", &line);
                }
            });
        }

        // 收集 stderr
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

        // 保存子进程
        self.port.store(port as u32, Ordering::Relaxed);
        {
            let mut guard = self.child.lock().await;
            *guard = Some(child);
        }

        // 等待健康检查通过
        let healthy = self.wait_for_healthy(port).await;
        if !healthy {
            tracing::warn!("Sidecar started but health check didn't pass within timeout");
            // 不杀进程——可能只是启动慢
        }

        // 启动监控循环
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
                // SAFETY: AtomicU32/AtomicBool are Send+Sync, the raw pointers point to
                // fields in SidecarManager which lives as long as the Tauri app.
                unsafe {
                    monitor_loop(manager, app_clone).await;
                }
            });
        }

        // 获取完整状态
        let status = self.build_status(port).await;

        // 通知前端
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

            // 发送 SIGTERM
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

            // 等待优雅退出
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
            // 没有管理中的子进程，尝试通过 PID 文件清理
            self.kill_by_pid_file();
        }

        self.cleanup_pid_file();
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

    // ── 内部辅助方法 ──────────────────────────────────────

    /// 等待健康检查通过
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

    /// 构建状态响应
    async fn build_status(&self, port: u16) -> SidecarStatus {
        let child = self.child.lock().await;
        let has_child = child.is_some();
        let pid = child.as_ref().and_then(|c| c.id());
        let restart_count = self.restart_count.load(Ordering::Relaxed);
        drop(child); // 提前释放锁

        // 尝试获取详细状态（不管是自己启动的还是 adopt 的）
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

        // 优先用子进程的 PID，否则用 API 返回的 PID
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

    /// 获取 sidecar 状态（合并 /health 和 /api/status）
    async fn fetch_api_status(&self, port: u16) -> Option<StatusResponse> {
        let client = reqwest::Client::builder()
            .timeout(HEALTH_TIMEOUT)
            .build()
            .ok()?;

        // 先尝试 /health 获取 PID 和 uptime
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
            None => return None, // 连 health 都不通，肯定没运行
        };

        let pid = health_data
            .as_ref()
            .and_then(|h| h.runtime.as_ref())
            .and_then(|r| r.pid);

        let uptime = health_data
            .as_ref()
            .and_then(|h| h.runtime.as_ref())
            .and_then(|r| r.uptime_seconds);

        // 再尝试 /api/status 获取 model/provider 等详情
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

        // 用 health 的 pid/uptime 补充
        if status.pid.is_none() {
            status.pid = pid;
        }
        if status.uptime_seconds.is_none() {
            status.uptime_seconds = uptime;
        }

        Some(status)
    }

    /// 写 PID 文件
    fn write_pid_file(&self, pid: u32) {
        let pid_path = self.config_dir.join(".sidecar.pid");
        if let Err(e) = std::fs::write(&pid_path, pid.to_string()) {
            tracing::warn!("Failed to write PID file: {e}");
        }
    }

    /// 清理 PID 文件
    fn cleanup_pid_file(&self) {
        let pid_path = self.config_dir.join(".sidecar.pid");
        let _ = std::fs::remove_file(&pid_path);
    }

    /// 通过 PID 文件杀残留进程
    fn kill_by_pid_file(&self) {
        let pid_path = self.config_dir.join(".sidecar.pid");
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
}

// ── 监控循环 ──────────────────────────────────────────────

/// 用于传递给监控循环的句柄（避免直接传 &self）
struct SidecarMonitorHandle {
    child: Arc<Mutex<Option<Child>>>,
    port: u16,
    restart_count: *const AtomicU32,
    last_restart: Arc<Mutex<Option<Instant>>>,
    log_buffer: Arc<Mutex<VecDeque<String>>>,
    monitoring: *const AtomicBool,
    stopping: *const AtomicBool,
    bin_path: PathBuf,
    config_dir: String,
}

unsafe impl Send for SidecarMonitorHandle {}

/// 后台监控循环
async unsafe fn monitor_loop(handle: SidecarMonitorHandle, app: AppHandle) {
    loop {
        tokio::time::sleep(Duration::from_secs(5)).await;

        let monitoring = unsafe { &*handle.monitoring };
        if !monitoring.load(Ordering::Relaxed) {
            break;
        }

        let stopping = unsafe { &*handle.stopping };
        if stopping.load(Ordering::Relaxed) {
            continue;
        }

        // 检查子进程是否还活着
        let mut child_guard = handle.child.lock().await;
        if let Some(ref mut child) = *child_guard {
            match child.try_wait() {
                Ok(Some(exit_status)) => {
                    // 进程已退出
                    let code = exit_status.code();
                    tracing::warn!("Sidecar exited unexpectedly: code={code:?}");

                    let restart_count_ref = unsafe { &*handle.restart_count };
                    let _current_count = restart_count_ref.load(Ordering::Relaxed);

                    // 检查重启窗口
                    let mut last_restart = handle.last_restart.lock().await;
                    if let Some(last) = *last_restart {
                        if last.elapsed() > RESTART_WINDOW {
                            restart_count_ref.store(0, Ordering::Relaxed);
                        }
                    }

                    let count = restart_count_ref.load(Ordering::Relaxed);
                    let will_restart =
                        count < MAX_AUTO_RESTARTS && !stopping.load(Ordering::Relaxed);

                    // Emit crash event
                    let _ = app.emit(
                        "sidecar://crash",
                        CrashEvent {
                            exit_code: code,
                            restart_count: count,
                            will_restart,
                        },
                    );

                    *child_guard = None;

                    if will_restart {
                        restart_count_ref.fetch_add(1, Ordering::Relaxed);
                        *last_restart = Some(Instant::now());
                        drop(child_guard);
                        drop(last_restart);

                        tracing::info!(
                            "Auto-restarting sidecar (attempt {}/{})",
                            count + 1,
                            MAX_AUTO_RESTARTS
                        );

                        // 等一秒再重启
                        tokio::time::sleep(Duration::from_secs(1)).await;

                        // 重新启动
                        let mut cmd = Command::new(&handle.bin_path);
                        cmd.arg("daemon")
                            .arg("--port")
                            .arg(handle.port.to_string())
                            .arg("--config-dir")
                            .arg(&handle.config_dir)
                            .stdout(std::process::Stdio::piped())
                            .stderr(std::process::Stdio::piped())
                            .env("ZEROCLAW_BUILD_VERSION", "huanxing-desktop");

                        match cmd.spawn() {
                            Ok(mut new_child) => {
                                let pid = new_child.id();
                                tracing::info!("Sidecar restarted, PID: {pid:?}");

                                // 收集新进程的日志
                                if let Some(stdout) = new_child.stdout.take() {
                                    let log_buf = handle.log_buffer.clone();
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
                                if let Some(stderr) = new_child.stderr.take() {
                                    let log_buf = handle.log_buffer.clone();
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
                                            let _ = app_clone
                                                .emit("sidecar://log", &format!("[stderr] {line}"));
                                        }
                                    });
                                }

                                let _ = app.emit(
                                    "sidecar://status-changed",
                                    SidecarEvent {
                                        running: true,
                                        pid,
                                        port: handle.port,
                                        model: None,
                                    },
                                );

                                let mut guard = handle.child.lock().await;
                                *guard = Some(new_child);
                            }
                            Err(e) => {
                                tracing::error!("Failed to restart sidecar: {e}");
                                let _ = app.emit(
                                    "sidecar://status-changed",
                                    SidecarEvent {
                                        running: false,
                                        pid: None,
                                        port: handle.port,
                                        model: None,
                                    },
                                );
                                monitoring.store(false, Ordering::Relaxed);
                                break;
                            }
                        }
                    } else {
                        tracing::warn!("Max auto-restarts reached, giving up");
                        let _ = app.emit(
                            "sidecar://status-changed",
                            SidecarEvent {
                                running: false,
                                pid: None,
                                port: handle.port,
                                model: None,
                            },
                        );
                        monitoring.store(false, Ordering::Relaxed);
                        break;
                    }
                }
                Ok(None) => {
                    // 进程仍在运行，一切正常
                }
                Err(e) => {
                    tracing::warn!("Error checking sidecar process: {e}");
                }
            }
        } else {
            // 没有子进程了，停止监控
            monitoring.store(false, Ordering::Relaxed);
            break;
        }
    }

    tracing::info!("Monitor loop exited");
}

// ── 配置管理 ──────────────────────────────────────────────

// ── 公开查询方法 ──────────────────────────────────────────

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
}

// ── Onboard：登录后初始化 ─────────────────────────────────

/// Onboard 请求（前端登录成功后发送）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardRequest {
    pub llm_token: String,
    pub user_nickname: Option<String>,
    pub user_uuid: Option<String>,
    pub api_base_url: Option<String>,
    /// LLM 网关地址（含 /v1 后缀），如 http://127.0.0.1:3180/v1
    pub llm_gateway_url: Option<String>,
}

/// Onboard 结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardResult {
    pub success: bool,
    pub config_created: bool,
    pub agent_created: bool,
    pub sidecar_started: bool,
    pub error: Option<String>,
}

impl SidecarManager {
    /// 执行 onboard 流程：
    /// 1. 创建 ~/.huanxing/ 目录结构
    /// 2. 从模板生成 config.toml
    /// 3. 创建默认 agent 配置
    /// 4. 创建完整 workspace（从 workspace-scaffold/ 复制 + 占位符替换）
    /// 5. 生成 secret key
    /// 6. 启动 sidecar
    pub async fn onboard(
        &self,
        req: OnboardRequest,
        app: AppHandle,
    ) -> Result<OnboardResult, String> {
        let mut result = OnboardResult {
            success: false,
            config_created: false,
            agent_created: false,
            sidecar_started: false,
            error: None,
        };

        let star_name = req.user_nickname.as_deref().unwrap_or("小星");
        let nickname = req.user_nickname.as_deref().unwrap_or("主人");
        let user_uuid = req.user_uuid.as_deref().unwrap_or("unknown");

        // 1. 创建目录结构
        std::fs::create_dir_all(&self.config_dir).map_err(|e| format!("创建配置目录失败: {e}"))?;
        let workspace_dir = self.config_dir.join("agents").join("default");
        std::fs::create_dir_all(&workspace_dir).ok();
        std::fs::create_dir_all(self.config_dir.join("agents")).ok();

        // 2. 生成 config.toml
        let api_base = req
            .api_base_url
            .as_deref()
            .unwrap_or("http://127.0.0.1:8020");
        // LLM 网关：优先使用前端传入的完整地址，否则从 api_base 派生
        let llm_gateway = req
            .llm_gateway_url
            .as_deref()
            .unwrap_or_else(|| "")
            .to_string();
        let llm_gateway = if llm_gateway.is_empty() {
            format!("{api_base}/api/v1/llm/proxy/v1")
        } else {
            llm_gateway
        };

        let config_content = generate_config_toml(
            &app,
            &req.llm_token,
            &llm_gateway,
            api_base,
            star_name,
            HUANXING_PORT,
        );

        let config_path = self.config_dir.join("config.toml");
        std::fs::write(&config_path, &config_content)
            .map_err(|e| format!("写入配置文件失败: {e}"))?;
        result.config_created = true;
        tracing::info!("Config created: {}", config_path.display());

        // 3. 创建默认 agent 配置
        let agent_dir = self.config_dir.join("agents").join("default");
        std::fs::create_dir_all(&agent_dir).ok();
        // 写入 agent 级别的 config.toml
        let agent_config = format!(
            r#"# 默认 Agent 配置 — 唤星桌面端自动生成
[agent]
name = "default"
template = "assistant"
display_name = "{star_name}"
hasn_id = ""
"#,
            star_name = star_name,
        );
        let agent_config_path = agent_dir.join("config.toml");
        if !agent_config_path.exists() {
            std::fs::write(&agent_config_path, &agent_config).ok();
            tracing::info!("Agent config created: {}", agent_config_path.display());
        }

        // 4. 创建完整 workspace — 从 workspace-scaffold/ 模板复制 + 占位符替换
        let now = chrono_now_pretty();
        let comm_style = "温暖、自然、简洁。适当使用 emoji（最多 1-2 个），避免机械化措辞。";
        let placeholders: &[(&str, &str)] = &[
            ("{{nickname}}", nickname),
            ("{{star_name}}", star_name),
            ("{{user_id}}", user_uuid),
            ("{{created_at}}", &now),
            ("{{comm_style}}", comm_style),
        ];

        let scaffold_result = scaffold_workspace(&app, &workspace_dir, placeholders);
        match scaffold_result {
            Ok(count) => {
                tracing::info!(
                    "Workspace scaffolded: {count} files created in {}",
                    workspace_dir.display()
                );
            }
            Err(e) => {
                tracing::warn!("Workspace scaffold partial failure: {e}");
                // Not fatal — sidecar can still run without workspace files
            }
        }

        // 5. 生成 secret key（如果不存在）
        let secret_path = self.config_dir.join(".secret_key");
        if !secret_path.exists() {
            use std::io::Write;
            let key: [u8; 32] = rand_bytes();
            let hex: String = key.iter().map(|b| format!("{b:02x}")).collect();
            if let Ok(mut f) = std::fs::File::create(&secret_path) {
                let _ = f.write_all(hex.as_bytes());
            }
        }

        // 6. 启动 sidecar
        match self.start(app).await {
            Ok(status) => {
                result.sidecar_started = true;
                tracing::info!(
                    "Sidecar started after onboard: PID={:?}, port={}",
                    status.pid,
                    status.port
                );
            }
            Err(e) => {
                tracing::warn!("Sidecar start failed after onboard: {e}");
                result.error = Some(format!("配置已创建，但引擎启动失败: {e}"));
            }
        }

        result.success = true;
        Ok(result)
    }
}

/// 从 workspace-scaffold/ 目录复制模板文件到用户 workspace。
///
/// 只复制 .md 文件，跳过 README.md。
/// 对每个文件做占位符替换，已存在的文件不覆盖。
/// 从 workspace-scaffold/ 目录读取单个模板文件
fn load_scaffold_file(app: &AppHandle, filename: &str) -> Option<String> {
    let scaffold_dir = app.path().resource_dir().ok()?.join("workspace-scaffold");

    let scaffold_dir = if scaffold_dir.exists() {
        scaffold_dir
    } else {
        let dev_path =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("workspace-scaffold");
        if dev_path.exists() {
            dev_path
        } else {
            return None;
        }
    };

    std::fs::read_to_string(scaffold_dir.join(filename)).ok()
}

fn scaffold_workspace(
    app: &AppHandle,
    workspace_dir: &std::path::Path,
    placeholders: &[(&str, &str)],
) -> Result<usize, String> {
    // 定位 workspace-scaffold 目录（在 app 资源目录中）
    let scaffold_dir = app
        .path()
        .resource_dir()
        .map_err(|e| format!("获取资源目录失败: {e}"))?
        .join("workspace-scaffold");

    // Fallback: 开发模式下直接用相对路径
    let scaffold_dir = if scaffold_dir.exists() {
        scaffold_dir
    } else {
        let dev_path =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("workspace-scaffold");
        if dev_path.exists() {
            dev_path
        } else {
            return Err(format!(
                "workspace-scaffold 目录不存在: {} 或 {}",
                scaffold_dir.display(),
                dev_path.display()
            ));
        }
    };

    tracing::info!("Scaffold source: {}", scaffold_dir.display());

    let mut count = 0;
    let entries =
        std::fs::read_dir(&scaffold_dir).map_err(|e| format!("读取 scaffold 目录失败: {e}"))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("读取目录条目失败: {e}"))?;
        let file_name = entry.file_name().to_string_lossy().to_string();

        // 只处理 .md 文件，跳过 README.md
        if !file_name.ends_with(".md") || file_name == "README.md" {
            continue;
        }

        let dest = workspace_dir.join(&file_name);

        // 已存在的文件不覆盖（用户可能已修改）
        if dest.exists() {
            tracing::debug!("Skipping existing: {}", dest.display());
            continue;
        }

        // 读取模板 + 替换占位符
        let content = std::fs::read_to_string(entry.path())
            .map_err(|e| format!("读取模板 {file_name} 失败: {e}"))?;

        let mut content = content;
        for (placeholder, value) in placeholders {
            content = content.replace(placeholder, value);
        }

        std::fs::write(&dest, &content).map_err(|e| format!("写入 {file_name} 失败: {e}"))?;

        tracing::info!("Created workspace file: {}", dest.display());
        count += 1;
    }

    Ok(count)
}

/// 生成随机 32 字节
fn rand_bytes() -> [u8; 32] {
    let mut buf = [0u8; 32];
    // 简单的随机源：时间戳 + PID 混合
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id() as u128;
    let mix = seed ^ (pid << 64);
    for (i, b) in buf.iter_mut().enumerate() {
        *b = ((mix >> (i % 16 * 8)) & 0xFF) as u8 ^ (i as u8).wrapping_mul(37);
    }
    buf
}

/// 配置模板 — 唤星桌面端专用
fn generate_config_toml(
    app: &tauri::AppHandle,
    llm_token: &str,
    llm_gateway: &str,
    api_base: &str,
    agent_name: &str,
    port: u16,
) -> String {
    // 尝试从 workspace-scaffold/config.toml.template 读取模板
    let template = load_scaffold_file(app, "config.toml.template").unwrap_or_default();

    if template.is_empty() {
        tracing::warn!("config.toml.template not found, using inline fallback");
        // 内联回退（最简配置）
        return format!(
            r#"# 唤星桌面端配置 — 自动生成（回退模板）
display_name = "{agent_name}"
default_provider = "custom:{llm_gateway_base}"
default_model = "claude-sonnet-4-6"
title_model = "claude-haiku-4-5"
default_temperature = 0.7

[memory]
backend = "sqlite"
auto_save = true

[gateway]
port = {port}
host = "127.0.0.1"
require_pairing = false

[huanxing]
enabled = true
api_base_url = "{api_base}"

[workspace]
enabled = true
workspaces_dir = "~/.huanxing/agents"

[runtime]
kind = "native"
"#,
            agent_name = agent_name,
            llm_gateway_base = llm_gateway.trim_end_matches("/v1"),
            api_base = api_base,
            port = port,
        );
    }

    let llm_gateway_base = llm_gateway.trim_end_matches("/v1");
    template
        .replace("{{timestamp}}", &chrono_now())
        .replace("{{star_name}}", agent_name)
        .replace("{{llm_token}}", llm_token)
        .replace("{{llm_gateway}}", llm_gateway)
        .replace("{{llm_gateway_base}}", llm_gateway_base)
        .replace("{{api_base}}", api_base)
        .replace("{{port}}", &port.to_string())
}

/// 简易时间戳（不依赖 chrono crate）
fn chrono_now() -> String {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("unix:{}", d.as_secs())
}

/// 人类可读的时间戳（不依赖 chrono crate）
fn chrono_now_pretty() -> String {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = d.as_secs();
    // Simple UTC date — good enough for a placeholder
    let days = secs / 86400;
    let mut y = 1970i64;
    let mut remaining = days as i64;
    loop {
        let days_in_year = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
            366
        } else {
            365
        };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let month_days: [i64; 12] = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut m = 0;
    for &md in &month_days {
        if remaining < md {
            break;
        }
        remaining -= md;
        m += 1;
    }
    let day = remaining + 1;
    let hour = (secs % 86400) / 3600;
    let min = (secs % 3600) / 60;
    format!("{y}-{:02}-{day:02} {hour:02}:{min:02} UTC", m + 1)
}

// ── 配置管理 ──────────────────────────────────────────────

/// 快捷配置项（前端可修改的字段）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuickConfig {
    pub default_model: Option<String>,
    pub default_temperature: Option<f64>,
    pub autonomy_level: Option<String>,
    pub gateway_port: Option<u16>,
}

impl SidecarManager {
    /// 读取 config.toml 中的快捷配置
    pub fn read_config(&self) -> Result<QuickConfig, String> {
        let config_path = self.config_dir.join("config.toml");
        let content =
            std::fs::read_to_string(&config_path).map_err(|e| format!("读取配置文件失败: {e}"))?;

        let table: toml::Table = content
            .parse()
            .map_err(|e| format!("解析 TOML 失败: {e}"))?;

        Ok(QuickConfig {
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
    pub fn update_config(&self, updates: QuickConfig) -> Result<(), String> {
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

    /// 检查是否有残留的 sidecar 进程（通过 PID 文件或端口占用）
    #[allow(dead_code)]
    pub fn check_leftover(&self) -> Option<u32> {
        // 1. 检查 PID 文件
        let pid_path = self.config_dir.join(".sidecar.pid");
        if let Ok(content) = std::fs::read_to_string(&pid_path) {
            if let Ok(pid) = content.trim().parse::<u32>() {
                // 检查进程是否存活
                #[cfg(unix)]
                {
                    let alive = unsafe { libc::kill(pid as i32, 0) } == 0;
                    if alive {
                        return Some(pid);
                    }
                }
            }
        }

        // 2. 检查端口占用
        let port = self.port.load(Ordering::Relaxed) as u16;
        if !Self::is_port_available(port) {
            tracing::info!("Port {port} is occupied, possible leftover sidecar");
            // 无法确定 PID，返回 0 表示有占用但不知道 PID
            return Some(0);
        }

        None
    }

    /// 尝试接管已有的 sidecar（端口上已有进程在跑）
    pub async fn adopt_existing(&self, port: u16) -> bool {
        let client = reqwest::Client::builder()
            .timeout(HEALTH_TIMEOUT)
            .build()
            .unwrap_or_default();

        match client
            .get(format!("http://127.0.0.1:{port}/health"))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!("Adopted existing sidecar on port {port}");
                self.port.store(port as u32, Ordering::Relaxed);
                true
            }
            _ => false,
        }
    }
}
