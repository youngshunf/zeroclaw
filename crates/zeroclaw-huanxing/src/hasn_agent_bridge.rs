//! HASN Agent 消息注入桥
//!
//! 职责:
//! 1. 在消息到达 Agent 之前进行来源分类和权限校验 (系统层硬逻辑)
//! 2. 构建标注化的注入提示词，让 Agent 能区分消息来源
//! 3. 通过 Agent 运行时执行 turn 并将回复发回 HASN 网络

use crate::hasn_chat_db::HasnChatDb;
use crate::hasn_connector::HasnAgentSession;
use crate::agent_bridge::global_bridge;
use anyhow::Context;
use hasn_client_core::model::{WsMessagePayload, build_send};
use hasn_client_core::ws::HasnWsClient;
use hasn_node::spawner::{InboundContext, ReplyChunk};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use zeroclaw_config::schema::Config;
use zeroclaw_infra::session_backend::SessionBackend;

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
    config: Config,
    session_backend: Option<Arc<dyn SessionBackend>>,
    chat_db: HasnChatDb,
    sessions: Arc<RwLock<HashMap<String, Arc<HasnAgentSession>>>>,
}

impl HasnAgentBridge {
    pub fn new(
        config: Config,
        session_backend: Option<Arc<dyn SessionBackend>>,
        chat_db: HasnChatDb,
        sessions: Arc<RwLock<HashMap<String, Arc<HasnAgentSession>>>>,
    ) -> Self {
        Self {
            config,
            session_backend,
            chat_db,
            sessions,
        }
    }

