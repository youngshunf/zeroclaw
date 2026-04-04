use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════════
// HASN Protocol v4.0 — Layer 2: 消息与通信 (对齐 02-消息与通信.md)
// ═══════════════════════════════════════════════════════════════════

/// 实体类型 — 标识 HASN 网络中的参与者角色
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    Human,
    Agent,
    Group,
    System,
}

impl Default for EntityType {
    fn default() -> Self {
        Self::Human
    }
}

impl std::fmt::Display for EntityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Human => write!(f, "human"),
            Self::Agent => write!(f, "agent"),
            Self::Group => write!(f, "group"),
            Self::System => write!(f, "system"),
        }
    }
}

/// 实体引用 — 消息中的 from / to 身份描述符
///
/// 对齐协议 2.1 节：
/// ```json
/// "from": { "hasn_id": "h_xxx", "entity_type": "human", "owner_id": "h_xxx" }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityRef {
    /// HASN 网络唯一 ID (h_xxx / a_xxx / g_xxx)
    pub hasn_id: String,
    /// 实体类型
    pub entity_type: EntityType,
    /// 所属主人的 HASN ID（Human 本身发信此处为其自身 ID）
    pub owner_id: String,
}

/// 内容类型 — 定义消息负载的格式语义
///
/// 对齐协议 2.4 节：从 text, image, file, voice, card
/// 到 LLM 原生的 stream_chunk, tool_call 等全部覆盖
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentType {
    /// 纯文本
    Text,
    /// 图片
    Image,
    /// 文件
    File,
    /// 语音
    Voice,
    /// 富文本卡片
    Card,
    /// 能力调用请求
    CapabilityRequest,
    /// 能力调用响应
    CapabilityResponse,
    /// LLM 流式传输片段
    StreamChunk,
    /// Agent 工具调用执行状态
    ToolCall,
}

impl Default for ContentType {
    fn default() -> Self {
        Self::Text
    }
}

impl std::fmt::Display for ContentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text => write!(f, "text"),
            Self::Image => write!(f, "image"),
            Self::File => write!(f, "file"),
            Self::Voice => write!(f, "voice"),
            Self::Card => write!(f, "card"),
            Self::CapabilityRequest => write!(f, "capability_request"),
            Self::CapabilityResponse => write!(f, "capability_response"),
            Self::StreamChunk => write!(f, "stream_chunk"),
            Self::ToolCall => write!(f, "tool_call"),
        }
    }
}

/// 消息内容 — 类型化的消息负载
///
/// 对齐协议 2.1 节：
/// ```json
/// "content": { "content_type": "text", "body": { "text": "..." } }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageContent {
    /// 内容类型标识
    pub content_type: ContentType,
    /// 内容体（JSON 动态值，根据 content_type 的不同而结构各异）
    pub body: serde_json::Value,
}

impl MessageContent {
    /// 快捷构造：纯文本消息
    pub fn text(text: &str) -> Self {
        Self {
            content_type: ContentType::Text,
            body: serde_json::json!({ "text": text }),
        }
    }

    /// 快捷构造：流式片段
    pub fn stream_chunk(stream_id: &str, chunk: &str, seq: u64, is_end: bool) -> Self {
        Self {
            content_type: ContentType::StreamChunk,
            body: serde_json::json!({
                "stream_id": stream_id,
                "chunk": chunk,
                "seq": seq,
                "is_end": is_end,
            }),
        }
    }

    /// 快捷构造：工具调用状态
    pub fn tool_call(
        tool_id: &str,
        tool_name: &str,
        display_text: &str,
        status: &str,
        args: Option<serde_json::Value>,
        result: Option<&str>,
    ) -> Self {
        let mut body = serde_json::json!({
            "tool_id": tool_id,
            "tool_name": tool_name,
            "display_text": display_text,
            "status": status,
        });
        if let Some(a) = args {
            body["args"] = a;
        }
        if let Some(r) = result {
            body["result"] = serde_json::Value::String(r.to_string());
        }
        Self {
            content_type: ContentType::ToolCall,
            body,
        }
    }

