use super::WeixinChannel;
use super::types::*;
use crate::channels::traits::{ChannelMessage, SendMessage};
use anyhow::{Result, anyhow};
use reqwest::{
    Client,
    header::{CONTENT_TYPE, HeaderMap, HeaderValue},
};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

const ILINK_APP_ID: &str = "";
const ILINK_APP_CLIENT_VERSION: u32 = 65547; // e.g. 1.0.11
const DEFAULT_LONG_POLL_TIMEOUT_MS: u64 = 35_000;
const DEFAULT_API_TIMEOUT_MS: u64 = 15_000;

fn build_headers(token: &str, body_len: usize) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    if let Ok(val) = HeaderValue::from_str("ilink_bot_token") {
        headers.insert("AuthorizationType", val);
    }
    if let Ok(val) = HeaderValue::from_str(&body_len.to_string()) {
        headers.insert("Content-Length", val);
    }

    // X-WECHAT-UIN
    let rand_uint32 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    use base64::{Engine, engine::general_purpose::STANDARD};
    let b64_uin = STANDARD.encode(rand_uint32.to_string().as_bytes());
    if let Ok(val) = HeaderValue::from_str(&b64_uin) {
        headers.insert("X-WECHAT-UIN", val);
    }

    // App Id
    if let Ok(val) = HeaderValue::from_str(ILINK_APP_ID) {
        headers.insert("iLink-App-Id", val);
    }

    // Client version
    if let Ok(val) = HeaderValue::from_str(&ILINK_APP_CLIENT_VERSION.to_string()) {
        headers.insert("iLink-App-ClientVersion", val);
    }

    if !token.is_empty() {
        if let Ok(val) = HeaderValue::from_str(&format!("Bearer {}", token)) {
            headers.insert("Authorization", val);
        }
    }

    headers
}

fn build_base_info() -> BaseInfo {
    BaseInfo {
        channel_version: Some("1.0.11".to_string()),
    }
}

pub async fn send_message(channel: &WeixinChannel, message: &SendMessage) -> Result<()> {
    tracing::debug!("Weixin send_message to: {}", message.recipient);

    let client = Client::builder()
        .timeout(Duration::from_millis(DEFAULT_API_TIMEOUT_MS))
        .build()?;

    let base_url = if channel.base_url.ends_with('/') {
        channel.base_url.clone()
    } else {
        format!("{}/", channel.base_url)
    };

    let url = format!("{}ilink/bot/sendmessage", base_url);

    let client_id = Uuid::new_v4().to_string();

    // Text item
    let text_item = MessageItem {
        item_type: Some(MESSAGE_ITEM_TYPE_TEXT),
        text_item: Some(TextItem {
            text: Some(message.content.clone()),
        }),
        ..Default::default()
    };

    // We only send text for now
    let msg = SendMessageReqMsg {
        from_user_id: Some("".to_string()),
        to_user_id: Some(message.recipient.clone()),
        client_id: Some(client_id),
        message_type: Some(MESSAGE_TYPE_BOT),
        message_state: Some(MESSAGE_STATE_FINISH),
        item_list: Some(vec![text_item]),
        context_token: None, // We don't have it explicitly yet, maybe pull from memory if strictly needed?
    };

    let req = SendMessageReq {
        msg: Some(msg),
        base_info: Some(build_base_info()),
    };

    let body_str = serde_json::to_string(&req)?;
    let headers = build_headers(&channel.bot_token, body_str.len());

    let resp = client
        .post(&url)
        .headers(headers)
        .body(body_str)
        .send()
        .await?;

    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        tracing::error!("send_message failed: {} {}", status, text);
        return Err(anyhow!("send_message failed: {} {}", status, text));
    }

    Ok(())
}

pub async fn get_upload_url(
    channel: &WeixinChannel,
    mut req: GetUploadUrlReq,
) -> Result<GetUploadUrlResp> {
    req.base_info = Some(build_base_info());
    let client = Client::builder()
        .timeout(Duration::from_millis(DEFAULT_API_TIMEOUT_MS))
        .build()?;

    let base_url = if channel.base_url.ends_with('/') {
        channel.base_url.clone()
    } else {
        format!("{}/", channel.base_url)
    };
    let url = format!("{}ilink/bot/getuploadurl", base_url);

    let body_str = serde_json::to_string(&req)?;
    let headers = build_headers(&channel.bot_token, body_str.len());

    let resp = client
        .post(&url)
        .headers(headers)
        .body(body_str)
        .send()
        .await?;

    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        tracing::error!("get_upload_url failed: {} {}", status, text);
        return Err(anyhow!("get_upload_url failed: {} {}", status, text));
    }

    let upload_resp: GetUploadUrlResp = serde_json::from_str(&text)?;
    Ok(upload_resp)
}

