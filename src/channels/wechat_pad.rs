use super::traits::{Channel, ChannelMessage, SendMessage};
use crate::huanxing::config::WechatPadConfig;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio::time::Duration;

// ── WeChatPadPro REST API endpoints ─────────────────────────────
const SEND_TEXT_MSG: &str = "/msg/SendTextMsg";
const SEND_IMAGE_MSG: &str = "/msg/SendImageMsg";
const GET_LOGIN_STATUS: &str = "/api/login/GetLoginStatus";

const DEDUP_CAPACITY: usize = 10_000;

fn current_unix_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn normalize_token(raw: &str) -> Option<String> {
    let token = raw.trim();
    (!token.is_empty()).then(|| token.to_string())
}

// ── Webhook payload types ───────────────────────────────────────

/// Top-level webhook callback from WeChatPadPro.
#[derive(Debug, serde::Deserialize)]
struct WebhookPayload {
    /// Event type (e.g. "message")
    #[serde(default)]
    event_type: Option<String>,
    /// Unix timestamp
    #[serde(default)]
    timestamp: Option<u64>,
    /// HMAC-SHA256 signature
    #[serde(default)]
    signature: Option<String>,
    /// Event payload
    #[serde(default)]
    data: Value,
    // Some WeChatPadPro versions put fields at top level instead of in `data`
    #[serde(rename = "MsgType", default)]
    msg_type: Option<i64>,
    #[serde(rename = "FromUserName", default)]
    from_user_name: Option<String>,
    #[serde(rename = "ToUserName", default)]
    to_user_name: Option<String>,
    #[serde(rename = "Content", default)]
    content: Option<String>,
    #[serde(rename = "MsgId", default)]
    msg_id: Option<i64>,
    #[serde(rename = "ImgBuf", default)]
    img_buf: Option<Value>,
}

impl WebhookPayload {
    /// Normalize the payload so message fields are accessible uniformly.
    fn msg_type_val(&self) -> Option<i64> {
        self.msg_type.or_else(|| {
            self.data
                .get("MsgType")
                .and_then(Value::as_i64)
        })
    }

    fn from_user(&self) -> Option<&str> {
        self.from_user_name.as_deref().or_else(|| {
            self.data
                .get("FromUserName")
                .and_then(Value::as_str)
        })
    }

    fn to_user(&self) -> Option<&str> {
        self.to_user_name.as_deref().or_else(|| {
            self.data
                .get("ToUserName")
                .and_then(Value::as_str)
        })
    }

    fn content_text(&self) -> Option<&str> {
        self.content.as_deref().or_else(|| {
            self.data
                .get("Content")
                .and_then(Value::as_str)
        })
    }

