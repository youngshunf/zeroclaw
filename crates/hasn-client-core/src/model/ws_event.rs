use serde::{Deserialize, Serialize};

/// WebSocket 下行事件 (统一节点架构 v4.0)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd")]
pub enum WsEvent {
    /// 连接成功（统一节点握手）
    #[serde(rename = "CONNECTED")]
    Connected {
        /// 节点 ID（统一节点模型）
        #[serde(default)]
        node_id: String,
        /// 节点类型: desktop / mobile / web / cloud
        #[serde(default)]
        node_type: String,
        /// 最大 Agent 承载量
        #[serde(default = "default_capacity")]
        capacity: i32,
        /// 用户 HASN ID
        #[serde(default)]
        user_hasn_id: String,
        /// 兼容旧字段（= node_id）
        #[serde(default)]
        client_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        star_id: Option<String>,
        server_time: String,
    },

    /// Agent 上报结果
    #[serde(rename = "REPORT_AGENTS_ACK")]
    ReportAgentsAck {
        accepted: Vec<String>,
        failed: Vec<ReportAgentFailed>,
    },

    /// 动态新增 Agent 结果
    #[serde(rename = "ADD_AGENT_ACK")]
    AddAgentAck {
        hasn_id: String,
        accepted: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },

    /// 收到新消息
    #[serde(rename = "MESSAGE")]
    Message {
        #[serde(skip_serializing_if = "Option::is_none")]
        to_id: Option<String>,
        message: WsMessagePayload,
    },

    /// 离线消息批量补推
    #[serde(rename = "OFFLINE_MESSAGES")]
    OfflineMessages { messages: Vec<serde_json::Value> },

    /// 发送确认
    #[serde(rename = "ACK")]
    Ack {
        msg_id: i64,
        conversation_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        local_id: Option<String>,
        status: String,
    },

    /// 对方正在输入
    #[serde(rename = "TYPING")]
    Typing {
        from_id: String,
        conversation_id: String,
    },

    /// 已读回执
    #[serde(rename = "READ_RECEIPT")]
    ReadReceipt {
        conversation_id: String,
        reader: String,
        last_msg_id: i64,
    },

    /// 在线状态变化
    #[serde(rename = "PRESENCE")]
    Presence { hasn_id: String, status: String },

    /// 消息撤回通知
    #[serde(rename = "MESSAGE_RECALLED")]
    MessageRecalled {
        msg_id: i64,
        conversation_id: String,
        recalled_by: String,
    },

    /// 心跳回复
    #[serde(rename = "PONG")]
    Pong { ts: i64 },

    /// 创建 Agent 工作区（中央节点定向下发）
    #[serde(rename = "PROVISION_AGENT")]
    ProvisionAgent {
        agent_hasn_id: String,
        owner_id: String,
        #[serde(default)]
        config: serde_json::Value,
    },

    /// 删除 Agent 工作区（中央节点定向下发）
    #[serde(rename = "DEPROVISION_AGENT")]
    DeprovisionAgent { agent_hasn_id: String },

    /// 错误
    #[serde(rename = "ERROR")]
    Error { code: i32, message: String },
}

fn default_capacity() -> i32 {
    1
}

/// REPORT_AGENTS 失败项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportAgentFailed {
    pub hasn_id: String,
    pub reason: String,
}

/// WS MESSAGE 载荷中的消息体
///
/// 这是**传输层**的载荷结构，直接对接中央服务端的 JSON 格式。
/// 服务端当前仍使用 i32 类型字段，因此此处保留兼容。
/// 通过 `into_envelope()` 方法可将其桥接到 v4.0 的强类型 `HasnEnvelope`。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsMessagePayload {
    pub id: i64,
    pub conversation_id: String,
    pub from_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_star_id: Option<String>,
    pub from_type: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_type: Option<i32>,
    /// 消息内容 (JSONB: {text: "xxx"} 或 {url: "xxx"})
    pub content: serde_json::Value,
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
    /// v4.0 扩展: 发送方 owner_id（新版服务端会填充）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_owner_id: Option<String>,
    /// v4.0 扩展: 接收方 owner_id
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_owner_id: Option<String>,
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
    ///
    /// 处理旧版 i32 → 新版 String enum 的映射：
    /// - from_type: 1=human, 2=agent, 3=system
    /// - content_type: 1=text, 2=image, 3=file, 4=voice, 5=card, 6=capability
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

        // 内容体规范化：确保 body 是 { "text": "..." } 结构
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

        let from_owner = self
            .from_owner_id
            .clone()
            .unwrap_or_else(|| self.from_id.clone());
        let to_id = self.to_id.clone().unwrap_or_default();
        let to_owner = self.to_owner_id.clone().unwrap_or_else(|| to_id.clone());

        HasnEnvelope {
            id: self
                .local_id
                .unwrap_or_else(|| format!("msg_{}", ulid::Ulid::new())),
            version: "1.0".to_string(),
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

/// WebSocket 上行命令 (节点 → 服务端, 统一节点架构 v4.0)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd")]
pub enum WsCommand {
    /// 上报管理的 Agent 列表
    #[serde(rename = "REPORT_AGENTS")]
    ReportAgents { agents: Vec<AgentReport> },

    /// 动态新增 Agent
    #[serde(rename = "ADD_AGENT")]
    AddAgent { hasn_id: String },

    /// 动态移除 Agent
    #[serde(rename = "REMOVE_AGENT")]
    RemoveAgent { hasn_id: String },

    /// 发送消息
    #[serde(rename = "SEND")]
    Send {
        #[serde(skip_serializing_if = "Option::is_none")]
        from_id: Option<String>,
        to: String,
        content: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        content_type: Option<i32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        msg_type: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        local_id: Option<String>,
        /// 回复的消息 ID（可选，用于消息引用/回复）
        #[serde(skip_serializing_if = "Option::is_none")]
        reply_to_id: Option<i64>,
    },

    /// 标记已读
    #[serde(rename = "READ")]
    Read {
        conversation_id: String,
        last_msg_id: i64,
    },

    /// 正在输入
    #[serde(rename = "TYPING")]
    Typing {
        conversation_id: String,
        to_id: String,
    },

    /// 心跳
    #[serde(rename = "PING")]
    Ping { ts: i64 },
}

/// Agent 上报项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentReport {
    pub hasn_id: String,
    /// Agent 归属者的 HASN ID（中央节点校验用）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
}