pub async fn send_media_message(
    channel: &WeixinChannel,
    recipient: &str,
    media_item: MessageItem,
) -> Result<()> {
    tracing::debug!("Weixin send_media_message to: {}", recipient);

    let client = Client::builder()
        .timeout(Duration::from_millis(DEFAULT_API_TIMEOUT_MS))
        .build()?;

    let base_url = if channel.base_url.ends_with('/') {
        channel.base_url.clone()
    } else {
        format!("{}/", channel.base_url)
    };

    let url = format!("{}ilink/bot/sendmessage", base_url);
    let client_id = Uuid::new_v4().to_string();

    let msg = SendMessageReqMsg {
        from_user_id: Some("".to_string()),
        to_user_id: Some(recipient.to_string()),
        client_id: Some(client_id),
        message_type: Some(MESSAGE_TYPE_BOT),
        message_state: Some(MESSAGE_STATE_FINISH),
        item_list: Some(vec![media_item]),
        context_token: None,
    };

    let req = SendMessageReq {
        msg: Some(msg),
        base_info: Some(build_base_info()),
    };

    let body_str = serde_json::to_string(&req)?;
    let headers = build_headers(&channel.bot_token, body_str.len());

    let resp = client
        .post(&url)
        .headers(headers)
        .body(body_str)
        .send()
        .await?;

    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        tracing::error!("send_media_message failed: {} {}", status, text);
        return Err(anyhow!("send_media_message failed: {} {}", status, text));
    }
    Ok(())
}

pub async fn get_updates_loop(
    channel: Arc<WeixinChannel>,
    tx: tokio::sync::mpsc::Sender<ChannelMessage>,
) {
    let client = match Client::builder().build() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to build HTTP client for weixin loop: {}", e);
            return;
        }
    };

    let base_url = if channel.base_url.ends_with('/') {
        channel.base_url.clone()
    } else {
        format!("{}/", channel.base_url)
    };

    let url = format!("{}ilink/bot/getupdates", base_url);
    let mut get_updates_buf = String::new();

    loop {
        let req = GetUpdatesReq {
            sync_buf: None,
            get_updates_buf: Some(get_updates_buf.clone()),
            base_info: Some(build_base_info()),
        };

        let body_str = match serde_json::to_string(&req) {
            Ok(b) => b,
            Err(e) => {
                tracing::error!("Weixin serialize payload error: {}", e);
                tokio::time::sleep(Duration::from_secs(3)).await;
                continue;
            }
        };

        let headers = build_headers(&channel.bot_token, body_str.len());

        // Timeout is slightly larger than the standard long poll
        let req_timeout = Duration::from_millis(DEFAULT_LONG_POLL_TIMEOUT_MS + 2000);

        let resp_result = client
            .post(&url)
            .headers(headers)
            .body(body_str)
            .timeout(req_timeout)
            .send()
            .await;

        match resp_result {
            Ok(resp) => {
                if !resp.status().is_success() {
                    let status = resp.status();
                    let text = resp.text().await.unwrap_or_default();
                    tracing::error!("getUpdates API error: {} - {}", status, text);
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    continue;
                }

                let text = match resp.text().await {
                    Ok(t) => t,
                    Err(e) => {
                        tracing::error!("getUpdates read body error: {}", e);
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        continue;
                    }
                };

                let get_updates_resp: GetUpdatesResp = match serde_json::from_str(&text) {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::error!("getUpdates JSON parse error: {}", e);
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        continue;
                    }
                };

                // Keep cursor updated
                if let Some(buf) = get_updates_resp.get_updates_buf {
                    get_updates_buf = buf;
                }

                if let Some(msgs) = get_updates_resp.msgs {
                    for wmsg in msgs {
                        if wmsg.message_type != Some(MESSAGE_TYPE_USER) {
                            continue; // skip bot's own messages or none
                        }

                        let sender_id = wmsg.from_user_id.unwrap_or_default();
                        if sender_id.is_empty() {
                            continue;
                        }

                        let items = wmsg.item_list.unwrap_or_default();
                        for item in items {
                            if item.item_type == Some(MESSAGE_ITEM_TYPE_TEXT) {
                                if let Some(txt_item) = item.text_item {
                                    if let Some(t) = txt_item.text {
                                        let ts = wmsg.create_time_ms.unwrap_or_else(|| {
                                            std::time::SystemTime::now()
                                                .duration_since(std::time::UNIX_EPOCH)
                                                .unwrap()
                                                .as_millis()
                                                as u64
                                        });

                                        let msg_id = item
                                            .msg_id
                                            .unwrap_or_else(|| Uuid::new_v4().to_string());

                                        let cm = ChannelMessage {
                                            id: msg_id,
                                            sender: sender_id.clone(),
                                            reply_target: sender_id.clone(),
                                            content: t,
                                            channel: "weixin".to_string(),
                                            timestamp: ts,
                                            thread_ts: None,
                                            interruption_scope_id: None,
                                            attachments: vec![],
                                        };

                                        if let Err(e) = tx.send(cm).await {
                                            tracing::error!(
                                                "Failed to route weixin message to bus: {}",
                                                e
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                if e.is_timeout() {
                    tracing::debug!("getUpdates long-poll timeout, retrying gracefully...");
                } else {
                    tracing::error!("getUpdates network error: {}", e);
                    tokio::time::sleep(Duration::from_secs(3)).await;
                }
            }
        }
    }
}
