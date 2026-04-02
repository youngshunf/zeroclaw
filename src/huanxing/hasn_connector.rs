//! HASN 连接管理器 — 运行在 ZeroClaw Sidecar 进程内
//!
//! 所有节点（桌面端/云端）共用同一套代码。
//! 负责：
//! - 管理到 HASN 中央节点的 WS 长连接
//! - REPORT_AGENTS 上报本地 Agent
//! - 处理入站消息：to_id ∈ local_agents → 进程内 agent_bridge.invoke()
//! - 事件广播到订阅者（供前端 /ws/hasn-events 消费）

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use tokio::sync::{broadcast, RwLock};
use tracing::{error, info, warn};

use hasn_client_core::model::{AgentReport, WsCommand, WsEvent};
use hasn_client_core::ws::HasnWsClient;

use crate::gateway::AppState;
use crate::huanxing::agent_bridge;
use crate::huanxing::db::TenantDb;

/// HASN 事件（广播到前端 /ws/hasn-events）
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type")]
pub enum HasnEvent {
    /// 连接状态变化
    #[serde(rename = "connected")]
    Connected {
        node_id: String,
        node_type: String,
    },
    /// 连接断开
    #[serde(rename = "disconnected")]
    Disconnected,
    /// HASN 消息（推送给前端展示）
    #[serde(rename = "message")]
    Message {
        payload: serde_json::Value,
    },
    /// Agent 上报结果
    #[serde(rename = "report_ack")]
    ReportAck {
        accepted: Vec<String>,
        failed: Vec<serde_json::Value>,
    },
    /// ACK 回执
    #[serde(rename = "ack")]
    Ack {
        msg_id: i64,
        conversation_id: String,
        local_id: Option<String>,
    },
    /// 对方正在输入
    #[serde(rename = "typing")]
    Typing {
        from_id: String,
        conversation_id: String,
    },
    /// 离线消息
    #[serde(rename = "offline_messages")]
    OfflineMessages {
        messages: Vec<serde_json::Value>,
    },
}

/// HASN Agent 专属多路复用会话状态
pub struct HasnAgentSession {
    pub agent: tokio::sync::Mutex<crate::agent::Agent>,
    pub session_key: String,
    pub session_backend: Option<std::sync::Arc<dyn crate::channels::session_backend::SessionBackend>>,
}

/// HASN 连接管理器
pub struct HasnConnector {
    /// hasn-client-core 的 WS 客户端
    ws: Arc<HasnWsClient>,
    /// 本节点的 node_id（连接成功后由服务端返回）
    node_id: Arc<RwLock<Option<String>>>,
    /// 本节点上报的 Agent hasn_id 集合
    local_agents: Arc<RwLock<HashSet<String>>>,
    /// 事件广播通道
    event_tx: broadcast::Sender<HasnEvent>,
    /// 是否已连接
    connected: Arc<RwLock<bool>>,
    /// 状态化会话: conversation_id -> HasnAgentSession
    sessions: Arc<RwLock<HashMap<String, Arc<HasnAgentSession>>>>,
}