    fn message_id(&self) -> String {
        self.msg_id
            .or_else(|| self.data.get("MsgId").and_then(Value::as_i64))
            .map(|id| id.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
    }

    fn timestamp_val(&self) -> u64 {
        self.timestamp
            .or_else(|| {
                self.data
                    .get("CreateTime")
                    .and_then(Value::as_u64)
            })
            .unwrap_or_else(current_unix_timestamp_secs)
    }

    fn image_url(&self) -> Option<String> {
        // Try nested data.ImgBuf.url or data.ImgUrl
        self.data
            .get("ImgBuf")
            .and_then(|v| v.get("url"))
            .and_then(Value::as_str)
            .map(String::from)
            .or_else(|| {
                self.data
                    .get("ImgUrl")
                    .and_then(Value::as_str)
                    .map(String::from)
            })
            .or_else(|| {
                self.img_buf
                    .as_ref()
                    .and_then(|v| v.get("url"))
                    .and_then(Value::as_str)
                    .map(String::from)
            })
    }
}

/// Whether the from wxid is a group (room) chat.
fn is_group_wxid(wxid: &str) -> bool {
    wxid.ends_with("@chatroom")
}

/// Extract the actual sender wxid from group message content.
/// Group messages from WeChatPadPro often have format:
///   "sender_wxid:\nactual content"
fn extract_group_sender_and_content(from: &str, content: &str) -> (String, String) {
    if !is_group_wxid(from) {
        return (from.to_string(), content.to_string());
    }
    // In group messages, Content format is: "wxid_xxx:\n actual message"
    if let Some(idx) = content.find(":\n") {
        let sender = content[..idx].trim().to_string();
        let body = content[idx + 2..].trim().to_string();
        if !sender.is_empty() && !body.is_empty() {
            return (sender, body);
        }
    }
    (from.to_string(), content.to_string())
}

/// Strip @-mention from message content. Returns the cleaned content.
fn strip_at_mention(content: &str) -> String {
    // WeChat @mentions look like: "@botname " or "@botname\u{2005}"
    let re_start = content
        .find('@')
        .and_then(|start| {
            // Find the space/separator after the mention
            let rest = &content[start + 1..];
            let end = rest
                .find(|c: char| c == ' ' || c == '\u{2005}' || c == '\n')
                .map(|i| start + 1 + i + 1)
                .unwrap_or(content.len());
            Some(end)
        });

    match re_start {
        Some(end) => {
            let mut result = String::new();
            result.push_str(content[..content.find('@').unwrap_or(0)].trim());
            if end < content.len() {
                if !result.is_empty() {
                    result.push(' ');
                }
                result.push_str(content[end..].trim());
            }
            if result.is_empty() {
                content.to_string()
            } else {
                result
            }
        }
        None => content.to_string(),
    }
}

// ── Webhook signature verification ──────────────────────────────

#[cfg(feature = "huanxing")]
fn verify_webhook_signature(body: &[u8], secret: &str, signature: &str) -> bool {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;

    let Ok(mut mac) = HmacSha256::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(body);

    let expected = hex::encode(mac.finalize().into_bytes());
    expected == signature
}

#[cfg(not(feature = "huanxing"))]
fn verify_webhook_signature(_body: &[u8], _secret: &str, _signature: &str) -> bool {
    true
}

// ── Channel implementation ──────────────────────────────────────

pub struct WechatPadChannel {
    api_base_url: String,
    admin_key: String,
    token: Option<String>,
    webhook_bind: String,
    webhook_secret: Option<String>,
    wxid: Option<String>,
    allowed_users: Vec<String>,
    allowed_groups: Vec<String>,
    group_at_only: bool,
    dedup: Arc<RwLock<HashSet<String>>>,
}

impl WechatPadChannel {
    pub fn from_config(config: WechatPadConfig) -> Result<Self> {
        let api_base_url = config.api_base_url.trim().trim_end_matches('/').to_string();
        if api_base_url.is_empty() {
            anyhow::bail!("wechat_pad.api_base_url cannot be empty");
        }
        let admin_key = config.admin_key.trim().to_string();
        if admin_key.is_empty() {
            anyhow::bail!("wechat_pad.admin_key cannot be empty");
        }

        Ok(Self {
            api_base_url,
            admin_key,
            token: normalize_token(config.token.as_deref().unwrap_or_default()),
            webhook_bind: config.webhook_bind.trim().to_string(),
            webhook_secret: config
                .webhook_secret
                .as_deref()
                .and_then(|s| normalize_token(s)),
            wxid: config.wxid.as_deref().and_then(|s| normalize_token(s)),
            allowed_users: config.allowed_users,
            allowed_groups: config.allowed_groups,
            group_at_only: config.group_at_only,
            dedup: Arc::new(RwLock::new(HashSet::new())),
        })
    }

    fn is_user_allowed(&self, user_id: &str) -> bool {
        self.allowed_users.iter().any(|u| u == "*" || u == user_id)
    }

    fn is_group_allowed(&self, group_id: &str) -> bool {
        if self.allowed_groups.is_empty() {
            return false;
        }
        self.allowed_groups
            .iter()
            .any(|g| g == "*" || g == group_id)
    }

    fn is_self_message(&self, from: &str) -> bool {
        match &self.wxid {
            Some(wxid) => wxid == from,
            None => false,
        }
    }

    async fn is_duplicate(&self, message_id: &str) -> bool {
        if message_id.is_empty() {
            return false;
        }
        let mut dedup = self.dedup.write().await;
        if dedup.contains(message_id) {
            return true;
        }
        if dedup.len() >= DEDUP_CAPACITY {
            let remove_n = dedup.len() / 2;
            let to_remove: Vec<String> = dedup.iter().take(remove_n).cloned().collect();
            for key in to_remove {
                dedup.remove(&key);
            }
        }
        dedup.insert(message_id.to_string());
        false
    }

