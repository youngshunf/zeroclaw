//! HASN 连接管理器 — 运行在 ZeroClaw Sidecar 进程内
//!
//! 所有节点（桌面端/云端）共用同一套代码。
//! 负责：
//! - 管理到 HASN 中央节点的 WS 长连接
//! - add_owner / renew_owner / add_agent 等控制平面命令
//! - 处理入站消息：to_id ∈ local_agents → 进程内 agent_bridge.invoke()
//! - 事件广播到订阅者（供前端 /ws/hasn-events 消费）
//!
//! 帧格式: { "hasn": "hasn/2.0", "method": "hasn.xxx.yyy", "params": {...} }

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use tokio::sync::{RwLock, broadcast};
use tracing::{error, info, warn};

use hasn_client_core::model::{
    HasnFrame,
    ConnectedParams,
    AddOwnerAckParams, RemoveOwnerAckParams, RenewOwnerAckParams, ListOwnersAckParams,
    AgentRegisterAckParams,
    MessageReceivedParams, OfflineMessagesParams, MessageAckParams,
    TypingParams, ErrorParams, ProvisionAgentParams, DeprovisionAgentParams,
    ReadReceiptParams, RecalledParams, EditedParams, PresenceParams,
    build_send, build_add_owner, build_remove_owner,
    build_renew_owner, build_list_owners, build_add_agent, build_remove_agent,
};
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
    Connected { node_id: String, node_type: String },
    /// 连接断开
    #[serde(rename = "disconnected")]
    Disconnected,
    /// HASN 消息（推送给前端展示）
    #[serde(rename = "message")]
    Message { payload: serde_json::Value },
    /// 实体上报结果
    #[serde(rename = "report_ack")]
    ReportAck {
        accepted: Vec<String>,
        failed: Vec<serde_json::Value>,
    },
    /// ACK 回执
    #[serde(rename = "ack")]
    Ack {
        msg_id: serde_json::Value,
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
    OfflineMessages { messages: Vec<serde_json::Value> },
    /// Agent 注册结果（hasn.agent.register_ack）
    #[serde(rename = "agent_registered")]
    AgentRegistered {
        hasn_id: String,
        star_id: String,
        agent_key: Option<String>,
        already_exists: bool,
    },
    /// 已读回执（hasn.message.read_receipt）
    #[serde(rename = "read_receipt")]
    ReadReceipt {
        conversation_id: String,
        reader: String,
        last_msg_id: String,
    },
    /// 消息撤回（hasn.message.recalled）
    #[serde(rename = "message_recalled")]
    MessageRecalled {
        msg_id: String,
        conversation_id: String,
        recalled_by: String,
    },
    /// 消息编辑（hasn.message.edited）
    #[serde(rename = "message_edited")]
    MessageEdited {
        msg_id: String,
        conversation_id: String,
        new_content: serde_json::Value,
    },
    /// 在线状态（hasn.presence）
    #[serde(rename = "presence")]
    Presence {
        hasn_id: String,
        status: String,
    },
    #[serde(rename = "owner_bound")]
    OwnerBound { owner_id: String, binding_id: String },
    #[serde(rename = "owner_removed")]
    OwnerRemoved { owner_id: String, accepted: bool },
    #[serde(rename = "owner_renewed")]
    OwnerRenewed { owner_id: String, binding_id: String, expires_at: Option<String> },
    #[serde(rename = "owners_list")]
    OwnersList { owners: Vec<serde_json::Value> },
}

/// HASN Agent 专属多路复用会话状态
pub struct HasnAgentSession {
    pub agent: tokio::sync::Mutex<crate::agent::Agent>,
    pub session_key: String,
    pub session_backend:
        Option<std::sync::Arc<dyn crate::channels::session_backend::SessionBackend>>,
}

