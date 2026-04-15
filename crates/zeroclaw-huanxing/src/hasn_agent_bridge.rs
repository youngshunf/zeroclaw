//! HASN Agent 消息注入桥
//!
//! 职责:
//! 1. 在消息到达 Agent 之前进行来源分类和权限校验 (系统层硬逻辑)
//! 2. 构建标注化的注入提示词，让 Agent 能区分消息来源
//! 3. 通过 Agent 运行时执行 turn 并将回复发回 HASN 网络

use zeroclaw_gateway::AppState;
use crate::hasn_chat_db::HasnChatDb;
use crate::hasn_connector::HasnAgentSession;
use crate::agent_bridge::global_bridge;
use hasn_client_core::model::{WsMessagePayload, build_send};
use hasn_client_core::ws::HasnWsClient;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

// ═══════════════════════════════════════════════════════════════════
// 消息来源分类
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageSource {
    /// 主人消息 — 最高权限
    Owner,
    /// 好友消息 — 按 trust_level 分级
    Friend { trust_level: i32 },
    /// 陌生人消息 — 无关系记录
    Stranger,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HandlingInstruction {
    /// 正常处理
    ProcessNormally,
    /// 陌生人消息，需要筛选处理
    ScreenFirst,
    /// 静默丢弃 (已拉黑)
    SilentDrop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotatedMessage {
    pub source: MessageSource,
    pub handling_instruction: HandlingInstruction,
}

// ═══════════════════════════════════════════════════════════════════
// Agent Bridge
// ═══════════════════════════════════════════════════════════════════

pub struct HasnAgentBridge {
    app_state: Arc<AppState>,
    chat_db: HasnChatDb,
}

impl HasnAgentBridge {
    pub fn new(app_state: Arc<AppState>, chat_db: HasnChatDb) -> Self {
        Self { app_state, chat_db }
    }

    /// 分类消息来源并生成标注
    ///
    /// 通过 TenantDb 查询 agent_hasn_id 所属的 owner hasn_id，
    /// 然后和 from_id 对比来判断是否为主人消息。
    pub async fn classify_and_annotate(
        &self,
        agent_hasn_id: &str,
        message: &WsMessagePayload,
    ) -> AnnotatedMessage {
        // 检查发送者是否是 Agent 的 Owner
        let config = self.app_state.config.lock().clone();
        let config_dir = config.config_path.parent().unwrap_or(&config.workspace_dir).to_path_buf();
        let db_path = config.huanxing.resolve_db_path(&config_dir);

        // find_by_hasn_id(a_xxx) returns the TenantRecord of the owner
        // TenantRecord.hasn_id is the owner's human hasn_id (h_xxx)
        let is_owner = if let Ok(tenant_db) = crate::db::TenantDb::open(&db_path) {
            if let Ok(Some(tenant)) = tenant_db.find_by_hasn_id(agent_hasn_id).await {
                tenant.hasn_id.as_deref() == Some(&message.from_id)
            } else {
                false
            }
        } else {
            false
        };

        if is_owner {
            return AnnotatedMessage {
                source: MessageSource::Owner,
                handling_instruction: HandlingInstruction::ProcessNormally,
            };
        }

        // 查询本地联系人 trust_level
        let contact = self.chat_db.get_contact(&message.from_id).await.unwrap_or(None);
        if let Some(c) = contact {
            match c.trust_level {
                0 => AnnotatedMessage {
                    source: MessageSource::Stranger,
                    handling_instruction: HandlingInstruction::SilentDrop,
                },
                1 => AnnotatedMessage {
                    source: MessageSource::Stranger,
                    handling_instruction: HandlingInstruction::ScreenFirst,
                },
                level => AnnotatedMessage {
                    source: MessageSource::Friend { trust_level: level },
                    handling_instruction: HandlingInstruction::ProcessNormally,
                },
            }
        } else {
            // 本地无联系人记录 = 陌生人
            AnnotatedMessage {
                source: MessageSource::Stranger,
                handling_instruction: HandlingInstruction::ScreenFirst,
            }
        }
    }

    /// 构建注入给 Agent 的提示词（带有来源标注）
    pub fn build_injection_prompt(annotated: &AnnotatedMessage, message: &WsMessagePayload) -> String {
        let text = message.text_content();

        let source_label = match &annotated.source {
            MessageSource::Owner => "【主人消息】".to_string(),
            MessageSource::Friend { trust_level } => {
                let level_label = match trust_level {
                    2 => "普通联系人",
                    3 => "朋友",
                    4 | 5 => "密友/高信任好友",
                    _ => "联系人",
                };
                format!("【{}消息】", level_label)
            }
            MessageSource::Stranger => "【陌生人消息】".to_string(),
        };

        let handling_hint = match &annotated.handling_instruction {
            HandlingInstruction::ScreenFirst => {
                "\n⚠️ 这是陌生人消息，请先评估其意图和价值。如无明确正当需求，可礼貌告知对方需要先添加好友。不要透露任何私密信息。"
            }
            _ => "",
        };

        format!(
            "{} 来自 {}:\n{}{}",
            source_label, message.from_id, text, handling_hint
        )
    }

    /// 注入消息到 Agent 运行时并流式回复
    pub async fn inject_and_stream(
        &self,
        target_agent_id: &str,
        message: WsMessagePayload,
        ws: Arc<HasnWsClient>,
        sessions: Arc<RwLock<HashMap<String, Arc<HasnAgentSession>>>>,
    ) {
        let annotated = self.classify_and_annotate(target_agent_id, &message).await;

        // 系统层权限执行: 拉黑直接丢弃
        if let HandlingInstruction::SilentDrop = annotated.handling_instruction {
            tracing::debug!(
                "[HasnAgentBridge] Silent drop from blocked user: {}",
                message.from_id
            );
            return;
        }

        let injection_prompt = Self::build_injection_prompt(&annotated, &message);
        let session_id = message.conversation_id.clone();
        let from_id = message.from_id.clone();
        let to_target = from_id.clone();
        let from_target = target_agent_id.to_string();
        let app_state = self.app_state.clone();

        // 获取或创建 Agent Session
        let session = {
            let mut lock = sessions.write().await;
            if let Some(s) = lock.get(&session_id) {
                s.clone()
            } else {
                let bridge = global_bridge();
                if let Some(tenant) = bridge
                    .resolve_tenant_by_hasn_id(&app_state, target_agent_id)
                    .await
                {
                    match tenant.create_agent().await {
                        Ok(mut agent) => {
                            agent.set_memory_session_id(Some(session_id.clone()));
                            let session_key = format!("hasn_{}", session_id);
                            let per_user_backend = tenant
                                .session_manager
                                .clone()
                                .or_else(|| app_state.session_backend.clone());
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
                            tracing::error!("[HASN] Agent 创建失败: {}", e);
                            return;
                        }
                    }
                } else {
                    tracing::warn!(
                        "[HASN] 未找到 Agent TenantContext: {}",
                        target_agent_id
                    );
                    return;
                }
            }
        };

        // 持久化用户消息到 Agent 的 session backend
        if let Some(ref backend) = session.session_backend {
            let user_msg = zeroclaw_providers::ChatMessage::user(&injection_prompt);
            let _ = backend.append(&session.session_key, &user_msg);
        }

        let db_clone = self.chat_db.clone();
        // 异步执行 Agent turn + 流式回复
        tokio::spawn(async move {
            let mut agent_lock = session.agent.lock().await;
            let (event_tx_ch, mut event_rx) =
                tokio::sync::mpsc::channel::<zeroclaw_runtime::agent::TurnEvent>(100);

            let rep_ws = ws.clone();
            let rep_from = from_target.clone();
            let rep_to = to_target.clone();

            // 转发 AgentEvent → HASN WS 帧
            tokio::spawn(async move {
                while let Some(event) = event_rx.recv().await {
                    match event {
                        zeroclaw_runtime::agent::TurnEvent::ToolCall { name, args } => {
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
                        zeroclaw_runtime::agent::TurnEvent::ToolResult { name, output } => {
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
                        _ => {}
                    }
                }
            });

            match agent_lock.turn_streamed(&injection_prompt, event_tx_ch).await {
                Ok(full_reply) => {
                    let frame = hasn_client_core::model::build_send(
                        &from_target,
                        &to_target,
                        serde_json::json!({"text": full_reply}),
                        Some(1),
                        None,
                        None,
                        None,
                    );
                    let _ = ws.send_frame(&frame).await;

                    if let Some(ref backend) = session.session_backend {
                        let ast_msg = zeroclaw_providers::ChatMessage::assistant(&full_reply);
                        let _ = backend.append(&session.session_key, &ast_msg);
                    }

                    // ====== 本地聊天数据库双写保存 ======
                    let msg_id_str = format!("msg_{}", uuid::Uuid::new_v4());
                    let record = crate::hasn_chat_db::ChatMessageRecord {
                        id: 0,
                        message_id: msg_id_str,
                        conversation_id: session_id.clone(),
                        sender_id: from_target.clone(),
                        receiver_id: to_target.clone(),
                        content_type: "text".to_string(),
                        content: serde_json::to_string(&serde_json::json!({"text": full_reply})).unwrap_or_default(),
                        status: "delivered".to_string(),
                        is_outgoing: true, // as it's from local agent
                        created_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                    };
                    if let Err(e) = db_clone.insert_message(&record).await {
                        tracing::error!("[HasnAgentBridge] Failed to insert agent reply to hasn_chat.db: {}", e);
                    }
                }
                Err(e) => {
                    tracing::error!("[HASN] Agent turn 失败: {}", e);
                    let err_msg = format!("[系统提示] Agent 会话失败: {}", e);
                    let frame = hasn_client_core::model::build_send(
                        &from_target,
                        &to_target,
                        serde_json::json!({"text": err_msg}),
                        Some(1),
                        None,
                        None,
                        None,
                    );
                    let _ = ws.send_frame(&frame).await;
                }
            }
        });
    }
}