impl HasnConnector {
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(256);
        Self {
            ws: Arc::new(HasnWsClient::new()),
            node_id: Arc::new(RwLock::new(None)),
            local_agents: Arc::new(RwLock::new(HashSet::new())),
            event_tx,
            connected: Arc::new(RwLock::new(false)),
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 连接 HASN 中央节点
    pub async fn connect(
        &self,
        central_url: &str,
        state: Arc<AppState>,
    ) -> anyhow::Result<()> {
        info!("[HASN] 连接中央节点: {}", &central_url[..central_url.find('?').unwrap_or(central_url.len())]);

        let event_tx = self.event_tx.clone();
        let node_id = self.node_id.clone();
        let local_agents = self.local_agents.clone();
        let connected = self.connected.clone();
        let ws_ref = self.ws.clone();
        let sessions = self.sessions.clone();

        self.ws
            .connect(central_url, move |event| {
                let event_tx = event_tx.clone();
                let node_id = node_id.clone();
                let local_agents = local_agents.clone();
                let connected = connected.clone();
                let sessions = sessions.clone();
                let state = state.clone();
                let ws_ref = ws_ref.clone();

                // 在 tokio runtime 中异步处理事件
                tokio::spawn(async move {
                    handle_ws_event(
                        event, &event_tx, &node_id, &local_agents,
                        &connected, &sessions, &state, ws_ref,
                    ).await;
                });
            })
            .await
            .map_err(|e| anyhow::anyhow!("HASN 连接失败: {}", e))?;

        Ok(())
    }

    /// 带重试的连接
    pub async fn connect_with_retry(
        &self,
        central_url: &str,
        max_retries: u32,
        state: Arc<AppState>,
    ) -> anyhow::Result<()> {
        let event_tx = self.event_tx.clone();
        let node_id = self.node_id.clone();
        let local_agents = self.local_agents.clone();
        let connected = self.connected.clone();
        let ws_ref = self.ws.clone();
        let sessions = self.sessions.clone();

        self.ws
            .connect_with_retry(
                central_url,
                move |event| {
                    let event_tx = event_tx.clone();
                    let node_id = node_id.clone();
                    let local_agents = local_agents.clone();
                    let connected = connected.clone();
                    let sessions = sessions.clone();
                    let state = state.clone();
                    let ws_ref = ws_ref.clone();

                    tokio::spawn(async move {
                        handle_ws_event(
                            event, &event_tx, &node_id, &local_agents,
                            &connected, &sessions, &state, ws_ref,
                        ).await;
                    });
                },
                max_retries,
            )
            .await
            .map_err(|e| anyhow::anyhow!("HASN 重连失败: {}", e))?;

        Ok(())
    }

    /// 断开连接
    pub async fn disconnect(&self) {
        self.ws.disconnect().await;
        *self.connected.write().await = false;
        *self.node_id.write().await = None;
        let _ = self.event_tx.send(HasnEvent::Disconnected);
        info!("[HASN] 已断开");
    }

    /// 发送消息
    pub async fn send_message(
        &self,
        to: &str,
        content: serde_json::Value,
        from_id: Option<String>,
        local_id: Option<String>,
    ) -> anyhow::Result<()> {
        let cmd = WsCommand::Send {
            from_id,
            to: to.to_string(),
            content,
            content_type: Some(1),
            msg_type: None,
            local_id,
            reply_to_id: None,
        };
        self.ws.send_command(&cmd).await
            .map_err(|e| anyhow::anyhow!("HASN 发送失败: {}", e))
    }

    /// 上报 Agent 列表
    pub async fn report_agents(&self, agents: Vec<AgentReport>) -> anyhow::Result<()> {
        // 记录到本地集合
        {
            let mut local = self.local_agents.write().await;
            local.clear();
            for a in &agents {
                local.insert(a.hasn_id.clone());
            }
        }

        let cmd = WsCommand::ReportAgents { agents };
        self.ws.send_command(&cmd).await
            .map_err(|e| anyhow::anyhow!("HASN 上报 Agent 失败: {}", e))
    }

    /// 订阅事件流
    pub fn subscribe(&self) -> broadcast::Receiver<HasnEvent> {
        self.event_tx.subscribe()
    }

    /// 获取连接状态
    pub async fn is_connected(&self) -> bool {
        *self.connected.read().await
    }

    /// 获取 node_id
    pub async fn get_node_id(&self) -> Option<String> {
        self.node_id.read().await.clone()
    }
}

/// 处理入站 WS 事件
async fn handle_ws_event(
    event: WsEvent,
    event_tx: &broadcast::Sender<HasnEvent>,
    node_id: &RwLock<Option<String>>,
    local_agents: &RwLock<HashSet<String>>,
    connected: &RwLock<bool>,
    sessions: &RwLock<HashMap<String, Arc<HasnAgentSession>>>,
    state: &AppState,
    ws: Arc<HasnWsClient>,
) {
    match event {
        WsEvent::Connected {
            node_id: nid,
            node_type,
            ..
        } => {
            *node_id.write().await = Some(nid.clone());
            *connected.write().await = true;
            info!("[HASN] 已连接: node_id={}, type={}", nid, node_type);
            let _ = event_tx.send(HasnEvent::Connected {
                node_id: nid,
                node_type,
            });
        }

        WsEvent::ReportAgentsAck { accepted, failed } => {
            info!("[HASN] Agent 上报: accepted={}, failed={}", accepted.len(), failed.len());
            let failed_json: Vec<serde_json::Value> = failed
                .iter()
                .map(|f| serde_json::json!({"hasn_id": f.hasn_id, "reason": f.reason}))
                .collect();
            let _ = event_tx.send(HasnEvent::ReportAck {
                accepted,
                failed: failed_json,
            });
        }

        WsEvent::Message { to_id, message } => {
            let target = to_id.as_deref().unwrap_or("");
            let is_local_agent = local_agents.read().await.contains(target);

            // 预留鉴权钩子 (H02/H05/H08 专利要求的权限拦截点)
            if !check_permissions_hook(&message) {
                warn!("[HASN] 消息鉴权失败，已拦截: msg_id={}", message.id);
                return;
            }

            if is_local_agent {
                info!("[HASN] 收到路由到本地 Agent {} 的消息", target);
                
                let session_id = message.conversation_id.clone();
                let content_text = message.text_content();
                let from_id = message.from_id.clone();

                // 1. 获取或创建状态化 Session
                let session = {
                    let mut lock = sessions.write().await;
                    if let Some(s) = lock.get(&session_id) {
                        s.clone()
                    } else {
                        // 初始化新的 Session
                        let bridge = agent_bridge::global_bridge();
                        if let Some(workspace) = bridge.resolve_workspace_by_hasn_id(state, target).await {
                            let config = state.config.lock().clone();
                            let mut agent_config = config.clone();
                            agent_config.workspace_dir = workspace.clone();
                            match crate::agent::Agent::from_config(&agent_config).await {
                                Ok(mut agent) => {
                                    agent.set_memory_session_id(Some(session_id.clone()));
                                    let session_key = format!("hasn_{session_id}");
                                    let per_user_backend = crate::huanxing::tenant::create_session_backend_for_workspace(&workspace, &config)
                                        .or_else(|| state.session_backend.clone());
                                    
                                    // 恢复历史
                                    if let Some(ref backend) = per_user_backend {
                                        let history = backend.load(&session_key);
                                        if !history.is_empty() {
                                            agent.seed_history(&history);
                                        }
                                    }

                                    let new_session = Arc::new(HasnAgentSession {
                                        agent: tokio::sync::Mutex::new(agent),
                                        session_key,
                                        session_backend: per_user_backend,
                                    });
                                    lock.insert(session_id.clone(), new_session.clone());
                                    new_session
                                }
                                Err(e) => {
                                    error!("[HASN] 创建 Agent 失败: {}", e);
                                    return;
                                }
                            }
                        } else {
                            warn!("[HASN] 未找到 Agent 工作区: {}", target);
                            return;
                        }
                    }
                };

                // 2. 持久化入站消息
                if let Some(ref backend) = session.session_backend {
                    let user_msg = crate::providers::ChatMessage::user(&content_text);
                    let _ = backend.append(&session.session_key, &user_msg);
                }

                // 3. 流式执行并推送回去 (turn_streamed)
                let ws_clone = ws.clone();
                let to_target = from_id.clone();
                let from_target = target.to_string();

                tokio::spawn(async move {
                    let mut agent_lock = session.agent.lock().await;
                    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<crate::agent::TurnEvent>(100);

                    // 启动一个子任务来处理反推 WS
                    let rep_ws = ws_clone.clone();
                    let rep_from = from_target.clone();
                    let rep_to = to_target.clone();
                    tokio::spawn(async move {
                        while let Some(event) = event_rx.recv().await {
                            match event {
                                crate::agent::TurnEvent::ToolCall { name, args } => {
                                    // 发送 ToolCall 到 HASN
                                    let cmd = WsCommand::Send {
                                        from_id: Some(rep_from.clone()),
                                        to: rep_to.clone(),
                                        content: serde_json::json!({
                                            "tool_name": name,
                                            "status": "running",
                                            "args": args
                                        }),
                                        content_type: Some(6), // 假设 6 为 tool_call
                                        msg_type: None, local_id: None, reply_to_id: None,
                                    };
                                    let _ = rep_ws.send_command(&cmd).await;
                                }
                                crate::agent::TurnEvent::ToolResult { name, output } => {
                                    // 发送 ToolResult 到 HASN
                                    let cmd = WsCommand::Send {
                                        from_id: Some(rep_from.clone()),
                                        to: rep_to.clone(),
                                        content: serde_json::json!({
                                            "tool_name": name,
                                            "status": "success",
                                            "result": output
                                        }),
                                        content_type: Some(6),
                                        msg_type: None, local_id: None, reply_to_id: None,
                                    };
                                    let _ = rep_ws.send_command(&cmd).await;
                                }
                                crate::agent::TurnEvent::Chunk { delta } => {
                                    // 如果支持流式分块，可以在此推送 chunk，目前演示暂不每块发网络包
                                    let _ = delta;
                                }
                                _ => {}
                            }
                        }
                    });

                    // 阻塞获得最终完整回复
                    match agent_lock.turn_streamed(&content_text, event_tx).await {
                        Ok(full_reply) => {
                            // 最终合并回复发往 HASN
                            let cmd = WsCommand::Send {
                                from_id: Some(from_target),
                                to: to_target,
                                content: serde_json::json!({"text": full_reply}),
                                content_type: Some(1),
                                msg_type: None, local_id: None, reply_to_id: None,
                            };
                            let _ = ws_clone.send_command(&cmd).await;

                            // 持久化最终回复
                            if let Some(ref backend) = session.session_backend {
                                let ast_msg = crate::providers::ChatMessage::assistant(&full_reply);
                                let _ = backend.append(&session.session_key, &ast_msg);
                            }
                        }
                        Err(e) => {
                            error!("[HASN] Agent turn 返回失败: {}", e);
                            // 发送错误通知给前端用户
                        }
                    }
                });

                // 把收到这个包的事件推送到前端 UI
                let payload = serde_json::to_value(&message).unwrap_or_default();
                let _ = event_tx.send(HasnEvent::Message { payload });

            } else {
                // to_id 不是本地 Agent (说明是发给当前人的)，直接转给 UI
                let payload = serde_json::to_value(&message).unwrap_or_default();
                let _ = event_tx.send(HasnEvent::Message { payload });
            }
        }

        WsEvent::Ack {
            msg_id,
            conversation_id,
            local_id,
            ..
        } => {
            let _ = event_tx.send(HasnEvent::Ack {
                msg_id,
                conversation_id,
                local_id,
            });
        }

        WsEvent::Typing {
            from_id,
            conversation_id,
        } => {
            let _ = event_tx.send(HasnEvent::Typing {
                from_id,
                conversation_id,
            });
        }

        WsEvent::OfflineMessages { messages } => {
            let _ = event_tx.send(HasnEvent::OfflineMessages { messages });
        }

        WsEvent::ProvisionAgent { agent_hasn_id, owner_id, config: _config } => {
            info!("[HASN] 收到 PROVISION_AGENT: {} (owner={})", agent_hasn_id, owner_id);
            // Phase 5: 创建 Agent 工作区 + ADD_AGENT
            let hx_config = state.config.lock().huanxing.clone();
            let config_dir = state.config.lock().config_path.parent().unwrap().to_path_buf();
            let _templates_dir = hx_config.resolve_templates_dir(&config_dir);
            let hx_db_path = hx_config.resolve_db_path(&config_dir);

            if let Ok(db) = TenantDb::open(&hx_db_path) {
                let st = state.clone();
                let aid = agent_hasn_id.clone();
                let uname = owner_id.clone();
                let ws_client = ws.clone();

                tokio::spawn(async move {
                    let seq = db.get_next_user_seq().await.unwrap_or(1);
                    let local_agent_id = format!("{seq:03}-{uname}-cloud");
                    let tenant_dir = format!("{seq:03}-{uname}");
                    
                    let _owner_workspace = st.config.lock().huanxing.resolve_owner_dir(&st.config.lock().config_path.parent().unwrap(), Some(&tenant_dir));
                    let agent_workspace = st.config.lock().huanxing.resolve_agent_workspace(&st.config.lock().config_path.parent().unwrap(), Some(&tenant_dir), &local_agent_id);

                    if let Err(e) = db.save_user_full(
                        &uname, &uname, &local_agent_id, Some("User"), "assistant", Some("Assistant"),
                        Some(&agent_workspace.to_string_lossy()), Some(&tenant_dir),
                        None, None, None, None
                    ).await {
                        error!("[HASN] PROVISION 保存 DB 失败: {}", e);
                    } else {
                        let _ = db.add_routing(&local_agent_id, "hasn", &uname).await;
                        info!("[HASN] 本地 DB 保存成功，开始创建工作区: {}", local_agent_id);
                        let template_base = config_dir.join("hub").join("templates");
                        let factory = huanxing_agent_factory::AgentFactory::new(config_dir.clone(), None);
                        let params = huanxing_agent_factory::CreateAgentParams {
                            tenant_id: tenant_dir.clone(),
                            template_id: "assistant".to_string(),
                            agent_name: local_agent_id.clone(),
                            display_name: "Assistant".to_string(),
                            is_desktop: false,
                            user_nickname: "User".to_string(),
                            provider: None,
                            api_key: None,
                            hasn_id: Some(aid.clone()),
                        };

                        struct ConnProgress;
                        impl huanxing_agent_factory::ProgressSink for ConnProgress {
                            fn on_progress(&self, step: &str, detail: &str) {
                                tracing::debug!("[HASN PROVISION] {} - {}", step, detail);
                            }
                        }

                        match factory.create_local_agent(&template_base, &params, &ConnProgress).await {
                            Ok(_) => {
                                info!("[HASN] 工作区创建成功。绑定 hasn_id: {}", aid);
                                let config_path = agent_workspace.join("config.toml");
                                if let Ok(content) = tokio::fs::read_to_string(&config_path).await {
                                    let mut updated = content.clone();
                                    if !updated.contains("hasn_id") {
                                        updated = format!("{updated}\nhasn_id = \"{aid}\"\n");
                                        let _ = tokio::fs::write(&config_path, updated).await;
                                    }
                                }
                                // 上报新加入的 Agent
                                let cmd = hasn_client_core::model::WsCommand::AddAgent {
                                    hasn_id: aid,
                                };
                                let _ = ws_client.send_command(&cmd).await;
                            }
                            Err(e) => error!("[HASN] PROVISION 工作区创建失败: {}", e),
                        }
                    }
                });
            } else {
                error!("[HASN] 无法打开 TenantDb，撤销 PROVISION");
            }
        }

        WsEvent::DeprovisionAgent { agent_hasn_id } => {
            info!("[HASN] 收到 DEPROVISION_AGENT: {}", agent_hasn_id);
            // TODO Phase 6: 清理 Agent 工作区
        }

        WsEvent::Error { code, message } => {
            error!("[HASN] 错误 {}: {}", code, message);
        }

        _ => {
            // PONG, PRESENCE, MESSAGE_RECALLED 等
        }
    }
}

/// 权限鉴定与环境隔离钩子 (Phase 3 Placeholder)
///
/// 后续对齐 H02/H05/H08 专利架构：
/// - H02 协议防御：异地高频拦截、信誉度骤降验证
/// - H05 服务与交易：确认是否处于 valid Trade Session
/// - H08 物理封禁：一键阻断所有请求
fn check_permissions_hook(_message: &hasn_client_core::model::WsMessagePayload) -> bool {
    // TODO: 实现权限鉴权和信誉验证拦截逻辑
    true
}

/// 全局 HasnConnector 单例
static CONNECTOR: std::sync::OnceLock<HasnConnector> = std::sync::OnceLock::new();

/// 获取全局 HasnConnector 实例
pub fn global_connector() -> &'static HasnConnector {
    CONNECTOR.get_or_init(HasnConnector::new)
}