    /// 提取纯文本（适配各种 content_type 的降级读取）
    pub fn text_content(&self) -> String {
        match self.content_type {
            ContentType::Text => self
                .body
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            ContentType::StreamChunk => self
                .body
                .get("chunk")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            ContentType::ToolCall => self
                .body
                .get("display_text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            _ => self.body.to_string(),
        }
    }
}

/// 关系类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationType {
    Social,
    Commerce,
    Service,
    Professional,
}

impl std::fmt::Display for RelationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Social => write!(f, "social"),
            Self::Commerce => write!(f, "commerce"),
            Self::Service => write!(f, "service"),
            Self::Professional => write!(f, "professional"),
        }
    }
}

/// 消息上下文 — 会话、线程与交易的关联元数据
///
/// 对齐协议 2.1 节的 context 块
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageContext {
    /// 会话 ID (conv_{uuid})
    pub conversation_id: String,
    /// 关系类型
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relation_type: Option<RelationType>,
    /// 作用域（commerce/service/professional 类消息强制提供）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    /// 交易会话 ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trade_session_id: Option<String>,
    /// 线程 ID（用于话题追踪）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    /// 回复的消息 ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,
    /// 关联的能力 ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capability_id: Option<String>,
}

/// 消息优先级
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Critical,
    High,
    Normal,
    Low,
}

impl Default for Priority {
    fn default() -> Self {
        Self::Normal
    }
}

/// 消息元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageMetadata {
    /// 消息优先级
    #[serde(default)]
    pub priority: Priority,
    /// 客户端创建时间 (ISO 8601)
    pub created_at: String,
    /// 服务端接收时间 (ISO 8601)，由 Server 填充
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_received_at: Option<String>,
}

/// 消息类型 — 对齐协议 2.3 节的完整消息类型表
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    /// 普通对话消息
    Message,
    /// 能力调用请求
    CapabilityRequest,
    /// 能力调用响应
    CapabilityResponse,
    /// Agent 通知主人
    Notification,
    /// 好友请求
    ContactRequest,
    /// 接受好友请求
    ContactAccept,
    /// 拒绝好友请求
    ContactReject,
    /// 服务发现查询
    DiscoveryQuery,
    /// 服务发现响应
    DiscoveryResponse,
    /// 交易上下文沟通
    TradeMessage,
    /// 订单状态变更
    OrderNotification,
    /// 群聊邀请
    GroupInvite,
    /// 群设置变更
    GroupUpdate,
    /// 正在输入（不持久化）
    Typing,
    /// 已读回执
    ReadReceipt,
    /// 在线状态变更（不持久化）
    Presence,
    /// 系统消息
    System,
    /// 经验卡片分享
    ExperienceShare,
    /// 消息撤回
    MessageRecall,
    /// 消息编辑
    MessageEdit,
}

impl Default for MessageType {
    fn default() -> Self {
        Self::Message
    }
}

/// HASN 消息信封 — 在 HASN 网络中传输的顶层包裹
///
/// 严格对齐 `02-消息与通信.md` 2.1 节的完整消息结构
///
/// ```json
/// {
///   "id": "msg_{ulid}",
///   "version": "1.0",
///   "type": "message",
///   "from": { "hasn_id": "...", "entity_type": "...", "owner_id": "..." },
///   "to": { "hasn_id": "...", "entity_type": "...", "owner_id": "..." },
///   "content": { "content_type": "text", "body": { "text": "..." } },
///   "context": { "conversation_id": "conv_xxx", ... },
///   "metadata": { "priority": "normal", "created_at": "..." }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HasnEnvelope {
    /// 消息全局唯一 ID (格式: msg_{ULID})
    pub id: String,
    /// 协议版本
    #[serde(default = "default_version")]
    pub version: String,
    /// 消息类型
    #[serde(rename = "type")]
    pub msg_type: MessageType,

    /// 发送方身份
    pub from: EntityRef,
    /// 接收方地址
    pub to: EntityRef,

    /// 消息内容
    pub content: MessageContent,
    /// 会话上下文
    pub context: MessageContext,
    /// 元数据
    pub metadata: MessageMetadata,
}

