//! WebSocket 事件处理与消息路由
//!
//! 将原 hasn.rs 中庞大的分发系统解耦到本文件

use hasn_client_core::{WsEvent, WsMessagePayload, WsCommand};
use tauri::{AppHandle, Emitter};
use std::sync::Arc;
use crate::commands::hasn::HasnClientState;

/// 处理 WebSocket 事件
pub fn handle_ws_event(event: WsEvent, app: &AppHandle, state: &Arc<HasnClientState>) {
    let sync = &state.sync_engine;
    match event {
        WsEvent::Connected {
            user_hasn_id,
            client_id,
            ..
        } => {
            tracing::info!("[HASN] 已连接: {} ({})", user_hasn_id, client_id);
            let _ = app.emit("hasn:connected", serde_json::json!({
                "user_hasn_id": user_hasn_id,
                "client_id": client_id,
            }));
        }

        WsEvent::ReportAgentsAck { accepted, failed } => {
            tracing::info!(
                "[HASN] Agent 上报: accepted={}, failed={}",
                accepted.len(),
                failed.len()
            );
            let _ = app.emit("hasn:agents_reported", serde_json::json!({
                "accepted": accepted,
                "failed": failed,
            }));
        }

        WsEvent::Message { to_id, message } => {
            // 1. 先写入本地 DB
            let msg = match sync.handle_incoming_message(message) {
                Ok(m) => m,
                Err(e) => {
                    tracing::error!("[HASN] 处理入站消息失败: {e}");
                    return;
                }
            };

            // 2. 通知前端显示
            let _ = app.emit("hasn:message", serde_json::json!({
                "id": msg.id,
                "local_id": msg.local_id,
                "conversation_id": msg.conversation_id,
                "from_id": msg.from_id,
                "from_type": msg.from_type,
                "content": msg.content,
                "content_type": msg.content_type,
                "status": msg.status,
                "created_at": msg.created_at,
                "to_id": to_id,
                "reply_to_id": msg.reply_to,
            }));

            // 3. 发送系统通知（消息来自别人时）
            {
                let content_preview = if msg.content.len() > 50 {
                    format!("{}...", &msg.content[..50])
                } else {
                    msg.content.clone()
                };
                let _ = app.emit("hasn:notification", serde_json::json!({
                    "title": msg.from_id,
                    "body": content_preview,
                }));
            }

            // 4. 路由判断：to_id 是否是本地 Agent？
            if let Some(target_id) = to_id {
                let state = state.clone();
                let target = target_id.clone();
                let from = msg.from_id.clone();
                let conv_id = msg.conversation_id.clone();
                let content = msg.content.clone();
                let ws = state.ws.clone();

                tokio::spawn(async move {
                    let is_local = state.local_agents.read().await.contains(&target);
                    if !is_local {
                        return; // 不是本地 Agent，仅显示，不转发
                    }

                    let port = match *state.sidecar_port.read().await {
                        Some(p) => p,
                        None => {
                            tracing::warn!("[HASN] 本地 Agent 消息但 sidecar_port 未设置");
                            return;
                        }
                    };

                    // 发送 TYPING 状态
                    let typing_cmd = WsCommand::Typing {
                        conversation_id: conv_id.clone(),
                        to_id: from.clone(),
                    };
                    let _ = ws.send_command(&typing_cmd).await;

                    // 调用 Sidecar Agent
                    tracing::info!(
                        "[HASN] 转发消息到本地 Agent: {} -> {} (port={})",
                        from, target, port
                    );
                    
                    // Call the local sidecar using the shared HTTP client in state
                    let client = state.http_client.clone();
                    match invoke_sidecar_agent_with_client(
                        client, port, &target, &conv_id, &from, &content,
                    ).await {
                        Ok(reply) => {
                            // 通过 HASN WS 回复
                            let reply_json = serde_json::json!({"text": &reply});
                            let send_cmd = WsCommand::Send {
                                from_id: Some(target.clone()),
                                to: from.clone(),
                                content: reply_json,
                                content_type: Some(1),
                                msg_type: Some("message".to_string()),
                                local_id: Some(format!("agent_{}", chrono::Utc::now().timestamp_millis())),
                                reply_to_id: None,
                            };
                            if let Err(e) = ws.send_command(&send_cmd).await {
                                tracing::error!("[HASN] Agent 回复发送失败: {e}");
                            } else {
                                tracing::info!("[HASN] Agent 回复已发送 ({} 字符)", reply.len());
                            }
                        }
                        Err(e) => {
                            tracing::error!("[HASN] Sidecar 调用失败: {e}");
                        }
                    }
                });
            }
        }

        WsEvent::OfflineMessages { messages } => {
            tracing::info!("[HASN] 收到 {} 条离线消息", messages.len());
            for raw in messages {
                if let Ok(payload) = serde_json::from_value::<WsMessagePayload>(raw) {
                    if let Ok(msg) = sync.handle_incoming_message(payload) {
                        let _ = app.emit("hasn:message", serde_json::json!({
                            "id": msg.id,
                            "conversation_id": msg.conversation_id,
                            "from_id": msg.from_id,
                            "content": msg.content,
                            "offline": true,
                        }));
                    }
                }
            }
        }

        WsEvent::Ack {
            msg_id,
            conversation_id,
            local_id,
            status,
        } => {
            let _ = sync.handle_ack(msg_id, &conversation_id, local_id.as_deref());
            let _ = app.emit("hasn:ack", serde_json::json!({
                "msg_id": msg_id,
                "conversation_id": conversation_id,
                "local_id": local_id,
                "status": status,
            }));
        }

        WsEvent::Typing {
            from_id,
            conversation_id,
        } => {
            let _ = app.emit("hasn:typing", serde_json::json!({
                "from_id": from_id,
                "conversation_id": conversation_id,
            }));
        }

        WsEvent::Presence { hasn_id, status } => {
            let _ = app.emit("hasn:presence", serde_json::json!({
                "hasn_id": hasn_id,
                "status": status,
            }));
        }

        WsEvent::MessageRecalled {
            msg_id,
            conversation_id,
            recalled_by,
        } => {
            let _ = app.emit("hasn:message_recalled", serde_json::json!({
                "msg_id": msg_id,
                "conversation_id": conversation_id,
                "recalled_by": recalled_by,
            }));
        }

        WsEvent::Pong { .. } => {}

        WsEvent::Error { code, message } => {
            tracing::error!("[HASN] 服务端错误: code={} msg={}", code, message);
            let _ = app.emit("hasn:error", serde_json::json!({
                "code": code,
                "message": message,
            }));
        }

        _ => {}
    }
}

/// 通过共享 HTTP 客户端调用本地 Sidecar 的 hasn-invoke 端点
async fn invoke_sidecar_agent_with_client(
    client: reqwest::Client,
    port: u16,
    hasn_id: &str,
    session_id: &str,
    from_id: &str,
    message: &str,
) -> Result<String, String> {
    let resp = client
        .post(format!("http://127.0.0.1:{port}/api/v1/agent/hasn-invoke"))
        .json(&serde_json::json!({
            "hasn_id": hasn_id,
            "session_id": session_id,
            "from_id": from_id,
            "message": message,
        }))
        .send()
        .await
        .map_err(|e| format!("Sidecar 调用网络异常: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Sidecar 返回 {status}: {body}"));
    }

    let result: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("解析 Sidecar JSON 响应失败: {e}"))?;

    result
        .get("reply")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "Sidecar 响应缺少 reply 字段".to_string())
}
