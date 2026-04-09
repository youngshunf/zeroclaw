//! WebSocket agent chat handler — 多路复用版本
//!
//! Protocol (v2 — 多 session 版本):
//! ```text
//! Client -> Server: {"type":"message","session_id":"sess_xxx","content":"Hello"}
//! Client -> Server: {"type":"history_request","session_id":"sess_xxx"}
//! Server -> Client: {"type":"session_start","session_id":"sess_xxx","resumed":true}
//! Server -> Client: {"type":"chunk","session_id":"sess_xxx","content":"Hi! "}
//! Server -> Client: {"type":"tool_call","session_id":"sess_xxx","call_id":"c1","name":"shell","display_name":"执行命令","args_preview":"ls"}
//! Server -> Client: {"type":"tool_result","session_id":"sess_xxx","call_id":"c1","status":"success","output_preview":"3 行"}
//! Server -> Client: {"type":"done","session_id":"sess_xxx","full_response":"..."}
//! Server -> Client: {"type":"history","session_id":"sess_xxx","messages":[...]}
//! ```
//!
//! 向后兼容：无 `session_id` 的入站帧会自动路由到连接级默认 session。

use crate::gateway::AppState;
use axum::{
    extract::{
        Query, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderMap, header},
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::collections::HashMap;
use tracing::debug;

/// 入站帧（tagged by "type"）
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum InboundFrame {
    /// 用户发送聊天消息
    Message {
        #[serde(default)]
        session_id: Option<String>,
        #[serde(default)]
        agent: Option<String>,
        #[serde(default)]
        content: String,
    },
    /// 请求某个 session 的历史记录
    HistoryRequest {
        #[serde(default)]
        session_id: Option<String>,
        /// HUANXING: 所属 agent，用于按 per-user workspace 加载隔离的历史
        #[serde(default)]
        agent: Option<String>,
    },
    /// 兼容旧版 connect 握手帧
    Connect {
        #[serde(default)]
        session_id: Option<String>,
        #[serde(default)]
        device_name: Option<String>,
        #[serde(default)]
        capabilities: Vec<String>,
    },
    /// 心跳 ping
    Ping,
}

/// 单连接内一个 session 的状态
struct AgentSession {
    agent: crate::agent::Agent,
    /// 持久化存储 key（格式：gw_{session_id}）
    session_key: String,
    /// Per-session 会话持久化后端。
    ///
    /// In multi-tenant mode this points to the user's workspace directory
    /// (data isolation); in single-tenant mode it mirrors the global
    /// backend from `AppState`.  When `None`, session persistence is
    /// disabled for this session.
    session_backend: Option<std::sync::Arc<dyn crate::channels::session_backend::SessionBackend>>,
    /// Optional observer injected into the agent.
    ///
    /// When multi-tenant is active this is a `WsObserver` that collects
    /// tool-call events for the frontend.  In single-tenant mode it is
    /// `None` (the default observer configured in Agent is used instead).
    ws_observer: Option<std::sync::Arc<dyn crate::observability::Observer>>,
    /// Agent ID / Name associated with this session.
    agent_id: Option<String>,
}

/// The sub-protocol we support for the chat WebSocket.
const WS_PROTOCOL: &str = "zeroclaw.v1";

/// Prefix used in `Sec-WebSocket-Protocol` to carry a bearer token.
const BEARER_SUBPROTO_PREFIX: &str = "bearer.";

/// 持久化 session key 前缀
const GW_SESSION_PREFIX: &str = "gw_";

#[derive(Deserialize)]
pub struct WsQuery {
    pub token: Option<String>,
    /// 连接级默认 session_id（向后兼容旧协议）
    pub session_id: Option<String>,
    /// Optional human-readable name for the session.
    pub name: Option<String>,
}

/// Extract a bearer token from WebSocket-compatible sources.
///
/// Precedence (first non-empty wins):
/// 1. `Authorization: Bearer <token>` header
/// 2. `Sec-WebSocket-Protocol: bearer.<token>` subprotocol
/// 3. `?token=<token>` query parameter
///
/// Browsers cannot set custom headers on `new WebSocket(url)`, so the query
/// parameter and subprotocol paths are required for browser-based clients.
fn extract_ws_token<'a>(headers: &'a HeaderMap, query_token: Option<&'a str>) -> Option<&'a str> {
    // 1. Authorization header
    if let Some(t) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|auth| auth.strip_prefix("Bearer "))
    {
        if !t.is_empty() {
            return Some(t);
        }
    }

    // 2. Sec-WebSocket-Protocol: bearer.<token>
    if let Some(t) = headers
        .get("sec-websocket-protocol")
        .and_then(|v| v.to_str().ok())
        .and_then(|protos| {
            protos
                .split(',')
                .map(|p| p.trim())
                .find_map(|p| p.strip_prefix(BEARER_SUBPROTO_PREFIX))
        })
    {
        if !t.is_empty() {
            return Some(t);
        }
    }

    // 3. ?token= query parameter
    if let Some(t) = query_token {
        if !t.is_empty() {
            return Some(t);
        }
    }

    None
}

