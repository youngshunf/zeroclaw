use serde::{Deserialize, Serialize};

/// 消息发送状态 (本地专用)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SendStatus {
    Sending,
    Sent,
    Failed,
    Synced,
}

impl Default for SendStatus {
    fn default() -> Self {
        Self::Sending
    }
}

impl SendStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Sending => "sending",
            Self::Sent => "sent",
            Self::Failed => "failed",
            Self::Synced => "synced",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "sending" => Self::Sending,
            "sent" => Self::Sent,
            "failed" => Self::Failed,
            "synced" => Self::Synced,
            _ => Self::Sending,
        }
    }
}

/// HASN 消息 (对齐后端 hasn_messages 表)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HasnMessage {
    /// 服务端 BIGINT 自增ID (发送中时为0)
    #[serde(default)]
    pub id: i64,

    /// 本地临时ID (UUID v4, 发送时立即生成)
    pub local_id: String,

    /// 会话ID (UUID)
    pub conversation_id: String,

    /// 发送者 hasn_id (h_xxx / a_xxx)
    pub from_id: String,

    /// 发送者唤星号
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_star_id: Option<String>,

    /// 发送者类型: 1=human 2=agent 3=system
    #[serde(default = "default_from_type")]
    pub from_type: i32,

    /// 消息内容
    pub content: String,

    /// 内容类型: 1=text 2=image 3=file 4=voice 5=rich 6=capability
    #[serde(default = "default_content_type")]
    pub content_type: i32,

    /// 附加元数据 (JSON)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,

    /// 回复的消息ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<i64>,

    /// 服务端消息状态: 1=sent 2=delivered 3=read 4=deleted
    #[serde(default = "default_status")]
    pub status: i32,

    /// 本地发送状态
    #[serde(default)]
    pub send_status: SendStatus,

    /// 创建时间 (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

fn default_from_type() -> i32 {
    1
}
fn default_content_type() -> i32 {
    1
}
fn default_status() -> i32 {
    1
}

impl HasnMessage {
    /// 创建一条待发送的本地消息
    pub fn new_outgoing(
        conversation_id: &str,
        from_id: &str,
        content: &str,
        content_type: i32,
        reply_to: Option<i64>,
    ) -> Self {
        Self {
            id: 0,
            local_id: uuid::Uuid::new_v4().to_string(),
            conversation_id: conversation_id.to_string(),
            from_id: from_id.to_string(),
            from_star_id: None,
            from_type: 1,
            content: content.to_string(),
            content_type,
            metadata: None,
            reply_to,
            status: 1,
            send_status: SendStatus::Sending,
            created_at: Some(chrono::Utc::now().to_rfc3339()),
        }
    }
}
