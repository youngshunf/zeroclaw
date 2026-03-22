use serde::{Deserialize, Serialize};

/// WebSocket 下行事件 (对齐 29/30 文档新协议)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd")]
pub enum WsEvent {
    /// 连接成功
    #[serde(rename = "CONNECTED")]
    Connected {
        user_hasn_id: String,
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
    OfflineMessages {
        messages: Vec<serde_json::Value>,
    },

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
    Presence {
        hasn_id: String,
        status: String,
    },

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

    /// 错误
    #[serde(rename = "ERROR")]
    Error { code: i32, message: String },
}

/// REPORT_AGENTS 失败项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportAgentFailed {
    pub hasn_id: String,
    pub reason: String,
}

/// WS MESSAGE 载荷中的消息体 (对齐后端 message_router 的 payload 格式)
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

    /// 转换为本地 HasnMessage
    pub fn into_hasn_message(self) -> crate::model::message::HasnMessage {
        // 提取文本内容
        let content_text = if let Some(text) = self.content.get("text").and_then(|v| v.as_str()) {
            text.to_string()
        } else if let Some(s) = self.content.as_str() {
            s.to_string()
        } else {
            self.content.to_string()
        };

        crate::model::message::HasnMessage {
            id: self.id,
            local_id: self.local_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            conversation_id: self.conversation_id,
            from_id: self.from_id,
            from_star_id: self.from_star_id,
            from_type: self.from_type,
            content: content_text,
            content_type: self.content_type,
            metadata: None,
            reply_to: self.reply_to_id,
            status: self.status.unwrap_or(1),
            send_status: crate::model::message::SendStatus::Synced,
            created_at: self.created_time,
        }
    }
}

/// WebSocket 上行命令 (客户端 → 服务端, 对齐 29 文档 §4.3)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd")]
pub enum WsCommand {
    /// 上报管理的 Agent 列表
    #[serde(rename = "REPORT_AGENTS")]
    ReportAgents {
        agents: Vec<AgentReport>,
    },

    /// 动态新增 Agent
    #[serde(rename = "ADD_AGENT")]
    AddAgent {
        hasn_id: String,
    },

    /// 动态移除 Agent
    #[serde(rename = "REMOVE_AGENT")]
    RemoveAgent {
        hasn_id: String,
    },

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
}