/// GET /ws/chat — WebSocket upgrade for agent chat
pub async fn handle_ws_chat(
    State(state): State<AppState>,
    Query(params): Query<WsQuery>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    // Auth: check header, subprotocol, then query param (precedence order)
    if state.pairing.require_pairing() {
        let token = extract_ws_token(&headers, params.token.as_deref()).unwrap_or("");
        if !state.pairing.is_authenticated(token) {
            return (
                axum::http::StatusCode::UNAUTHORIZED,
                "Unauthorized — provide Authorization header, Sec-WebSocket-Protocol bearer, or ?token= query param",
            )
                .into_response();
        }
    }

    // Echo Sec-WebSocket-Protocol if the client requests our sub-protocol.
    let ws = if headers
        .get("sec-websocket-protocol")
        .and_then(|v| v.to_str().ok())
        .map_or(false, |protos| {
            protos.split(',').any(|p| p.trim() == WS_PROTOCOL)
        }) {
        ws.protocols([WS_PROTOCOL])
    } else {
        ws
    };

    // 连接级默认 session_id（向后兼容旧协议 URL 携带 ?session_id=xxx）
    let default_session_id = params.session_id;
    ws.on_upgrade(move |socket| handle_socket(socket, state, default_session_id))
        .into_response()
}

/// 主处理循环——单 WS 连接，内部维护 sessions HashMap
///
/// `default_session_id`：兼容旧协议，URL 携带的 session_id 作为连接级默认值。
async fn handle_socket(socket: WebSocket, state: AppState, default_session_id: Option<String>) {
    let (mut sender, mut receiver) = socket.split();

    // 连接级默认 session_id（无 session_id 帧的兜底）
    let conn_default_sid = default_session_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    // 单连接内的所有 session 状态
    let mut sessions: HashMap<String, AgentSession> = HashMap::new();

    // 发送连接建立确认
    let ack = serde_json::json!({"type": "connected", "message": "Connection established"});
    let _ = sender.send(Message::Text(ack.to_string().into())).await;

    while let Some(msg) = receiver.next().await {
        let text = match msg {
            Ok(Message::Text(t)) => t,
            Ok(Message::Close(_)) | Err(_) => break,
            _ => continue,
        };

        let frame: InboundFrame = match serde_json::from_str(&text) {
            Ok(f) => f,
            Err(_) => continue, // 忽略无法解析的帧
        };

        match frame {
            InboundFrame::Message {
                session_id,
                agent: agent_name,
                content,
            } => {
                if content.is_empty() {
                    continue;
                }
                // 无 session_id 的帧路由到连接级默认 session（向后兼容）
                let sid = session_id.unwrap_or_else(|| conn_default_sid.clone());

                // 获取或初始化该 session 的 Agent
                if !sessions.contains_key(&sid) {
                    match init_agent_session(&state, &sid, agent_name.as_deref()).await {
                        Ok(s) => {
                            // 通知前端 session 状态（已恢复或新建）
                            let resumed = !s.agent.history().is_empty();
                            let msg_count = s.agent.history().len();
                            let start_frame = serde_json::json!({
                                "type": "session_start",
                                "session_id": sid,
                                "resumed": resumed,
                                "message_count": msg_count,
                            });
                            let _ = sender
                                .send(Message::Text(start_frame.to_string().into()))
                                .await;
                            sessions.insert(sid.clone(), s);
                        }
                        Err(e) => {
                            let err = serde_json::json!({
                                "type": "error",
                                "session_id": sid,
                                "message": format!("Failed to initialise agent: {e}"),
                            });
                            let _ = sender.send(Message::Text(err.to_string().into())).await;
                            continue;
                        }
                    }
                }

                let session = sessions.get_mut(&sid).unwrap();

                // Persist user message — prefer per-session backend, fall back to global
                {
                    let backend = session
                        .session_backend
                        .as_ref()
                        .or(state.session_backend.as_ref());
                    if let Some(b) = backend {
                        let user_msg = crate::providers::ChatMessage::user(&content);
                        let _ = b.append(&session.session_key, &user_msg);
                    }
                }

                process_chat_message(&state, session, &mut sender, &content, &sid).await;
            }

            InboundFrame::HistoryRequest {
                session_id,
                agent: agent_name,
            } => {
                let sid = session_id.unwrap_or_else(|| conn_default_sid.clone());
                let session_key = format!("{GW_SESSION_PREFIX}{sid}");

                // Load history — prefer per-session backend from active
                // session cache, or resolve from agent workspace; fall
                // back to the global session backend.
                let messages: Vec<crate::providers::ChatMessage> = {
                    let backend = if let Some(sess) = sessions.get(&sid) {
                        // Session already initialised — reuse its backend
                        sess.session_backend
                            .clone()
                            .or_else(|| state.session_backend.clone())
                    } else {
                        // No active session — resolve the same TenantContext path used by init
                        let config = state.config.lock().clone();
                        match resolve_tenant_context(agent_name.as_deref(), &config).await {
                            Ok(Some(tenant)) => tenant
                                .session_manager
                                .clone()
                                .or_else(|| state.session_backend.clone()),
                            Ok(None) | Err(_) => state.session_backend.clone(),
                        }
                    };
                    backend.map(|b| b.load(&session_key)).unwrap_or_default()
                };

                let history_frame = serde_json::json!({
                    "type": "history",
                    "session_id": sid,
                    "messages": messages,
                });
                let _ = sender
                    .send(Message::Text(history_frame.to_string().into()))
                    .await;
            }

            InboundFrame::Connect {
                session_id,
                device_name,
                capabilities,
            } => {
                // 兼容旧版 connect 握手，仅 debug 日志
                debug!(
                    session_id = ?session_id,
                    device_name = ?device_name,
                    capabilities = ?capabilities,
                    "WebSocket connect params received (legacy)"
                );
                let ack =
                    serde_json::json!({"type": "connected", "message": "Connection established"});
                let _ = sender.send(Message::Text(ack.to_string().into())).await;
            }

            InboundFrame::Ping => {
                let pong = serde_json::json!({"type": "pong"});
                let _ = sender.send(Message::Text(pong.to_string().into())).await;
            }
        }
    }
}

