use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════
// HASN Protocol v2.0 — WebSocket 帧模型
//
// 帧格式: { "hasn": "hasn/2.0", "method": "hasn.xxx.yyy", "params": {...} }
// 对齐文档: Core/01-传输层协议.md §0.3
// ═══════════════════════════════════════════════════════════════

/// 当前协议版本
pub const HASN_PROTOCOL: &str = "hasn/2.0";

// ─── 通用帧结构 ───

/// 下行帧（Server → Node）— 通用 JSON 结构
///
/// 使用 serde_json::Value 做灵活解析，因为 params 结构因 method 而异。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HasnFrame {
    /// 协议标识 + 版本: "hasn/2.0"
    pub hasn: String,
    /// 方法名: "hasn.xxx.yyy"  
    #[serde(default)]
    pub method: String,
    /// 参数体（method 决定结构）
    #[serde(default)]
    pub params: serde_json::Value,
    /// 请求 ID（可选，需响应时携带）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// 响应结果（仅响应帧使用）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// 响应错误（仅响应帧使用）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<HasnError>,
}

/// 协议错误
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HasnError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<serde_json::Value>,
}

impl HasnFrame {
    /// 构造事件帧（Server 推送）
    pub fn event(method: &str, params: serde_json::Value) -> Self {
        Self {
            hasn: HASN_PROTOCOL.to_string(),
            method: method.to_string(),
            params,
            id: None,
            result: None,
            error: None,
        }
    }

    /// 构造请求帧（Node 上行）
    pub fn request(method: &str, params: serde_json::Value) -> Self {
        Self {
            hasn: HASN_PROTOCOL.to_string(),
            method: method.to_string(),
            params,
            id: None,
            result: None,
            error: None,
        }
    }
}

// ─── 下行事件 params 结构 ───

/// hasn.connected params
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedParams {
    pub node_id: String,
    #[serde(default)]
    pub node_type: String,
    #[serde(default = "default_capacity")]
    pub capacity: i32,
    pub server_time: String,
    #[serde(default)]
    pub supported_versions: Vec<String>,
    #[serde(default)]
    pub extensions: Vec<String>,
}

fn default_capacity() -> i32 {
    1
}

/// hasn.node.report_entities_ack params
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportEntitiesAckParams {
    pub accepted: Vec<String>,
    pub failed: Vec<ReportEntityFailed>,
}

/// 实体上报失败项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportEntityFailed {
    pub hasn_id: String,
    pub reason: String,
}

/// hasn.node.add_entity_ack params
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddEntityAckParams {
    pub hasn_id: String,
    pub accepted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// hasn.agent.register_ack params
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRegisterAckParams {
    pub hasn_id: String,
    pub star_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_key: Option<String>,
    #[serde(default)]
    pub already_exists: bool,
}

/// hasn.message.received params
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageReceivedParams {
    pub to_id: String,
    pub message: WsMessagePayload,
}

/// hasn.node.offline_messages params
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineMessagesParams {
    pub messages: Vec<serde_json::Value>,
}

/// hasn.message.ack params
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageAckParams {
    pub msg_id: serde_json::Value,
    pub conversation_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_id: Option<String>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
}

/// hasn.pong params  
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PongParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ts: Option<serde_json::Value>,
}

/// hasn.error params
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorParams {
    pub code: i32,
    pub message: String,
}

/// hasn.typing params
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypingParams {
    pub from_id: String,
    pub conversation_id: String,
}

/// hasn.node.provision_agent params
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisionAgentParams {
    pub agent_hasn_id: String,
    pub owner_id: String,
    #[serde(default)]
    pub config: serde_json::Value,
}

/// hasn.node.deprovision_agent params
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeprovisionAgentParams {
    pub agent_hasn_id: String,
}

// ─── WS MESSAGE 载荷 ───

/// WS MESSAGE 载荷中的消息体
///
/// 这是**传输层**的载荷结构，直接对接中央服务端的 JSON 格式。
/// 通过 `into_envelope()` 方法可将其桥接到 v4.0 的强类型 `HasnEnvelope`。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsMessagePayload {
    pub id: serde_json::Value,
    pub conversation_id: String,
    pub from_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_star_id: Option<String>,
    #[serde(default = "default_from_type")]
    pub from_type: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_type: Option<i32>,
    /// 消息内容 (JSONB: {text: "xxx"} 或 {url: "xxx"})
    pub content: serde_json::Value,
    #[serde(default = "default_content_type")]
    pub content_type: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_time: Option<String>,
    /// 标记为自己发的消息（多端同步用）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub self_sent: Option<bool>,
    /// 发送方 owner_id
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_owner_id: Option<String>,
    /// 接收方 owner_id
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_owner_id: Option<String>,
}