    fn config_dir(&self) -> std::path::PathBuf {
        self.config
            .config_path
            .parent()
            .unwrap_or(&self.config.workspace_dir)
            .to_path_buf()
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
        let config_dir = self.config_dir();
        let db_path = self.config.huanxing.resolve_db_path(&config_dir);

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

    async fn dispatch_message_to_reply_chunks(
        &self,
        target_agent_id: &str,
        message: WsMessagePayload,
    ) -> anyhow::Result<mpsc::Receiver<ReplyChunk>> {
        let annotated = self.classify_and_annotate(target_agent_id, &message).await;
        let (tx, rx) = mpsc::channel(32);

        if let HandlingInstruction::SilentDrop = annotated.handling_instruction {
            tracing::debug!(
                "[HasnAgentBridge] Silent drop from blocked user: {}",
                message.from_id
            );
            drop(tx);
            return Ok(rx);
        }

        let injection_prompt = Self::build_injection_prompt(&annotated, &message);
        let session_id = message.conversation_id.clone();
        let from_id = message.from_id.clone();
        let from_target = target_agent_id.to_string();
        let config = self.config.clone();
        let session_backend = self.session_backend.clone();
        let sessions = self.sessions.clone();
        let db_clone = self.chat_db.clone();

        tokio::spawn(async move {
            let bridge = global_bridge();
            let Some(tenant) = bridge
                .resolve_tenant_by_hasn_id_with_config(&config, &from_target)
                .await
            else {
                let _ = tx
                    .send(ReplyChunk::Error(
                        "huanxing dispatch failed: tenant lookup".to_string(),
                    ))
                    .await;
                return;
            };

            let session = {
                let mut lock = sessions.write().await;
                if let Some(existing) = lock.get(&session_id) {
                    existing.clone()
                } else {
                    match tenant.create_agent().await {
                        Ok(mut agent) => {
                            agent.set_memory_session_id(Some(session_id.clone()));
                            let session_key = format!("hasn_{}", session_id);
                            let per_user_backend = tenant
                                .session_manager
                                .clone()
                                .or_else(|| session_backend.clone());
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
                        Err(err) => {
                            let _ = tx.send(reply_chunk_for_error(&err)).await;
                            return;
                        }
                    }
                }
            };

            if let Some(ref backend) = session.session_backend {
                let user_msg = zeroclaw_providers::ChatMessage::user(&injection_prompt);
                let _ = backend.append(&session.session_key, &user_msg);
            }

            let mut agent_lock = session.agent.lock().await;
            let (event_tx_ch, mut event_rx) =
                mpsc::channel::<zeroclaw_runtime::agent::TurnEvent>(100);
            let chunk_tx = tx.clone();
            tokio::spawn(async move {
                while let Some(event) = event_rx.recv().await {
                    match event {
                        zeroclaw_runtime::agent::TurnEvent::ToolCall { name, args } => {
                            let _ = chunk_tx
                                .send(ReplyChunk::ToolCall {
                                    tool_id: name.clone(),
                                    tool_name: name,
                                    status: "running".to_string(),
                                    result: Some(args.to_string()),
                                })
                                .await;
                        }
                        zeroclaw_runtime::agent::TurnEvent::ToolResult { name, output } => {
                            let _ = chunk_tx
                                .send(ReplyChunk::ToolCall {
                                    tool_id: name.clone(),
                                    tool_name: name,
                                    status: "success".to_string(),
                                    result: Some(output),
                                })
                                .await;
                        }
                        _ => {}
                    }
                }
            });

            match agent_lock.turn_streamed(&injection_prompt, event_tx_ch).await {
                Ok(full_reply) => {
                    if let Some(ref backend) = session.session_backend {
                        let ast_msg = zeroclaw_providers::ChatMessage::assistant(&full_reply);
                        let _ = backend.append(&session.session_key, &ast_msg);
                    }

                    let msg_id_str = format!("msg_{}", uuid::Uuid::new_v4());
                    let record = crate::hasn_chat_db::ChatMessageRecord {
                        id: 0,
                        message_id: msg_id_str,
                        conversation_id: session_id.clone(),
                        sender_id: from_target.clone(),
                        receiver_id: from_id.clone(),
                        content_type: "text".to_string(),
                        content: serde_json::to_string(&serde_json::json!({ "text": full_reply }))
                            .unwrap_or_default(),
                        status: "delivered".to_string(),
                        is_outgoing: true,
                        created_at: chrono::Local::now()
                            .format("%Y-%m-%d %H:%M:%S")
                            .to_string(),
                    };
                    if let Err(err) = db_clone.insert_message(&record).await {
                        tracing::error!(
                            "[HasnAgentBridge] Failed to insert agent reply to hasn_chat.db: {}",
                            err
                        );
                    }
                    let _ = tx.send(ReplyChunk::Text(full_reply)).await;
                    let _ = tx.send(ReplyChunk::Done).await;
                }
                Err(err) => {
                    let _ = tx.send(reply_chunk_for_error(&err)).await;
                }
            }
        });

        Ok(rx)
    }

    pub async fn dispatch_to_reply_chunks(
        &self,
        ctx: &InboundContext,
    ) -> anyhow::Result<mpsc::Receiver<ReplyChunk>> {
        let mut message: WsMessagePayload = serde_json::from_value(ctx.raw.clone())
            .context("failed to rebuild inbound payload for huanxing dispatch")?;
        message.conversation_id = ctx.conversation_id.clone();
        message.from_id = ctx.from_hasn_id.clone();
        message.to_id = Some(ctx.agent_hasn_id.clone());
        message.content = serde_json::json!({ "text": ctx.user_message });
        self.dispatch_message_to_reply_chunks(&ctx.agent_hasn_id, message)
            .await
    }

    /// 注入消息到 Agent 运行时并流式回复
    ///
    /// Phase 05-05 — 本入口已 deprecated：hasn-node router 取而代之。
    /// `dispatch_to_reply_chunks` 仍保留（HuanxingNativeSpawner 通过它调本
    /// bridge 的 tenant lookup + agent runtime 能力），这里只对 WS 出站那层
    /// 加 debug_assert 防止生产调用 fall through。
    pub async fn inject_and_stream(
        &self,
        target_agent_id: &str,
        message: WsMessagePayload,
        ws: Arc<HasnWsClient>,
    ) {
        // Phase 05-05 Task 5 — legacy WS 出站 deprecated
        debug_assert!(
            false,
            "Phase 05-05: legacy path deprecated, use hasn-node connector"
        );

        let Ok(mut receiver) = self
            .dispatch_message_to_reply_chunks(target_agent_id, message.clone())
            .await
        else {
            let frame = build_send(
                target_agent_id,
                &message.from_id,
                serde_json::json!({"text": "huanxing dispatch failed: tenant lookup"}),
                Some(1),
                Some("error"),
                None,
                None,
            );
            let _ = ws.send_frame(&frame).await;
            return;
        };

        let from_target = target_agent_id.to_string();
        let to_target = message.from_id.clone();
        tokio::spawn(async move {
            while let Some(chunk) = receiver.recv().await {
                match chunk {
                    ReplyChunk::Text(text) => {
                        let frame = build_send(
                            &from_target,
                            &to_target,
                            serde_json::json!({ "text": text }),
                            Some(1),
                            None,
                            None,
                            None,
                        );
                        let _ = ws.send_frame(&frame).await;
                    }
                    ReplyChunk::ToolCall {
                        tool_id: _tool_id,
                        tool_name,
                        status,
                        result,
                    } => {
                        let frame = build_send(
                            &from_target,
                            &to_target,
                            serde_json::json!({
                                "tool_name": tool_name,
                                "status": status,
                                "result": result,
                            }),
                            Some(6),
                            None,
                            None,
                            None,
                        );
                        let _ = ws.send_frame(&frame).await;
                    }
                    ReplyChunk::Error(err) => {
                        let frame = build_send(
                            &from_target,
                            &to_target,
                            serde_json::json!({ "text": err }),
                            Some(1),
                            Some("error"),
                            None,
                            None,
                        );
                        let _ = ws.send_frame(&frame).await;
                    }
                    ReplyChunk::Done => break,
                }
            }
        });
    }
}

fn reply_chunk_for_error(err: &anyhow::Error) -> ReplyChunk {
    let lowered = err.to_string().to_lowercase();
    if ["busy", "queue", "lock", "capacity", "resource", "semaphore", "limit"]
        .iter()
        .any(|needle| lowered.contains(needle))
    {
        ReplyChunk::Error("busy, retry later".to_string())
    } else {
        ReplyChunk::Error(format!("huanxing dispatch failed: {err}"))
    }
}