    fn http_client(&self) -> reqwest::Client {
        crate::config::build_runtime_proxy_client("channel.wechat_pad")
    }

    fn auth_header_value(&self) -> String {
        self.token
            .as_deref()
            .unwrap_or(&self.admin_key)
            .to_string()
    }

    async fn post_api(&self, endpoint: &str, body: &Value) -> Result<Value> {
        let url = format!("{}{}", self.api_base_url, endpoint);
        let response = self
            .http_client()
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", self.auth_header_value())
            .json(body)
            .timeout(Duration::from_secs(15))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let err = response.text().await.unwrap_or_default();
            let sanitized = crate::providers::sanitize_api_error(&err);
            anyhow::bail!("WeChatPad API request failed ({status}): {sanitized}");
        }

        let payload: Value = response.json().await.unwrap_or_else(|_| json!({}));
        Ok(payload)
    }

    fn parse_webhook_event(&self, payload: &WebhookPayload) -> Option<ChannelMessage> {
        let msg_type = payload.msg_type_val()?;
        let from = payload.from_user()?;

        // Filter out self-sent messages
        if self.is_self_message(from) {
            return None;
        }

        let message_id = payload.message_id();

        let is_group = is_group_wxid(from);

        // Auth checks
        if is_group {
            if !self.is_group_allowed(from) {
                tracing::debug!("WeChatPad: ignoring message from unauthorized group: {from}");
                return None;
            }
        }

        // Build content based on message type
        let raw_content = match msg_type {
            // Text message
            1 => payload.content_text().unwrap_or("").to_string(),
            // Image message
            3 => {
                if let Some(url) = payload.image_url() {
                    format!("[IMAGE:{url}]")
                } else {
                    "[收到一张图片]".to_string()
                }
            }
            // Voice message
            34 => "[收到一条语音消息]".to_string(),
            // Emoji sticker
            47 => return None,
            // App message (file, link)
            49 => {
                let title = payload
                    .data
                    .get("FileName")
                    .and_then(Value::as_str)
                    .or_else(|| {
                        payload
                            .data
                            .get("Content")
                            .and_then(Value::as_str)
                    })
                    .unwrap_or("[收到一个应用消息]");
                format!("[文件/链接: {title}]")
            }
            // System notification
            10000 | 10002 => return None,
            _ => return None,
        };

        if raw_content.trim().is_empty() {
            return None;
        }

        // For group messages, extract actual sender and clean content
        let (sender_id, content) = if is_group {
            let (sender, body) = extract_group_sender_and_content(from, &raw_content);

            // Check if group_at_only and verify @-mention
            if self.group_at_only {
                if let Some(ref wxid) = self.wxid {
                    let has_at = body.contains(&format!("@{wxid}"))
                        || body.contains("@所有人");
                    if !has_at {
                        // Check if Content contains @mention in some form
                        let has_informal_at = body.starts_with('@');
                        if !has_informal_at {
                            return None;
                        }
                    }
                }
            }

            let cleaned = strip_at_mention(&body);
            (sender, cleaned)
        } else {
            (from.to_string(), raw_content)
        };

        // Check user authorization
        if !self.is_user_allowed(&sender_id) {
            tracing::debug!("WeChatPad: ignoring message from unauthorized user: {sender_id}");
            return None;
        }

        let content = content.trim().to_string();
        if content.is_empty() {
            return None;
        }

        let reply_target = if is_group {
            format!("group:{from}")
        } else {
            format!("user:{sender_id}")
        };

        Some(ChannelMessage {
            id: message_id.clone(),
            sender: sender_id,
            reply_target,
            content,
            channel: "wechat_pad".to_string(),
            timestamp: payload.timestamp_val(),
            thread_ts: Some(message_id),
            interruption_scope_id: None,
        })
    }
}

#[async_trait]
impl Channel for WechatPadChannel {
    fn name(&self) -> &str {
        "wechat_pad"
    }