fn default_from_type() -> i32 {
    1
}
fn default_content_type() -> i32 {
    1
}

impl WsMessagePayload {
    /// 提取文本内容
    pub fn text_content(&self) -> String {
        if let Some(text) = self.content.get("text").and_then(|v| v.as_str()) {
            text.to_string()
        } else if let Some(s) = self.content.as_str() {
            s.to_string()
        } else {
            self.content.to_string()
        }
    }

    /// 将传输层载荷桥接为 v4.0 强类型 HasnEnvelope
    pub fn into_envelope(self) -> crate::model::message::HasnEnvelope {
        use crate::model::message::*;

        let from_entity_type = match self.from_type {
            2 => EntityType::Agent,
            3 => EntityType::System,
            _ => EntityType::Human,
        };

        let to_entity_type = match self.to_type.unwrap_or(1) {
            2 => EntityType::Agent,
            3 => EntityType::System,
            _ => EntityType::Human,
        };

        let content_type = match self.content_type {
            2 => ContentType::Image,
            3 => ContentType::File,
            4 => ContentType::Voice,
            5 => ContentType::Card,
            6 => ContentType::CapabilityRequest,
            _ => ContentType::Text,
        };

        let body = if content_type == ContentType::Text {
            if self.content.get("text").is_some() {
                self.content.clone()
            } else if let Some(s) = self.content.as_str() {
                serde_json::json!({ "text": s })
            } else {
                serde_json::json!({ "text": self.content.to_string() })
            }
        } else {
            self.content.clone()
        };

        let msg_id = match &self.id {
            serde_json::Value::Number(n) => format!("msg_{}", n),
            serde_json::Value::String(s) => s.clone(),
            _ => format!("msg_{}", ulid::Ulid::new()),
        };

        let from_owner = self
            .from_owner_id
            .clone()
            .unwrap_or_else(|| self.from_id.clone());
        let to_id = self.to_id.clone().unwrap_or_default();
        let to_owner = self.to_owner_id.clone().unwrap_or_else(|| to_id.clone());

        HasnEnvelope {
            id: msg_id,
            version: "2.0".to_string(),
            msg_type: MessageType::Message,
            from: EntityRef {
                hasn_id: self.from_id,
                entity_type: from_entity_type,
                owner_id: from_owner,
            },
            to: EntityRef {
                hasn_id: to_id,
                entity_type: to_entity_type,
                owner_id: to_owner,
            },
            content: MessageContent { content_type, body },
            context: MessageContext {
                conversation_id: self.conversation_id,
                relation_type: None,
                scope: None,
                trade_session_id: None,
                thread_id: None,
                reply_to: self.reply_to_id.map(|id| id.to_string()),
                capability_id: None,
            },
            metadata: MessageMetadata {
                priority: Priority::Normal,
                created_at: self
                    .created_time
                    .unwrap_or_else(|| chrono::Utc::now().to_rfc3339()),
                server_received_at: None,
            },
        }
    }
}

// ─── 上行命令构造辅助 ───

/// 实体上报项（Human 或 Agent）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityReport {
    pub hasn_id: String,
    pub entity_type: String,
    /// Human 实体需要的 auth_token
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_token: Option<String>,
    /// Agent 实体需要的 owner_id
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
}

/// 兼容旧代码的 Agent 上报项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentReport {
    pub hasn_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
}

/// 构造 hasn.node.report_entities 请求帧
pub fn build_report_entities(entities: Vec<EntityReport>) -> HasnFrame {
    HasnFrame::request(
        "hasn.node.report_entities",
        serde_json::json!({ "entities": entities }),
    )
}

/// 构造 hasn.node.add_entity 请求帧
pub fn build_add_entity(
    hasn_id: &str,
    entity_type: &str,
    auth_token: Option<&str>,
    owner_id: Option<&str>,
) -> HasnFrame {
    let mut params = serde_json::json!({
        "hasn_id": hasn_id,
        "entity_type": entity_type,
    });
    if let Some(token) = auth_token {
        params["auth_token"] = serde_json::Value::String(token.to_string());
    }
    if let Some(oid) = owner_id {
        params["owner_id"] = serde_json::Value::String(oid.to_string());
    }
    HasnFrame::request("hasn.node.add_entity", params)
}

/// 构造 hasn.node.remove_entity 请求帧
pub fn build_remove_entity(hasn_id: &str, entity_type: &str) -> HasnFrame {
    HasnFrame::request(
        "hasn.node.remove_entity",
        serde_json::json!({
            "hasn_id": hasn_id,
            "entity_type": entity_type,
        }),
    )
}

