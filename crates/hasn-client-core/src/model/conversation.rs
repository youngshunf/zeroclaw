use serde::{Deserialize, Serialize};

/// HASN 会话 (对齐后端 hasn_conversations 表)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HasnConversation {
    /// 会话ID (UUID)
    pub id: String,

    /// 会话类型: direct / group
    #[serde(default = "default_conv_type")]
    pub conv_type: String,

    /// 对方 hasn_id (direct 会话)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer_hasn_id: Option<String>,

    /// 对方唤星号
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer_star_id: Option<String>,

    /// 对方显示名
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer_name: Option<String>,

    /// 对方类型: human / agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer_type: Option<String>,

    /// 对方头像
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer_avatar_url: Option<String>,

    /// 最后消息时间 (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_message_at: Option<String>,

    /// 最后消息预览
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_message_preview: Option<String>,

    /// 消息总数
    #[serde(default)]
    pub message_count: i64,

    /// 未读数
    #[serde(default)]
    pub unread_count: i32,

    /// 状态: active / archived / deleted
    #[serde(default = "default_conv_status")]
    pub status: String,
}

fn default_conv_type() -> String { "direct".to_string() }
fn default_conv_status() -> String { "active".to_string() }