/// HASN 连接管理器
pub struct HasnConnector {
    /// hasn-client-core 的 WS 客户端
    ws: Arc<HasnWsClient>,
    /// 本节点的 node_id（连接成功后由服务端返回）
    node_id: Arc<RwLock<Option<String>>>,
    /// 本节点上报的实体 hasn_id 集合（Human + Agent）
    local_entities: Arc<RwLock<HashSet<String>>>,
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
            local_entities: Arc::new(RwLock::new(HashSet::new())),
            event_tx,
            connected: Arc::new(RwLock::new(false)),
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 连接 HASN 中央节点
    pub async fn connect(
        &self,
        central_url: &str,
        auth_headers: Vec<(String, String)>,
        state: Arc<AppState>,
    ) -> anyhow::Result<()> {
        info!(
            "[HASN] 连接中央节点: {}",
            &central_url[..central_url.find('?').unwrap_or(central_url.len())]
        );

        let event_tx = self.event_tx.clone();
        let node_id = self.node_id.clone();
        let local_entities = self.local_entities.clone();
        let connected = self.connected.clone();
        let ws_ref = self.ws.clone();
        let sessions = self.sessions.clone();

        self.ws
            .connect_with_headers(central_url, &auth_headers, move |frame| {
                let event_tx = event_tx.clone();
                let node_id = node_id.clone();
                let local_entities = local_entities.clone();
                let connected = connected.clone();
                let sessions = sessions.clone();
                let state = state.clone();
                let ws_ref = ws_ref.clone();

                tokio::spawn(async move {
                    handle_ws_frame(
                        frame,
                        &event_tx,
                        &node_id,
                        &local_entities,
                        &connected,
                        &sessions,
                        &state,
                        ws_ref,
                    )
                    .await;
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
        auth_headers: Vec<(String, String)>,
        max_retries: u32,
        state: Arc<AppState>,
    ) -> anyhow::Result<()> {
        let event_tx = self.event_tx.clone();
        let node_id = self.node_id.clone();
        let local_entities = self.local_entities.clone();
        let connected = self.connected.clone();
        let ws_ref = self.ws.clone();
        let sessions = self.sessions.clone();

        self.ws
            .connect_with_retry_headers(
                central_url,
                &auth_headers,
                move |frame| {
                    let event_tx = event_tx.clone();
                    let node_id = node_id.clone();
                    let local_entities = local_entities.clone();
                    let connected = connected.clone();
                    let sessions = sessions.clone();
                    let state = state.clone();
                    let ws_ref = ws_ref.clone();

                    tokio::spawn(async move {
                        handle_ws_frame(
                            frame,
                            &event_tx,
                            &node_id,
                            &local_entities,
                            &connected,
                            &sessions,
                            &state,
                            ws_ref,
                        )
                        .await;
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
        let frame = build_send(
            &from_id.unwrap_or_default(),
            to,
            content,
            Some(1),
            None,
            local_id.as_deref(),
            None,
        );
        self.ws
            .send_frame(&frame)
            .await
            .map_err(|e| anyhow::anyhow!("HASN 发送失败: {}", e))
    }

    pub async fn add_owner(
        &self,
        owner_id: &str,
        proof_type: &str,
        credential: &str,
    ) -> anyhow::Result<()> {
        let frame = build_add_owner(owner_id, proof_type, credential);
        self.ws
            .send_frame(&frame)
            .await
            .map_err(|e| anyhow::anyhow!("HASN add_owner 失败: {}", e))
    }

    pub async fn renew_owner(
        &self,
        owner_id: &str,
        proof_type: &str,
        credential: &str,
    ) -> anyhow::Result<()> {
        let frame = build_renew_owner(owner_id, proof_type, credential);
        self.ws
            .send_frame(&frame)
            .await
            .map_err(|e| anyhow::anyhow!("HASN renew_owner 失败: {}", e))
    }

    pub async fn remove_owner(&self, owner_id: &str) -> anyhow::Result<()> {
        let frame = build_remove_owner(owner_id);
        self.ws
            .send_frame(&frame)
            .await
            .map_err(|e| anyhow::anyhow!("HASN remove_owner 失败: {}", e))
    }

    pub async fn list_owners(&self) -> anyhow::Result<()> {
        let frame = build_list_owners();
        self.ws
            .send_frame(&frame)
            .await
            .map_err(|e| anyhow::anyhow!("HASN list_owners 失败: {}", e))
    }

    pub async fn add_agent_presence(&self, agent_id: &str, owner_id: &str) -> anyhow::Result<()> {
        {
            let mut local = self.local_entities.write().await;
            local.insert(agent_id.to_string());
        }
        let frame = build_add_agent(agent_id, owner_id);
        self.ws
            .send_frame(&frame)
            .await
            .map_err(|e| anyhow::anyhow!("HASN add_agent 失败: {}", e))
    }

    pub async fn remove_agent_presence(&self, agent_id: &str) -> anyhow::Result<()> {
        {
            let mut local = self.local_entities.write().await;
            local.remove(agent_id);
        }
        let frame = build_remove_agent(agent_id);
        self.ws
            .send_frame(&frame)
            .await
            .map_err(|e| anyhow::anyhow!("HASN remove_agent 失败: {}", e))
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

/// 处理入站 WS 帧（HASN v2.0 格式）
async fn handle_ws_frame(
    frame: HasnFrame,
    event_tx: &broadcast::Sender<HasnEvent>,
    node_id: &RwLock<Option<String>>,
    local_entities: &RwLock<HashSet<String>>,
    connected: &RwLock<bool>,
    sessions: &RwLock<HashMap<String, Arc<HasnAgentSession>>>,
    state: &AppState,
    ws: Arc<HasnWsClient>,
) {
    let method = frame.method.as_str();

    match method {
        "hasn.connected" => {
            if let Ok(params) = serde_json::from_value::<ConnectedParams>(frame.params) {
                *node_id.write().await = Some(params.node_id.clone());
                *connected.write().await = true;
                info!(
                    "[HASN] 已连接: node_id={}, type={}",
                    params.node_id, params.node_type
                );
                let _ = event_tx.send(HasnEvent::Connected {
                    node_id: params.node_id,
                    node_type: params.node_type,
                });
            }
        }

        "hasn.node.add_owner_ack" => {
            if let Ok(params) = serde_json::from_value::<AddOwnerAckParams>(frame.params) {
                local_entities.write().await.insert(params.owner_id.clone());
                let _ = event_tx.send(HasnEvent::OwnerBound {
                    owner_id: params.owner_id,
                    binding_id: params.binding_id,
                });
            }
        }

        "hasn.node.remove_owner_ack" => {
            if let Ok(params) = serde_json::from_value::<RemoveOwnerAckParams>(frame.params) {
                local_entities.write().await.remove(&params.owner_id);
                let _ = event_tx.send(HasnEvent::OwnerRemoved {
                    owner_id: params.owner_id,
                    accepted: params.accepted,
                });
            }
        }

        "hasn.node.renew_owner_ack" => {
            if let Ok(params) = serde_json::from_value::<RenewOwnerAckParams>(frame.params) {
                let _ = event_tx.send(HasnEvent::OwnerRenewed {
                    owner_id: params.owner_id,
                    binding_id: params.binding_id,
                    expires_at: params.expires_at,
                });
            }
        }

        "hasn.node.list_owners_ack" => {
            if let Ok(params) = serde_json::from_value::<ListOwnersAckParams>(frame.params) {
                let _ = event_tx.send(HasnEvent::OwnersList { owners: params.owners });
            }
        }

        "hasn.message.received" => {
            if let Ok(params) = serde_json::from_value::<MessageReceivedParams>(frame.params) {
                let target = &params.to_id;
                let is_local = local_entities.read().await.contains(target.as_str());

                // 预留鉴权钩子
                if !check_permissions_hook(&params.message) {
                    warn!("[HASN] 消息鉴权失败，已拦截: msg_id={:?}", params.message.id);
                    return;
                }

                if is_local {
                    info!("[HASN] 收到路由到本地 Agent {} 的消息", target);

                    let session_id = params.message.conversation_id.clone();
                    let content_text = params.message.text_content();
                    let from_id = params.message.from_id.clone();

                    // 1. 获取或创建状态化 Session
                    let session = {
                        let mut lock = sessions.write().await;
                        if let Some(s) = lock.get(&session_id) {
                            s.clone()
                        } else {
                            let bridge = agent_bridge::global_bridge();
                            if let Some(tenant) =
                                bridge.resolve_tenant_by_hasn_id(state, target).await
                            {
                                match tenant.create_agent().await {
                                    Ok(mut agent) => {
                                        agent.set_memory_session_id(Some(session_id.clone()));
                                        let session_key = format!("hasn_{session_id}");
                                        let per_user_backend = tenant
                                            .session_manager
                                            .clone()
                                            .or_else(|| state.session_backend.clone());

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
                                warn!("[HASN] 未找到 Agent TenantContext: {}", target);
                                return;
                            }
                        }
                    };

                    // 2. 持久化入站消息
                    if let Some(ref backend) = session.session_backend {
                        let user_msg = crate::providers::ChatMessage::user(&content_text);
                        let _ = backend.append(&session.session_key, &user_msg);
                    }

                    // 3. 流式执行并推送回去
                    let ws_clone = ws.clone();
                    let to_target = from_id.clone();
                    let from_target = target.to_string();

                    tokio::spawn(async move {
                        let mut agent_lock = session.agent.lock().await;
                        let (event_tx_ch, mut event_rx) =
                            tokio::sync::mpsc::channel::<crate::agent::TurnEvent>(100);

                        let rep_ws = ws_clone.clone();
                        let rep_from = from_target.clone();
                        let rep_to = to_target.clone();
                        tokio::spawn(async move {
                            while let Some(event) = event_rx.recv().await {
                                match event {
                                    crate::agent::TurnEvent::ToolCall { name, args } => {
                                        let frame = build_send(
                                            &rep_from,
                                            &rep_to,
                                            serde_json::json!({
                                                "tool_name": name,
                                                "status": "running",
                                                "args": args
                                            }),
                                            Some(6),
                                            None,
                                            None,
                                            None,
                                        );
                                        let _ = rep_ws.send_frame(&frame).await;
                                    }
                                    crate::agent::TurnEvent::ToolResult { name, output } => {
                                        let frame = build_send(
                                            &rep_from,
                                            &rep_to,
                                            serde_json::json!({
                                                "tool_name": name,
                                                "status": "success",
                                                "result": output
                                            }),
                                            Some(6),
                                            None,
                                            None,
                                            None,
                                        );
                                        let _ = rep_ws.send_frame(&frame).await;
                                    }
                                    crate::agent::TurnEvent::Chunk { delta } => {
                                        let _ = delta;
                                    }
                                    _ => {}
                                }
                            }
                        });

                        match agent_lock.turn_streamed(&content_text, event_tx_ch).await {
                            Ok(full_reply) => {
                                let frame = build_send(
                                    &from_target,
                                    &to_target,
                                    serde_json::json!({"text": full_reply}),
                                    Some(1),
                                    None,
                                    None,
                                    None,
                                );
                                let _ = ws_clone.send_frame(&frame).await;

                                if let Some(ref backend) = session.session_backend {
                                    let ast_msg =
                                        crate::providers::ChatMessage::assistant(&full_reply);
                                    let _ = backend.append(&session.session_key, &ast_msg);
                                }
                            }
                            Err(e) => {
                                error!("[HASN] Agent turn 返回失败: {}", e);
                            }
                        }
                    });

                    let payload = serde_json::to_value(&params.message).unwrap_or_default();
                    let _ = event_tx.send(HasnEvent::Message { payload });
                } else {
                    let payload = serde_json::to_value(&params.message).unwrap_or_default();
                    let _ = event_tx.send(HasnEvent::Message { payload });
                }
            }
        }

        "hasn.message.ack" => {
            if let Ok(params) = serde_json::from_value::<MessageAckParams>(frame.params) {
                let _ = event_tx.send(HasnEvent::Ack {
                    msg_id: params.msg_id,
                    conversation_id: params.conversation_id,
                    local_id: params.local_id,
                });
            }
        }

        "hasn.typing" => {
            if let Ok(params) = serde_json::from_value::<TypingParams>(frame.params) {
                let _ = event_tx.send(HasnEvent::Typing {
                    from_id: params.from_id,
                    conversation_id: params.conversation_id,
                });
            }
        }

        "hasn.node.offline_messages" => {
            if let Ok(params) = serde_json::from_value::<OfflineMessagesParams>(frame.params) {
                let _ = event_tx.send(HasnEvent::OfflineMessages {
                    messages: params.messages,
                });
            }
        }

        "hasn.node.provision_agent" => {
            if let Ok(params) = serde_json::from_value::<ProvisionAgentParams>(frame.params) {
                info!(
                    "[HASN] 收到 PROVISION_AGENT: {} (owner={})",
                    params.agent_hasn_id, params.owner_id
                );
                let hx_config = state.config.lock().huanxing.clone();
                let config_dir = state
                    .config
                    .lock()
                    .config_path
                    .parent()
                    .unwrap()
                    .to_path_buf();
                let hx_db_path = hx_config.resolve_db_path(&config_dir);

                if let Ok(db) = TenantDb::open(&hx_db_path) {
                    let st = state.clone();
                    let aid = params.agent_hasn_id.clone();
                    let uname = params.owner_id.clone();
                    let ws_client = ws.clone();

                    tokio::spawn(async move {
                        let seq = db.get_next_user_seq().await.unwrap_or(1);
                        let local_agent_id = format!("{seq:03}-{uname}-cloud");
                        let tenant_dir = format!("{seq:03}-{uname}");

                        let agent_workspace = st.config.lock().huanxing.resolve_agent_workspace(
                            &st.config.lock().config_path.parent().unwrap(),
                            Some(&tenant_dir),
                            &local_agent_id,
                        );

                        if let Err(e) = db
                            .save_user_full(
                                &uname,
                                &uname,
                                &local_agent_id,
                                Some("User"),
                                "assistant",
                                Some("Assistant"),
                                Some(&agent_workspace.to_string_lossy()),
                                Some(&tenant_dir),
                                None, // hasn_id
                                None,
                                None,
                                None,
                                None,
                            )
                            .await
                        {
                            error!("[HASN] PROVISION 保存 DB 失败: {}", e);
                        } else {
                            let _ = db.add_routing(&local_agent_id, "hasn", &uname).await;
                            info!(
                                "[HASN] 本地 DB 保存成功，开始创建工作区: {}",
                                local_agent_id
                            );
                            let factory =
                                huanxing_agent_factory::AgentFactory::new(config_dir.clone(), None);
                            let params = huanxing_agent_factory::CreateAgentParams {
                                tenant_id: tenant_dir.clone(),
                                template_id: "assistant".to_string(),
                                agent_name: local_agent_id.clone(),
                                display_name: "Assistant".to_string(),
                                is_desktop: false,
                                user_nickname: "User".to_string(),
                                user_phone: uname.clone(),
                                owner_dir: st
                                    .config
                                    .lock()
                                    .huanxing
                                    .resolve_owner_dir(&config_dir, Some(&tenant_dir))
                                    .to_string_lossy()
                                    .to_string(),
                                provider: None,
                                model: None,
                                api_key: None,
                                hasn_id: Some(aid.clone()),
                                fallback_provider: None,
                                embedding_provider: None,
                                llm_gateway: None,
                            };

                            struct ConnProgress;
                            impl huanxing_agent_factory::ProgressSink for ConnProgress {
                                fn on_progress(&self, step: &str, detail: &str) {
                                    tracing::debug!("[HASN PROVISION] {} - {}", step, detail);
                                }
                            }

                            let template_base = config_dir.join("hub").join("templates");
                            match factory
                                .create_local_agent(&template_base, &params, &ConnProgress)
                                .await
                            {
                                Ok(_) => {
                                    info!("[HASN] 工作区创建成功。绑定 hasn_id: {}", aid);
                                    let _ =
                                        crate::huanxing::config::promote_legacy_agent_config_from_workspace(
                                            &agent_workspace,
                                        );
                                    let config_path =
                                        crate::huanxing::config::agent_config_path_from_workspace(
                                            &agent_workspace,
                                        );
                                    if let Ok(content) =
                                        tokio::fs::read_to_string(&config_path).await
                                    {
                                        let mut updated = content.clone();
                                        if !updated.contains("hasn_id") {
                                            updated =
                                                format!("{updated}\nhasn_id = \"{aid}\"\n");
                                            let _ =
                                                tokio::fs::write(&config_path, updated).await;
                                        }
                                    }
                                    // 上报新加入的实体 (Agent)
                                    let add_frame = hasn_client_core::model::build_add_agent(
                                        &aid, &uname,
                                    );
                                    let _ = ws_client.send_frame(&add_frame).await;
                                }
                                Err(e) => error!("[HASN] PROVISION 工作区创建失败: {}", e),
                            }
                        }
                    });
                } else {
                    error!("[HASN] 无法打开 TenantDb，撤销 PROVISION");
                }
            }
        }

        "hasn.node.deprovision_agent" => {
            if let Ok(params) = serde_json::from_value::<DeprovisionAgentParams>(frame.params) {
                info!("[HASN] 收到 DEPROVISION_AGENT: {}", params.agent_hasn_id);
                // TODO Phase 6: 清理 Agent 工作区
            }
        }

        "hasn.error" => {
            if let Ok(params) = serde_json::from_value::<ErrorParams>(frame.params) {
                error!("[HASN] 错误 {}: {}", params.code, params.message);
            }
        }

        "hasn.pong" => {
            // 心跳回复，忽略
        }

        "hasn.agent.register_ack" => {
            if let Ok(params) = serde_json::from_value::<AgentRegisterAckParams>(frame.params) {
                info!(
                    "[HASN] Agent 注册结果: {} (star_id={}, already_exists={})",
                    params.hasn_id, params.star_id, params.already_exists
                );
                let _ = event_tx.send(HasnEvent::AgentRegistered {
                    hasn_id: params.hasn_id,
                    star_id: params.star_id,
                    agent_key: params.agent_key,
                    already_exists: params.already_exists,
                });
            }
        }

        "hasn.message.read_receipt" => {
            if let Ok(params) = serde_json::from_value::<ReadReceiptParams>(frame.params) {
                let _ = event_tx.send(HasnEvent::ReadReceipt {
                    conversation_id: params.conversation_id,
                    reader: params.reader,
                    last_msg_id: params.last_msg_id,
                });
            }
        }

        "hasn.message.recalled" => {
            if let Ok(params) = serde_json::from_value::<RecalledParams>(frame.params) {
                warn!("[HASN] 消息撤回: {} by {}", params.msg_id, params.recalled_by);
                let _ = event_tx.send(HasnEvent::MessageRecalled {
                    msg_id: params.msg_id,
                    conversation_id: params.conversation_id,
                    recalled_by: params.recalled_by,
                });
            }
        }

        "hasn.message.edited" => {
            if let Ok(params) = serde_json::from_value::<EditedParams>(frame.params) {
                let _ = event_tx.send(HasnEvent::MessageEdited {
                    msg_id: params.msg_id,
                    conversation_id: params.conversation_id,
                    new_content: params.new_content,
                });
            }
        }

        "hasn.presence" => {
            if let Ok(params) = serde_json::from_value::<PresenceParams>(frame.params) {
                let _ = event_tx.send(HasnEvent::Presence {
                    hasn_id: params.hasn_id,
                    status: params.status,
                });
            }
        }

        _ => {
            warn!("[HASN] 未知方法: {}", method);
        }
    }
}

/// 权限鉴定与环境隔离钩子 (Phase 3 Placeholder)
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