fn default_version() -> String {
    "2.0".to_string()
}

impl HasnEnvelope {
    /// 构造一条待发送的普通文本消息
    pub fn new_text(from: EntityRef, to: EntityRef, conversation_id: &str, text: &str) -> Self {
        Self {
            id: format!("msg_{}", ulid::Ulid::new()),
            version: "2.0".to_string(),
            msg_type: MessageType::Message,
            from,
            to,
            content: MessageContent::text(text),
            context: MessageContext {
                conversation_id: conversation_id.to_string(),
                relation_type: None,
                scope: None,
                trade_session_id: None,
                thread_id: None,
                reply_to: None,
                capability_id: None,
            },
            metadata: MessageMetadata {
                priority: Priority::Normal,
                created_at: chrono::Utc::now().to_rfc3339(),
                server_received_at: None,
            },
        }
    }

    /// 提取纯文本内容（降级读取，用于向内网 LLM 翻译投递）
    pub fn text_content(&self) -> String {
        self.content.text_content()
    }
}

// ═══════════════════════════════════════════════════════════════════
// 本地消息状态 (纯客户端侧, 不上网)
// ═══════════════════════════════════════════════════════════════════

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

/// 本地消息记录 — 用于 SQLite 持久化的扁平化存储结构
///
/// 这是 `HasnEnvelope` 写入数据库后的"降维表示"。
/// 从网络包到本地存储需要经过 `HasnEnvelope -> HasnMessageRecord` 转换。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HasnMessageRecord {
    /// 消息全局 ID (msg_{ulid})
    pub id: String,
    /// 会话 ID
    pub conversation_id: String,

    /// 发送方 HASN ID
    pub from_hasn_id: String,
    /// 发送方的 owner_id（多租户隔离墙）
    pub from_owner_id: String,
    /// 发送方实体类型 ("human" / "agent")
    pub from_entity_type: String,

    /// 接收方 HASN ID
    pub to_hasn_id: String,
    /// 接收方的 owner_id
    pub to_owner_id: String,

    /// 内容类型 (字符串枚举: "text", "tool_call", ...)
    pub content_type: String,
    /// 内容体 (JSON 字符串)
    pub body: String,

    /// 送达状态 ("sending", "sent", "delivered", "read")
    pub status: String,
    /// 本地发送状态 ("sending", "sent", "failed", "synced")
    pub send_status: SendStatus,

    /// 创建时间 (ISO 8601)
    pub created_at: String,
}

impl HasnMessageRecord {
    /// 从网络 Envelope 转换为本地存储 Record
    pub fn from_envelope(env: &HasnEnvelope) -> Self {
        Self {
            id: env.id.clone(),
            conversation_id: env.context.conversation_id.clone(),
            from_hasn_id: env.from.hasn_id.clone(),
            from_owner_id: env.from.owner_id.clone(),
            from_entity_type: env.from.entity_type.to_string(),
            to_hasn_id: env.to.hasn_id.clone(),
            to_owner_id: env.to.owner_id.clone(),
            content_type: env.content.content_type.to_string(),
            body: env.content.body.to_string(),
            status: "sent".to_string(),
            send_status: SendStatus::Synced,
            created_at: env.metadata.created_at.clone(),
        }
    }

