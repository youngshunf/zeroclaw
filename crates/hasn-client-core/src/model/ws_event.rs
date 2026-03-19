use serde::{Deserialize, Serialize};
use crate::model::message::HasnMessage;

/// WebSocket 下行事件 (对齐后端 ws_native.py 的 cmd 协议)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd")]
pub enum WsEvent {
    /// 收到新消息
    #[serde(rename = "MESSAGE")]
    Message { message: WsMessagePayload },

    /// 发送确认
    #[serde(rename = "ACK")]
    Ack {
        msg_id: i64,
        conversation_id: String,
        local_id: Option<String>,
        status: String,
    },

    /// 心跳回复
    #[serde(rename = "PONG")]
    Pong { ts: i64 },

    /// 错误
    #[serde(rename = "ERROR")]
    Error { code: i32, message: String },
}

/// WS MESSAGE 载荷中的消息体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsMessagePayload {
    pub id: i64,
    pub conversation_id: String,
    pub from_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_star_id: Option<String>,
    pub from_type: i32,
    pub content: String,
    pub content_type: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

impl WsMessagePayload {
    /// 转换为本地 HasnMessage
    pub fn into_hasn_message(self) -> HasnMessage {
        HasnMessage {
            id: self.id,
            local_id: uuid::Uuid::new_v4().to_string(),
            conversation_id: self.conversation_id,
            from_id: self.from_id,
            from_star_id: self.from_star_id,
            from_type: self.from_type,
            content: self.content,
            content_type: self.content_type,
            metadata: None,
            reply_to: None,
            status: 1,
            send_status: crate::model::message::SendStatus::Synced,
            created_at: self.created_at,
        }
    }
}

/// WebSocket 上行命令 (客户端 → 服务端)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd")]
pub enum WsCommand {
    /// 发送消息
    #[serde(rename = "SEND")]
    Send {
        to: String,
        content: String,
        content_type: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        local_id: Option<String>,
    },

    /// 标记已读
    #[serde(rename = "READ")]
    Read {
        conversation_id: String,
        last_msg_id: i64,
    },

    /// 心跳
    #[serde(rename = "PING")]
    Ping { ts: i64 },
}