async fn resolve_tenant_context(
    agent_name: Option<&str>,
    config: &crate::config::Config,
) -> anyhow::Result<Option<crate::huanxing::TenantContext>> {
    let Some(agent_name) = agent_name else {
        tracing::warn!(
            "[hx_ws] resolve_tenant_context: agent_name is None, \
             global config api_key present={}",
            config.api_key.is_some()
        );
        return Ok(None);
    };
    tracing::info!(
        "[hx_ws] resolve_tenant_context: looking up agent_name={:?}, \
         huanxing.enabled={}",
        agent_name,
        config.huanxing.enabled
    );
    let result = crate::huanxing::TenantContext::load_by_agent_or_hasn(config, agent_name).await;
    match &result {
        Ok(Some(ctx)) => tracing::info!(
            "[hx_ws] resolve_tenant_context: found tenant, \
             agent_id={}, has_api_key={}, provider={:?}, model={:?}",
            ctx.agent_id,
            ctx.api_key.is_some(),
            ctx.provider,
            ctx.model,
        ),
        Ok(None) => tracing::warn!(
            "[hx_ws] resolve_tenant_context: NO tenant found for {:?}",
            agent_name
        ),
        Err(e) => tracing::error!(
            "[hx_ws] resolve_tenant_context: error loading tenant {:?}: {}",
            agent_name, e
        ),
    }
    result
}

