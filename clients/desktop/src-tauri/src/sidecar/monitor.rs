use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use crate::sidecar::constants::*;
use crate::sidecar::models::{CrashEvent, SidecarEvent};

/// 用于传递给监控循环的句柄（避免直接传 &self）
pub struct SidecarMonitorHandle {
    pub(crate) child: Arc<Mutex<Option<Child>>>,
    pub(crate) port: u16,
    pub(crate) restart_count: *const AtomicU32,
    pub(crate) last_restart: Arc<Mutex<Option<Instant>>>,
    pub(crate) log_buffer: Arc<Mutex<VecDeque<String>>>,
    pub(crate) monitoring: *const AtomicBool,
    pub(crate) stopping: *const AtomicBool,
    pub(crate) bin_path: PathBuf,
    pub(crate) config_dir: String,
}

unsafe impl Send for SidecarMonitorHandle {}

/// 后台监控循环
pub async unsafe fn monitor_loop(handle: SidecarMonitorHandle, app: AppHandle) {
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

        let mut child_guard = handle.child.lock().await;
        if let Some(ref mut child) = *child_guard {
            match child.try_wait() {
                Ok(Some(exit_status)) => {
                    let code = exit_status.code();
                    tracing::warn!("Sidecar exited unexpectedly: code={code:?}");

                    let restart_count_ref = unsafe { &*handle.restart_count };

                    let mut last_restart = handle.last_restart.lock().await;
                    if let Some(last) = *last_restart {
                        if last.elapsed() > RESTART_WINDOW {
                            restart_count_ref.store(0, Ordering::Relaxed);
                        }
                    }

                    let count = restart_count_ref.load(Ordering::Relaxed);
                    let will_restart =
                        count < MAX_AUTO_RESTARTS && !stopping.load(Ordering::Relaxed);

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

                        tokio::time::sleep(Duration::from_secs(1)).await;

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
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!("Error checking sidecar process: {e}");
                }
            }
        } else {
            monitoring.store(false, Ordering::Relaxed);
            break;
        }
    }

    tracing::info!("Monitor loop exited");
}