    async fn send(&self, message: &SendMessage) -> Result<()> {
        let content = message.content.trim();
        if content.is_empty() {
            return Ok(());
        }

        // Determine recipient wxid
        let to_wxid = message
            .recipient
            .strip_prefix("group:")
            .or_else(|| message.recipient.strip_prefix("user:"))
            .unwrap_or(&message.recipient)
            .trim();

        if to_wxid.is_empty() {
            anyhow::bail!("WeChatPad recipient is empty");
        }

        // Handle image markers
        let mut text_parts = Vec::new();
        for line in content.lines() {
            let trimmed = line.trim();
            if let Some(marker) = trimmed
                .strip_prefix("[IMAGE:")
                .and_then(|v| v.strip_suffix(']'))
                .map(str::trim)
                .filter(|v| !v.is_empty())
            {
                // Send image
                let body = json!({
                    "toUserName": to_wxid,
                    "imgUrl": marker,
                });
                if let Err(e) = self.post_api(SEND_IMAGE_MSG, &body).await {
                    tracing::warn!("WeChatPad: failed to send image: {e}");
                }
                continue;
            }
            text_parts.push(line);
        }

        let text_content = text_parts.join("\n").trim().to_string();
        if !text_content.is_empty() {
            let body = json!({
                "toUserName": to_wxid,
                "content": text_content,
            });
            self.post_api(SEND_TEXT_MSG, &body).await?;
        }

        Ok(())
    }

    async fn listen(&self, tx: tokio::sync::mpsc::Sender<ChannelMessage>) -> Result<()> {
        use axum::{routing::post, Router};

        let tx = Arc::new(tx);
        let channel = Arc::new(WechatPadListenerState {
            channel_config: ChannelConfigSnapshot {
                webhook_secret: self.webhook_secret.clone(),
                wxid: self.wxid.clone(),
                allowed_users: self.allowed_users.clone(),
                allowed_groups: self.allowed_groups.clone(),
                group_at_only: self.group_at_only,
            },
            dedup: self.dedup.clone(),
            tx: tx.clone(),
        });

        let app = Router::new()
            .route("/webhook", post(webhook_handler))
            .route("/", post(webhook_handler))
            .with_state(channel);

        let bind_addr: std::net::SocketAddr = self
            .webhook_bind
            .parse()
            .map_err(|e| anyhow!("Invalid webhook_bind address '{}': {e}", self.webhook_bind))?;

        tracing::info!("WeChatPad: webhook listener starting on {bind_addr}");

        let listener = tokio::net::TcpListener::bind(bind_addr)
            .await
            .map_err(|e| anyhow!("Failed to bind WeChatPad webhook listener on {bind_addr}: {e}"))?;

        axum::serve(listener, app)
            .await
            .map_err(|e| anyhow!("WeChatPad webhook server error: {e}"))?;

        Ok(())
    }

    async fn health_check(&self) -> bool {
        match self.post_api(GET_LOGIN_STATUS, &json!({})).await {
            Ok(resp) => {
                // Check login status from response
                resp.get("data")
                    .and_then(|d| d.get("online"))
                    .and_then(Value::as_bool)
                    .unwrap_or_else(|| {
                        // Fallback: if we got a successful HTTP response, consider it healthy
                        true
                    })
            }
            Err(_) => false,
        }
    }
}

// ── Axum webhook handler ────────────────────────────────────────

/// Snapshot of config fields needed by the webhook handler.
#[derive(Clone)]
struct ChannelConfigSnapshot {
    webhook_secret: Option<String>,
    wxid: Option<String>,
    allowed_users: Vec<String>,
    allowed_groups: Vec<String>,
    group_at_only: bool,
}

struct WechatPadListenerState {
    channel_config: ChannelConfigSnapshot,
    dedup: Arc<RwLock<HashSet<String>>>,
    tx: Arc<tokio::sync::mpsc::Sender<ChannelMessage>>,
}