/// 构造 hasn.message.send 请求帧
pub fn build_send(
    from_id: &str,
    to: &str,
    content: serde_json::Value,
    content_type: Option<i32>,
    msg_type: Option<&str>,
    local_id: Option<&str>,
    reply_to_id: Option<i64>,
) -> HasnFrame {
    let mut params = serde_json::json!({
        "from_id": from_id,
        "to": to,
        "content": content,
    });
    if let Some(ct) = content_type {
        params["content_type"] = serde_json::json!(ct);
    }
    if let Some(mt) = msg_type {
        params["type"] = serde_json::Value::String(mt.to_string());
    }
    if let Some(lid) = local_id {
        params["local_id"] = serde_json::Value::String(lid.to_string());
    }
    if let Some(rid) = reply_to_id {
        params["context"] = serde_json::json!({ "reply_to": rid });
    }
    HasnFrame::request("hasn.message.send", params)
}

/// 构造 hasn.ping 请求帧
pub fn build_ping(ts: i64) -> HasnFrame {
    HasnFrame::request("hasn.ping", serde_json::json!({ "ts": ts }))
}

/// 构造 hasn.message.read 请求帧
pub fn build_read(conversation_id: &str, last_msg_id: i64) -> HasnFrame {
    HasnFrame::request(
        "hasn.message.read",
        serde_json::json!({
            "conversation_id": conversation_id,
            "last_msg_id": last_msg_id,
        }),
    )
}

/// 构造 hasn.typing 请求帧（上行：通知对方正在输入）
///
/// 对齐协议 §3.10
pub fn build_typing(conversation_id: &str, to_id: &str) -> HasnFrame {
    HasnFrame::request(
        "hasn.typing",
        serde_json::json!({
            "conversation_id": conversation_id,
            "to_id": to_id,
        }),
    )
}

/// 构造 hasn.message.recall 请求帧
///
/// 对齐协议 §3.8
pub fn build_recall(msg_id: &str, conversation_id: &str) -> HasnFrame {
    HasnFrame::request(
        "hasn.message.recall",
        serde_json::json!({
            "msg_id": msg_id,
            "conversation_id": conversation_id,
        }),
    )
}

/// 构造 hasn.message.edit 请求帧
///
/// 对齐协议 §3.9
pub fn build_edit(
    msg_id: &str,
    conversation_id: &str,
    content_type: &str,
    body: serde_json::Value,
) -> HasnFrame {
    HasnFrame::request(
        "hasn.message.edit",
        serde_json::json!({
            "msg_id": msg_id,
            "conversation_id": conversation_id,
            "content": {
                "content_type": content_type,
                "body": body,
            },
        }),
    )
}

/// 构造 hasn.agent.register 请求帧（通过 WS 创建新 Agent）
///
/// 对齐协议 §3.3a
pub fn build_agent_register(
    owner_id: &str,
    agent_name: &str,
    display_name: &str,
    agent_type: Option<&str>,
    role: Option<&str>,
    description: Option<&str>,
    capabilities: Option<&[&str]>,
) -> HasnFrame {
    let mut params = serde_json::json!({
        "owner_id": owner_id,
        "agent_name": agent_name,
        "display_name": display_name,
    });
    if let Some(at) = agent_type {
        params["agent_type"] = serde_json::Value::String(at.to_string());
    }
    if let Some(r) = role {
        params["role"] = serde_json::Value::String(r.to_string());
    }
    if let Some(d) = description {
        params["description"] = serde_json::Value::String(d.to_string());
    }
    if let Some(caps) = capabilities {
        params["capabilities"] = serde_json::json!(caps);
    }
    HasnFrame::request("hasn.agent.register", params)
}

/// 构造 hasn.agent.deregister 请求帧
///
/// 对齐协议 §3.3b
pub fn build_agent_deregister(hasn_id: &str) -> HasnFrame {
    HasnFrame::request(
        "hasn.agent.deregister",
        serde_json::json!({ "hasn_id": hasn_id }),
    )
}

// ─── 缺失的下行事件 params 结构 ───

/// hasn.message.read_receipt params（对齐协议 §3.7）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadReceiptParams {
    pub conversation_id: String,
    pub reader: String,
    pub last_msg_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
}

/// hasn.message.recalled params（对齐协议 §3.8）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecalledParams {
    pub msg_id: String,
    pub conversation_id: String,
    pub recalled_by: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
}

/// hasn.message.edited params（对齐协议 §3.9）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditedParams {
    pub msg_id: String,
    pub conversation_id: String,
    pub edited_by: String,
    pub new_content: serde_json::Value,
    #[serde(default)]
    pub edit_version: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
}

/// hasn.presence params
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceParams {
    pub hasn_id: String,
    /// online / offline / away
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
}
