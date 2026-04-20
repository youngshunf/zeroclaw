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
//!
//! # Phase 05-05 — Deprecated shim
//!
//! 除 `global_connector()` symbol 供 `hasn_sync.rs` 等非 WS 热路径使用外，
//! 所有 WS 帧入站/出站/连接控制在 05-05 cutover 后由 hasn-node 独占。
//! 入口函数（`HasnConnector::connect` / `handle_ws_frame`）都加了
//! `debug_assert!(false, …)` 以便 debug build 里立即暴露误入 legacy 路径
//! 的调用；release build 则继续执行原逻辑以保持过渡期兼容。
//!
//! 模块 `test_harness` (仅 `#[cfg(any(debug_assertions, test))]`) 暴露一组
//! 计数器 + getter，供集成测试断言 legacy 入口未被触发。

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

use zeroclaw_gateway::AppState;
use crate::db::TenantDb;

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
    pub agent: tokio::sync::Mutex<zeroclaw_runtime::agent::Agent>,
    pub session_key: String,
    pub session_backend:
        Option<std::sync::Arc<dyn zeroclaw_infra::session_backend::SessionBackend>>,
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
        // Phase 05-05 Task 5 — legacy WS 入口 deprecated
        debug_assert!(
            false,
            "Phase 05-05: legacy path deprecated, use hasn-node connector"
        );
        test_harness::note_connect_called();

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
                        local_entities.clone(),
                        &connected,
                        sessions.clone(),
                        state.clone(),
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
                            local_entities.clone(),
                            &connected,
                            sessions.clone(),
                            state.clone(),
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
        let is_ws_connected = self.ws.status().await == hasn_client_core::ws::WsStatus::Connected;
        *self.connected.read().await && is_ws_connected
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
    local_entities: Arc<RwLock<HashSet<String>>>,
    connected: &RwLock<bool>,
    sessions: Arc<RwLock<HashMap<String, Arc<HasnAgentSession>>>>,
    state: Arc<AppState>,
    ws: Arc<HasnWsClient>,
) {
    // Phase 05-05 Task 5 — legacy WS 帧入站处理 deprecated
    debug_assert!(
        false,
        "Phase 05-05: legacy path deprecated, use hasn-node connector"
    );
    test_harness::note_handle_ws_frame_called();

    let method = frame.method.as_str();
    // 调试：记录所有入站帧方法
    info!("[HASN] 收到帧: method={}", method);

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

                let owner_id = params.owner_id.clone();
                
                // 启动当前用户的后台增量同步任务
                crate::hasn_sync::spawn_periodic_sync(state.clone(), owner_id.clone());

                let state_clone = state.clone();
                let ws_clone = ws.clone();
                let local_entities_clone = local_entities.clone();

                tokio::spawn(async move {
                    let config = state_clone.config.lock().clone();
                    let config_dir = config.config_path.parent().unwrap_or(&config.workspace_dir);
                    let db_path = config.huanxing.resolve_db_path(config_dir);

                    if let Ok(db) = TenantDb::open(&db_path) {
                        if let Ok(Some(tenant)) = db.find_by_hasn_id(&owner_id).await {
                            if let Ok(agents) = db.get_user_agents(&tenant.user_id).await {
                                for agent in agents {
                                    if let Some(agent_hasn_id) = agent.hasn_id {
                                        tracing::info!("[HASN] 自动发现本地 Agent {} ({}), 发起驻留注册", agent.agent_id, agent_hasn_id);
                                        local_entities_clone.write().await.insert(agent_hasn_id.clone());
                                        let req_frame = build_add_agent(&agent_hasn_id, &owner_id);
                                        let _ = ws_clone.send_frame(&req_frame).await;
                                    }
                                }
                            }
                        }
                    }
                });

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
            match serde_json::from_value::<MessageReceivedParams>(frame.params.clone()) {
                Ok(params) => {
                let target = &params.to_id;
                let is_local = local_entities.read().await.contains(target.as_str());

                // 预留鉴权钩子
                if !check_permissions_hook(&params.message) {
                    warn!("[HASN] 消息鉴权失败，已拦截: msg_id={:?}", params.message.id);
                    return;
                }

                if is_local {
                    let router = crate::hasn_router::MessageRouter::new(
                        state.clone(),
                        ws.clone(),
                        sessions.clone(),
                    );
                    if let Err(e) = router.dispatch(params.message.clone()).await {
                        error!("[HASN] 消息路由失败: {}", e);
                    }
                    let payload = serde_json::to_value(&params.message).unwrap_or_default();
                    let _ = event_tx.send(HasnEvent::Message { payload });
                } else {
                    let payload = serde_json::to_value(&params.message).unwrap_or_default();
                    let _ = event_tx.send(HasnEvent::Message { payload });
                }
                }
                Err(e) => {
                    error!("[HASN] MessageReceivedParams 反序列化失败: {}\nRaw Payload: {}", e, frame.params);
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
                                avatar_url: None,
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
                                        zeroclaw_config::huanxing::promote_legacy_agent_config_from_workspace(
                                            &agent_workspace,
                                        );
                                    let config_path =
                                        zeroclaw_config::huanxing::agent_config_path_from_workspace(
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

        "hasn.node.add_agent_ack" => {
            // Agent 驻留注册确认 — 仅记录日志
            info!("[HASN] Agent 驻留注册确认: {:?}", frame.params);
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

// ════════════════════════════════════════════════════════════════════
// Phase 05-05 Task 5 — test_harness
// ════════════════════════════════════════════════════════════════════
//
// debug build 下统计 legacy 入口被调用次数。release build 下编译为空壳
// 以避免对热路径造成任何观测开销。集成测试（cfg=test）可通过
// `was_connect_called()` / `was_handle_ws_frame_called()` / reset() 断言
// 本 cutover 后 legacy 入口不再被生产路径触发。

#[cfg(any(debug_assertions, test))]
pub mod test_harness {
    use std::sync::atomic::{AtomicUsize, Ordering};

    static CONNECT_CALLS: AtomicUsize = AtomicUsize::new(0);
    static HANDLE_WS_FRAME_CALLS: AtomicUsize = AtomicUsize::new(0);

    #[inline]
    pub(super) fn note_connect_called() {
        CONNECT_CALLS.fetch_add(1, Ordering::SeqCst);
    }

    #[inline]
    pub(super) fn note_handle_ws_frame_called() {
        HANDLE_WS_FRAME_CALLS.fetch_add(1, Ordering::SeqCst);
    }

    /// 计数器归零（测试 setup 用）
    pub fn reset() {
        CONNECT_CALLS.store(0, Ordering::SeqCst);
        HANDLE_WS_FRAME_CALLS.store(0, Ordering::SeqCst);
    }

    /// Legacy `HasnConnector::connect` 被调用过？
    pub fn was_connect_called() -> bool {
        CONNECT_CALLS.load(Ordering::SeqCst) > 0
    }

    /// Legacy `handle_ws_frame` 被调用过？
    pub fn was_handle_ws_frame_called() -> bool {
        HANDLE_WS_FRAME_CALLS.load(Ordering::SeqCst) > 0
    }

    pub fn connect_call_count() -> usize {
        CONNECT_CALLS.load(Ordering::SeqCst)
    }

    pub fn handle_ws_frame_call_count() -> usize {
        HANDLE_WS_FRAME_CALLS.load(Ordering::SeqCst)
    }
}

// release build 下提供同名 no-op shim，保持 inline 调用点 API 一致
#[cfg(not(any(debug_assertions, test)))]
pub mod test_harness {
    #[inline(always)]
    pub(super) fn note_connect_called() {}
    #[inline(always)]
    pub(super) fn note_handle_ws_frame_called() {}
}