/// 初始化一个新的 AgentSession，从持久化存储恢复历史。
///
/// When multi-tenant is active and `agent_name` resolves to a valid
/// workspace, the session uses an isolated Agent (separate system prompt,
/// memory, skills) and a per-workspace session backend for data isolation.
async fn init_agent_session(
    state: &AppState,
    session_id: &str,
    agent_name: Option<&str>,
) -> anyhow::Result<AgentSession> {
    let config = state.config.lock().clone();

    let tenant_context = resolve_tenant_context(agent_name, &config).await?;

    // Create per-agent session backend (or fall back to global)
    let per_user_backend = if let Some(ref tenant) = tenant_context {
        tenant
            .session_manager
            .clone()
            .or_else(|| state.session_backend.clone())
    } else {
        state.session_backend.clone()
    };

    // Create Agent — use TenantContext when available
    let mut agent = if let Some(ref tenant) = tenant_context {
        tracing::info!(
            "[hx_ws] Creating agent from TenantContext: api_key={}, \
             resolved_config.api_key={}",
            tenant.api_key.is_some(),
            tenant.runtime_config().api_key.is_some(),
        );
        tenant.create_agent().await?
    } else {
        tracing::warn!(
            "[hx_ws] Creating agent from GLOBAL config (no tenant): \
             api_key={}",
            config.api_key.is_some(),
        );
        crate::agent::Agent::from_config(&config).await?
    };
    agent.set_memory_session_id(Some(session_id.to_string()));

    let session_key = format!("{GW_SESSION_PREFIX}{session_id}");

    // Restore history from persistent storage
    {
        let backend = per_user_backend.as_ref().or(state.session_backend.as_ref());
        if let Some(b) = backend {
            let messages = b.load(&session_key);
            if !messages.is_empty() {
                agent.seed_history(&messages);
            }
        }
    }

    // Inject WsObserver to collect tool-call events for the frontend
    let ws_observer: Option<std::sync::Arc<dyn crate::observability::Observer>> = {
        let obs = std::sync::Arc::new(crate::huanxing::ws_observer::WsObserver::new());
        agent.set_observer(obs.clone());
        Some(obs)
    };

    Ok(AgentSession {
        agent,
        session_key,
        session_backend: per_user_backend,
        ws_observer,
        agent_id: tenant_context
            .as_ref()
            .map(|tenant| tenant.agent_id.clone())
            .or_else(|| agent_name.map(|s| s.to_string())),
    })
}