    /// 从本地记录构建精简的 Envelope（用于重放/重发）
    pub fn to_envelope(&self) -> HasnEnvelope {
        let from_type = match self.from_entity_type.as_str() {
            "agent" => EntityType::Agent,
            "system" => EntityType::System,
            _ => EntityType::Human,
        };
        let content_type = match self.content_type.as_str() {
            "image" => ContentType::Image,
            "file" => ContentType::File,
            "voice" => ContentType::Voice,
            "card" => ContentType::Card,
            "capability_request" => ContentType::CapabilityRequest,
            "capability_response" => ContentType::CapabilityResponse,
            "stream_chunk" => ContentType::StreamChunk,
            "tool_call" => ContentType::ToolCall,
            _ => ContentType::Text,
        };
        let body: serde_json::Value =
            serde_json::from_str(&self.body).unwrap_or(serde_json::json!({ "text": self.body }));

        HasnEnvelope {
            id: self.id.clone(),
            version: "2.0".to_string(),
            msg_type: MessageType::Message,
            from: EntityRef {
                hasn_id: self.from_hasn_id.clone(),
                entity_type: from_type,
                owner_id: self.from_owner_id.clone(),
            },
            to: EntityRef {
                hasn_id: self.to_hasn_id.clone(),
                entity_type: EntityType::Human,
                owner_id: self.to_owner_id.clone(),
            },
            content: MessageContent { content_type, body },
            context: MessageContext {
                conversation_id: self.conversation_id.clone(),
                relation_type: None,
                scope: None,
                trade_session_id: None,
                thread_id: None,
                reply_to: None,
                capability_id: None,
            },
            metadata: MessageMetadata {
                priority: Priority::Normal,
                created_at: self.created_at.clone(),
                server_received_at: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_type_serialization() {
        assert_eq!(
            serde_json::to_string(&EntityType::Human).unwrap(),
            "\"human\""
        );
        assert_eq!(
            serde_json::to_string(&EntityType::Agent).unwrap(),
            "\"agent\""
        );
    }

    #[test]
    fn content_type_serialization() {
        assert_eq!(
            serde_json::to_string(&ContentType::StreamChunk).unwrap(),
            "\"stream_chunk\""
        );
        assert_eq!(
            serde_json::to_string(&ContentType::ToolCall).unwrap(),
            "\"tool_call\""
        );
    }

    #[test]
    fn message_content_text_extraction() {
        let c = MessageContent::text("Hello HASN");
        assert_eq!(c.text_content(), "Hello HASN");

        let c = MessageContent::stream_chunk("s1", "delta text", 1, false);
        assert_eq!(c.text_content(), "delta text");

        let c = MessageContent::tool_call("t1", "shell", "执行命令", "running", None, None);
        assert_eq!(c.text_content(), "执行命令");
    }

    #[test]
    fn envelope_round_trip() {
        let env = HasnEnvelope::new_text(
            EntityRef {
                hasn_id: "h_alice".to_string(),
                entity_type: EntityType::Human,
                owner_id: "h_alice".to_string(),
            },
            EntityRef {
                hasn_id: "a_bob_assistant".to_string(),
                entity_type: EntityType::Agent,
                owner_id: "h_bob".to_string(),
            },
            "conv_test",
            "你好",
        );

        assert!(env.id.starts_with("msg_"));
        assert_eq!(env.from.owner_id, "h_alice");
        assert_eq!(env.to.owner_id, "h_bob");
        assert_eq!(env.text_content(), "你好");

        // JSON round-trip
        let json = serde_json::to_string_pretty(&env).unwrap();
        let parsed: HasnEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.text_content(), "你好");
    }

    #[test]
    fn record_from_envelope_round_trip() {
        let env = HasnEnvelope::new_text(
            EntityRef {
                hasn_id: "h_user1".to_string(),
                entity_type: EntityType::Human,
                owner_id: "h_user1".to_string(),
            },
            EntityRef {
                hasn_id: "a_agent1".to_string(),
                entity_type: EntityType::Agent,
                owner_id: "h_user1".to_string(),
            },
            "conv_abc",
            "test round trip",
        );

        let record = HasnMessageRecord::from_envelope(&env);
        assert_eq!(record.from_hasn_id, "h_user1");
        assert_eq!(record.from_owner_id, "h_user1");
        assert_eq!(record.content_type, "text");

        let rebuilt = record.to_envelope();
        assert_eq!(rebuilt.text_content(), "test round trip");
    }
}