async fn webhook_handler(
    axum::extract::State(state): axum::extract::State<Arc<WechatPadListenerState>>,
    body: axum::body::Bytes,
) -> axum::http::StatusCode {
    // Parse JSON payload
    let payload: WebhookPayload = match serde_json::from_slice(&body) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("WeChatPad webhook: failed to parse payload: {e}");
            return axum::http::StatusCode::BAD_REQUEST;
        }
    };

    // Verify webhook signature if secret is configured
    if let Some(ref secret) = state.channel_config.webhook_secret {
        let signature = payload.signature.as_deref().unwrap_or("");
        if !verify_webhook_signature(&body, secret, signature) {
            tracing::warn!("WeChatPad webhook: signature verification failed");
            return axum::http::StatusCode::UNAUTHORIZED;
        }
    }

    // Build a temporary channel reference for parsing
    let temp_channel = WechatPadChannel {
        api_base_url: String::new(),
        admin_key: String::new(),
        token: None,
        webhook_bind: String::new(),
        webhook_secret: state.channel_config.webhook_secret.clone(),
        wxid: state.channel_config.wxid.clone(),
        allowed_users: state.channel_config.allowed_users.clone(),
        allowed_groups: state.channel_config.allowed_groups.clone(),
        group_at_only: state.channel_config.group_at_only,
        dedup: state.dedup.clone(),
    };

    // Check dedup
    let message_id = payload.message_id();
    if temp_channel.is_duplicate(&message_id).await {
        return axum::http::StatusCode::OK;
    }

    // Parse webhook event into ChannelMessage
    if let Some(msg) = temp_channel.parse_webhook_event(&payload) {
        if state.tx.send(msg).await.is_err() {
            tracing::warn!("WeChatPad webhook: channel sender closed");
        }
    }

    axum::http::StatusCode::OK
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> WechatPadConfig {
        WechatPadConfig {
            api_base_url: "http://127.0.0.1:8849".into(),
            admin_key: "test_key".into(),
            token: None,
            webhook_bind: "0.0.0.0:9850".into(),
            webhook_secret: None,
            wxid: Some("wxid_bot".into()),
            allowed_users: vec!["*".into()],
            allowed_groups: vec!["test_group@chatroom".into()],
            group_at_only: true,
            rate_limit_per_minute: 20,
        }
    }

    #[test]
    fn channel_name() {
        let channel = WechatPadChannel::from_config(test_config()).unwrap();
        assert_eq!(channel.name(), "wechat_pad");
    }

    #[test]
    fn parse_text_message() {
        let channel = WechatPadChannel::from_config(test_config()).unwrap();
        let payload = WebhookPayload {
            event_type: Some("message".into()),
            timestamp: Some(1700000000),
            signature: None,
            data: json!({}),
            msg_type: Some(1),
            from_user_name: Some("wxid_user1".into()),
            to_user_name: Some("wxid_bot".into()),
            content: Some("Hello World".into()),
            msg_id: Some(12345),
            img_buf: None,
        };

        let msg = channel.parse_webhook_event(&payload).unwrap();
        assert_eq!(msg.channel, "wechat_pad");
        assert_eq!(msg.sender, "wxid_user1");
        assert_eq!(msg.reply_target, "user:wxid_user1");
        assert_eq!(msg.content, "Hello World");
        assert_eq!(msg.id, "12345");
    }

    #[test]
    fn parse_image_message() {
        let channel = WechatPadChannel::from_config(test_config()).unwrap();
        let payload = WebhookPayload {
            event_type: Some("message".into()),
            timestamp: Some(1700000000),
            signature: None,
            data: json!({
                "ImgUrl": "https://example.com/photo.jpg"
            }),
            msg_type: Some(3),
            from_user_name: Some("wxid_user1".into()),
            to_user_name: Some("wxid_bot".into()),
            content: None,
            msg_id: Some(12346),
            img_buf: None,
        };

        let msg = channel.parse_webhook_event(&payload).unwrap();
        assert!(msg.content.contains("[IMAGE:https://example.com/photo.jpg]"));
    }

    #[test]
    fn filter_self_message() {
        let channel = WechatPadChannel::from_config(test_config()).unwrap();
        let payload = WebhookPayload {
            event_type: Some("message".into()),
            timestamp: Some(1700000000),
            signature: None,
            data: json!({}),
            msg_type: Some(1),
            from_user_name: Some("wxid_bot".into()),
            to_user_name: Some("wxid_user1".into()),
            content: Some("Hi".into()),
            msg_id: Some(12347),
            img_buf: None,
        };

        assert!(channel.parse_webhook_event(&payload).is_none());
    }

    #[test]
    fn parse_group_message_with_sender() {
        let mut cfg = test_config();
        cfg.group_at_only = false;
        let channel = WechatPadChannel::from_config(cfg).unwrap();
        let payload = WebhookPayload {
            event_type: Some("message".into()),
            timestamp: Some(1700000000),
            signature: None,
            data: json!({}),
            msg_type: Some(1),
            from_user_name: Some("test_group@chatroom".into()),
            to_user_name: Some("wxid_bot".into()),
            content: Some("wxid_sender:\nHello group".into()),
            msg_id: Some(12348),
            img_buf: None,
        };

        let msg = channel.parse_webhook_event(&payload).unwrap();
        assert_eq!(msg.sender, "wxid_sender");
        assert_eq!(msg.reply_target, "group:test_group@chatroom");
        assert_eq!(msg.content, "Hello group");
    }

    #[test]
    fn group_at_only_filters_non_mention() {
        let channel = WechatPadChannel::from_config(test_config()).unwrap();
        let payload = WebhookPayload {
            event_type: Some("message".into()),
            timestamp: Some(1700000000),
            signature: None,
            data: json!({}),
            msg_type: Some(1),
            from_user_name: Some("test_group@chatroom".into()),
            to_user_name: Some("wxid_bot".into()),
            content: Some("wxid_sender:\nHello everyone".into()),
            msg_id: Some(12349),
            img_buf: None,
        };

        // Should be filtered because group_at_only=true and no @mention
        assert!(channel.parse_webhook_event(&payload).is_none());
    }

    #[test]
    fn user_allowed_wildcard() {
        let channel = WechatPadChannel::from_config(test_config()).unwrap();
        assert!(channel.is_user_allowed("any_user"));
    }

    #[test]
    fn user_allowed_specific() {
        let mut cfg = test_config();
        cfg.allowed_users = vec!["wxid_specific".into()];
        let channel = WechatPadChannel::from_config(cfg).unwrap();
        assert!(channel.is_user_allowed("wxid_specific"));
        assert!(!channel.is_user_allowed("wxid_other"));
    }

    #[test]
    fn ignore_system_notification() {
        let channel = WechatPadChannel::from_config(test_config()).unwrap();
        let payload = WebhookPayload {
            event_type: Some("message".into()),
            timestamp: Some(1700000000),
            signature: None,
            data: json!({}),
            msg_type: Some(10000),
            from_user_name: Some("wxid_user1".into()),
            to_user_name: Some("wxid_bot".into()),
            content: Some("系统通知".into()),
            msg_id: Some(12350),
            img_buf: None,
        };

        assert!(channel.parse_webhook_event(&payload).is_none());
    }

    #[test]
    fn is_group_wxid_check() {
        assert!(is_group_wxid("12345@chatroom"));
        assert!(!is_group_wxid("wxid_user1"));
    }

    #[test]
    fn extract_group_sender() {
        let (sender, content) =
            extract_group_sender_and_content("group@chatroom", "wxid_abc:\nHello world");
        assert_eq!(sender, "wxid_abc");
        assert_eq!(content, "Hello world");
    }

    #[test]
    fn strip_at_mention_basic() {
        let cleaned = strip_at_mention("@botname Hello world");
        assert_eq!(cleaned, "Hello world");
    }

    #[test]
    fn config_validation_empty_url() {
        let mut cfg = test_config();
        cfg.api_base_url = "".into();
        assert!(WechatPadChannel::from_config(cfg).is_err());
    }

    #[test]
    fn config_validation_empty_key() {
        let mut cfg = test_config();
        cfg.admin_key = "".into();
        assert!(WechatPadChannel::from_config(cfg).is_err());
    }

    #[tokio::test]
    async fn dedup_works() {
        let channel = WechatPadChannel::from_config(test_config()).unwrap();
        assert!(!channel.is_duplicate("msg_1").await);
        assert!(channel.is_duplicate("msg_1").await);
        assert!(!channel.is_duplicate("msg_2").await);
    }
}