/// 处理单条消息并回复，所有出站帧携带 session_id
///
/// 使用 `turn_streamed` 实现流式输出：LLM 生成的每个 chunk 都会立即
/// 通过 WebSocket 发送给前端，而不是等整个回复生成完毕。
async fn process_chat_message(
    state: &AppState,
    session: &mut AgentSession,
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    content: &str,
    session_id: &str,
) {
    let provider_label = state
        .config
        .lock()
        .default_provider
        .clone()
        .unwrap_or_else(|| "unknown".to_string());

    // 广播 agent_start 事件（内部监控用）
    let _ = state.event_tx.send(serde_json::json!({
        "type": "agent_start",
        "provider": provider_label,
        "model": state.model,
    }));

    // ── 使用 turn_streamed 实现流式输出 ──────────────────────────
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<crate::agent::TurnEvent>(64);
    let session_id_owned = session_id.to_string();
    let agent_id = session.agent_id.clone();

    // 在后台 task 中运行 turn_streamed
    let turn_handle = {
        // SAFETY: 我们需要 &mut Agent，但 tokio::spawn 要求 'static。
        // 这里用 unsafe 延长生命周期——实际上我们会在 handle.await 后再访问 session，
        // 所以不存在数据竞争。
        //
        // 但为了避免 unsafe，改用 tokio::task::spawn_blocking 是不行的（Agent 不是 Send）。
        // 更好的做法是在当前 task 中交替 poll turn_streamed future 和 relay events。
        //
        // 实际上，最安全的做法是直接在当前 async 上下文中驱动 turn_streamed，
        // 并用 select! 同时处理 event_rx。但 turn_streamed 需要 &mut self，
        // 所以我们不能同时 borrow session 和 sender。
        //
        // 最终方案：直接在 select! 中同时驱动 turn future 和 relay events。
        event_tx
    };

    // 使用 pin! 来 pin future，这样可以在 select! 中重复 poll
    let turn_future = session.agent.turn_streamed(content, turn_handle);
    tokio::pin!(turn_future);

    let mut tool_call_counter = 0u32;
    let mut turn_result: Option<Result<String, anyhow::Error>> = None;
    let mut progress_lines: Vec<String> = Vec::new();

    let mut ping_interval = tokio::time::interval(std::time::Duration::from_secs(15));
    ping_interval.tick().await; // consume the first immediate tick

    loop {
        tokio::select! {
            // Send periodic WebSocket Ping to keep proxy/load balancer connection alive
            _ = ping_interval.tick() => {
                let _ = sender.send(Message::Ping(vec![].into())).await;
            }
            // Poll turn_streamed future
            result = &mut turn_future, if turn_result.is_none() => {
                turn_result = Some(result);
            }
            // Relay events to WebSocket
            event = event_rx.recv() => {
                match event {
                    Some(crate::agent::TurnEvent::Chunk { delta }) => {
                        let frame = serde_json::json!({
                            "type": "chunk",
                            "agent": agent_id,
                            "session_id": session_id_owned,
                            "content": delta,
                        });
                        let _ = sender.send(Message::Text(frame.to_string().into())).await;
                    }
                    Some(crate::agent::TurnEvent::Thinking { delta }) => {
                        // 累积思考内容到 progress_lines
                        for line in delta.lines() {
                            let trimmed = line.trim();
                            if !trimmed.is_empty() {
                                progress_lines.push(trimmed.to_string());
                            }
                        }
                        let frame = serde_json::json!({
                            "type": "thinking",
                            "agent": agent_id,
                            "session_id": session_id_owned,
                            "content": delta,
                        });
                        let _ = sender.send(Message::Text(frame.to_string().into())).await;
                    }
                    Some(crate::agent::TurnEvent::ToolCall { name, args }) => {
                        let call_id = format!("c{tool_call_counter}_{name}");
                        tool_call_counter += 1;
                        progress_lines.push(format!("\u{1f527} 调用工具: {name}"));
                        let frame = serde_json::json!({
                            "type": "tool_call",
                            "agent": agent_id,
                            "session_id": session_id_owned,
                            "call_id": call_id,
                            "name": name,
                            "display_name": name,
                            "args_preview": serde_json::to_string(&args).unwrap_or_default(),
                        });
                        let _ = sender.send(Message::Text(frame.to_string().into())).await;
                    }
                    Some(crate::agent::TurnEvent::ToolResult { name, output }) => {
                        let call_id = format!("c{}_{name}", tool_call_counter.saturating_sub(1));
                        let preview = if output.len() > 200 {
                            format!("{}...", &output[..200])
                        } else {
                            output
                        };
                        progress_lines.push("\u{2705} 工具执行完成".to_string());
                        let frame = serde_json::json!({
                            "type": "tool_result",
                            "agent": agent_id,
                            "session_id": session_id_owned,
                            "call_id": call_id,
                            "status": "success",
                            "output_preview": preview,
                        });
                        let _ = sender.send(Message::Text(frame.to_string().into())).await;
                    }
                    None => {
                        // Channel closed — turn_streamed has completed and dropped tx
                        break;
                    }
                }
            }
        }

        // If turn completed AND channel is drained, break
        if turn_result.is_some() && event_rx.is_empty() {
            // Drain any remaining events
            while let Ok(event) = event_rx.try_recv() {
                match event {
                    crate::agent::TurnEvent::Chunk { delta } => {
                        let frame = serde_json::json!({
                            "type": "chunk",
                            "agent": agent_id,
                            "session_id": session_id_owned,
                            "content": delta,
                        });
                        let _ = sender.send(Message::Text(frame.to_string().into())).await;
                    }
                    crate::agent::TurnEvent::Thinking { delta } => {
                        for line in delta.lines() {
                            let trimmed = line.trim();
                            if !trimmed.is_empty() {
                                progress_lines.push(trimmed.to_string());
                            }
                        }
                        let frame = serde_json::json!({
                            "type": "thinking",
                            "agent": agent_id,
                            "session_id": session_id_owned,
                            "content": delta,
                        });
                        let _ = sender.send(Message::Text(frame.to_string().into())).await;
                    }
                    crate::agent::TurnEvent::ToolCall { name, args } => {
                        let call_id = format!("c{tool_call_counter}_{name}");
                        tool_call_counter += 1;
                        progress_lines.push(format!("\u{1f527} 调用工具: {name}"));
                        let frame = serde_json::json!({
                            "type": "tool_call",
                            "agent": agent_id,
                            "session_id": session_id_owned,
                            "call_id": call_id,
                            "name": name,
                            "display_name": name,
                            "args_preview": serde_json::to_string(&args).unwrap_or_default(),
                        });
                        let _ = sender.send(Message::Text(frame.to_string().into())).await;
                    }
                    crate::agent::TurnEvent::ToolResult { name, output } => {
                        let call_id = format!("c{}_{name}", tool_call_counter.saturating_sub(1));
                        let preview = if output.len() > 200 {
                            format!("{}...", &output[..200])
                        } else {
                            output
                        };
                        progress_lines.push("\u{2705} 工具执行完成".to_string());
                        let frame = serde_json::json!({
                            "type": "tool_result",
                            "agent": agent_id,
                            "session_id": session_id_owned,
                            "call_id": call_id,
                            "status": "success",
                            "output_preview": preview,
                        });
                        let _ = sender.send(Message::Text(frame.to_string().into())).await;
                    }
                }
            }
            break;
        }
    }

    // ── 处理 turn 结果 ──────────────────────────────────────────
    match turn_result.unwrap_or_else(|| Err(anyhow::anyhow!("turn was not completed"))) {
        Ok(response) => {
            // Persist assistant reply
            {
                let backend = session
                    .session_backend
                    .as_ref()
                    .or(state.session_backend.as_ref());
                if let Some(b) = backend {
                    let assistant_msg = crate::providers::ChatMessage::assistant(&response);
                    let _ = b.append(&session.session_key, &assistant_msg);
                }
            }

            let done = serde_json::json!({
                "type": "done",
                "agent": session.agent_id,
                "session_id": session_id,
                "full_response": response,
                "progress_lines": progress_lines,
            });
            let _ = sender.send(Message::Text(done.to_string().into())).await;

            let _ = state.event_tx.send(serde_json::json!({
                "type": "agent_end",
                "provider": provider_label,
                "model": state.model,
            }));
        }
        Err(e) => {
            let sanitized = crate::providers::sanitize_api_error(&e.to_string());
            let err = serde_json::json!({
                "type": "error",
                "session_id": session_id,
                "message": sanitized,
            });
            let _ = sender.send(Message::Text(err.to_string().into())).await;

            let _ = state.event_tx.send(serde_json::json!({
                "type": "error",
                "component": "ws_chat",
                "message": sanitized,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;

    #[test]
    fn extract_ws_token_from_authorization_header() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer zc_test123".parse().unwrap());
        assert_eq!(extract_ws_token(&headers, None), Some("zc_test123"));
    }

    #[test]
    fn extract_ws_token_from_subprotocol() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "sec-websocket-protocol",
            "zeroclaw.v1, bearer.zc_sub456".parse().unwrap(),
        );
        assert_eq!(extract_ws_token(&headers, None), Some("zc_sub456"));
    }

    #[test]
    fn extract_ws_token_from_query_param() {
        let headers = HeaderMap::new();
        assert_eq!(
            extract_ws_token(&headers, Some("zc_query789")),
            Some("zc_query789")
        );
    }

    #[test]
    fn extract_ws_token_precedence_header_over_subprotocol() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer zc_header".parse().unwrap());
        headers.insert("sec-websocket-protocol", "bearer.zc_sub".parse().unwrap());
        assert_eq!(
            extract_ws_token(&headers, Some("zc_query")),
            Some("zc_header")
        );
    }

    #[test]
    fn extract_ws_token_precedence_subprotocol_over_query() {
        let mut headers = HeaderMap::new();
        headers.insert("sec-websocket-protocol", "bearer.zc_sub".parse().unwrap());
        assert_eq!(extract_ws_token(&headers, Some("zc_query")), Some("zc_sub"));
    }

    #[test]
    fn extract_ws_token_returns_none_when_empty() {
        let headers = HeaderMap::new();
        assert_eq!(extract_ws_token(&headers, None), None);
    }

    #[test]
    fn extract_ws_token_skips_empty_header_value() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer ".parse().unwrap());
        assert_eq!(
            extract_ws_token(&headers, Some("zc_fallback")),
            Some("zc_fallback")
        );
    }

    #[test]
    fn extract_ws_token_skips_empty_query_param() {
        let headers = HeaderMap::new();
        assert_eq!(extract_ws_token(&headers, Some("")), None);
    }

    #[test]
    fn extract_ws_token_subprotocol_with_multiple_entries() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "sec-websocket-protocol",
            "zeroclaw.v1, bearer.zc_tok, other".parse().unwrap(),
        );
        assert_eq!(extract_ws_token(&headers, None), Some("zc_tok"));
    }
}
