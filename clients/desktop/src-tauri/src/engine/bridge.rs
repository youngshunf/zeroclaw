//! Tauri Command Bridge — 引擎请求转发层
//!
//! 将前端 `invoke()` 调用转为对 EmbeddedEngine (localhost:PORT) 的 HTTP 请求。
//! 移动端没有独立 sidecar 进程，所有引擎交互都通过此 bridge 进行。

use crate::engine::EmbeddedEngine;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

/// 引擎状态响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineStatus {
    pub running: bool,
    pub port: u16,
    pub healthy: bool,
    pub config_dir: String,
}

/// 通用引擎 HTTP 请求转发
///
/// 前端调用 `invoke('engine_request', { path, method, body })` →
/// bridge 转发为 HTTP 请求到 `localhost:{engine_port}{path}`
#[tauri::command]
pub async fn engine_request(
    state: State<'_, Arc<Mutex<EmbeddedEngine>>>,
    path: String,
    method: Option<String>,
    body: Option<String>,
) -> Result<String, String> {
    let engine = state.lock().await;
    let port = engine.port();

    if !engine.is_running() {
        return Err("引擎未运行".to_string());
    }

    let url = format!("http://127.0.0.1:{port}{path}");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))?;

    let method_str = method.as_deref().unwrap_or("GET");
    let request = match method_str.to_uppercase().as_str() {
        "POST" => {
            let mut req = client.post(&url);
            if let Some(ref b) = body {
                req = req
                    .header("Content-Type", "application/json")
                    .body(b.clone());
            }
            req
        }
        "PUT" => {
            let mut req = client.put(&url);
            if let Some(ref b) = body {
                req = req
                    .header("Content-Type", "application/json")
                    .body(b.clone());
            }
            req
        }
        "DELETE" => client.delete(&url),
        "PATCH" => {
            let mut req = client.patch(&url);
            if let Some(ref b) = body {
                req = req
                    .header("Content-Type", "application/json")
                    .body(b.clone());
            }
            req
        }
        _ => client.get(&url),
    };

    let response = request
        .send()
        .await
        .map_err(|e| format!("引擎请求失败: {e}"))?;

    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|e| format!("读取引擎响应失败: {e}"))?;

    if status.is_success() {
        Ok(text)
    } else {
        Err(format!("引擎返回错误 {status}: {text}"))
    }
}

/// 获取引擎运行状态
#[tauri::command]
pub async fn get_engine_status(
    state: State<'_, Arc<Mutex<EmbeddedEngine>>>,
) -> Result<EngineStatus, String> {
    let engine = state.lock().await;
    let port = engine.port();
    let running = engine.is_running();
    let config_dir = engine.config_dir().to_string_lossy().to_string();

    // 简单 health check（不阻塞太久）
    let healthy = if running {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap_or_default();

        client
            .get(format!("http://127.0.0.1:{port}/health"))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    } else {
        false
    };

    Ok(EngineStatus {
        running,
        port,
        healthy,
        config_dir,
    })
}

/// 重启引擎
#[tauri::command]
pub async fn restart_engine(
    state: State<'_, Arc<Mutex<EmbeddedEngine>>>,
) -> Result<EngineStatus, String> {
    let mut engine = state.lock().await;
    let config_dir = engine.config_dir().to_string_lossy().to_string();
    let port = engine.port();

    // 停止现有引擎
    engine.stop();
    drop(engine);

    // 等待端口释放
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // 启动新引擎
    let new_engine = EmbeddedEngine::start(&config_dir, port)?;
    let running = new_engine.is_running();

    let mut guard = state.lock().await;
    *guard = new_engine;

    Ok(EngineStatus {
        running,
        port,
        healthy: false, // 刚启动，还没通过 health check
        config_dir,
    })
}
