use reqwest::{Client, Method, RequestBuilder};
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::RwLock;

use crate::error::HasnError;
use crate::model::*;

/// 后端响应统一格式
#[derive(Debug, serde::Deserialize)]
struct ApiResponse<T> {
    code: i32,
    msg: Option<String>,
    data: Option<T>,
}

/// HASN HTTP API 客户端
pub struct HasnApiClient {
    http: Client,
    base_url: String,
    hasn_token: RwLock<Option<String>>,
    platform_token: RwLock<Option<String>>,
}

impl HasnApiClient {
    pub fn new(base_url: &str) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("build http client");
        Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            hasn_token: RwLock::new(None),
            platform_token: RwLock::new(None),
        }
    }

    /// 设置 HASN JWT token
    pub async fn set_hasn_token(&self, token: &str) {
        *self.hasn_token.write().await = Some(token.to_string());
    }

    /// 设置平台 access_token
    pub async fn set_platform_token(&self, token: &str) {
        *self.platform_token.write().await = Some(token.to_string());
    }

    /// 构造请求 (自动附 HASN JWT)
    fn hasn_request(&self, method: Method, path: &str) -> RequestBuilder {
        let url = format!("{}/api/v1/hasn{}", self.base_url, path);
        self.http.request(method, &url)
    }

    /// 发起请求并解析响应
    async fn send_hasn<T: DeserializeOwned>(&self, req: RequestBuilder) -> Result<T, HasnError> {
        let token = self.hasn_token.read().await;
        let req = if let Some(t) = token.as_ref() {
            req.header("Authorization", format!("Bearer {}", t))
        } else {
            req
        };

        let resp = req.send().await.map_err(HasnError::Http)?;
        let status = resp.status();
        let body = resp.text().await.map_err(HasnError::Http)?;

        if !status.is_success() {
            return Err(HasnError::Api {
                status: status.as_u16(),
                message: body,
            });
        }

        let api_resp: ApiResponse<T> = serde_json::from_str(&body).map_err(|e| {
            HasnError::Parse(format!(
                "JSON解析失败: {} body={}",
                e,
                &body[..body.len().min(200)]
            ))
        })?;

        if api_resp.code != 200 {
            return Err(HasnError::Api {
                status: api_resp.code as u16,
                message: api_resp.msg.unwrap_or_else(|| "未知错误".to_string()),
            });
        }

        api_resp
            .data
            .ok_or_else(|| HasnError::Parse("响应 data 为空".to_string()))
    }

    // ═══════════════════════════════════
    // 认证
    // ═══════════════════════════════════

    /// HASN 注册 (内部接口, 需要平台token)
    pub async fn hasn_register(
        &self,
        huanxing_user_id: i64,
        nickname: &str,
        phone: Option<&str>,
        agent_name: &str,
    ) -> Result<HasnRegisterResponse, HasnError> {
        let mut body = serde_json::json!({
            "huanxing_user_id": huanxing_user_id,
            "nickname": nickname,
            "agent_name": agent_name,
        });
        if let Some(p) = phone {
            body["phone"] = serde_json::Value::String(p.to_string());
        }

        let req = self
            .hasn_request(Method::POST, "/auth/register")
            .json(&body);
        self.send_hasn(req).await
    }

    // ═══════════════════════════════════
    // 会话
    // ═══════════════════════════════════

    /// 获取会话列表
    pub async fn list_conversations(
        &self,
        limit: i32,
        offset: i32,
    ) -> Result<Vec<HasnConversation>, HasnError> {
        let req = self
            .hasn_request(Method::GET, "/conversations")
            .query(&[("limit", limit.to_string()), ("offset", offset.to_string())]);
        self.send_hasn(req).await
    }

    /// 获取消息历史 (游标分页)
    pub async fn get_messages(
        &self,
        conversation_id: &str,
        before_id: Option<i64>,
        limit: i32,
    ) -> Result<Vec<HasnMessage>, HasnError> {
        let mut params = vec![("limit".to_string(), limit.to_string())];
        if let Some(bid) = before_id {
            params.push(("before_id".to_string(), bid.to_string()));
        }

        let req = self
            .hasn_request(
                Method::GET,
                &format!("/conversations/{}/messages", conversation_id),
            )
            .query(&params);
        self.send_hasn(req).await
    }

    /// 标记会话已读
    pub async fn mark_read(
        &self,
        conversation_id: &str,
        last_msg_id: i64,
    ) -> Result<(), HasnError> {
        let req = self
            .hasn_request(
                Method::POST,
                &format!("/conversations/{}/read", conversation_id),
            )
            .json(&serde_json::json!({ "last_msg_id": last_msg_id }));

        let _: serde_json::Value = self.send_hasn(req).await?;
        Ok(())
    }

    /// 获取未读计数
    pub async fn get_unread_counts(&self) -> Result<HashMap<String, i32>, HasnError> {
        let req = self.hasn_request(Method::GET, "/conversations/unread");
        self.send_hasn(req).await
    }

    // ═══════════════════════════════════
    // 消息
    // ═══════════════════════════════════

    /// 通过 REST API 发送消息
    pub async fn send_message(
        &self,
        to_star_id: &str,
        content: &str,
        content_type: i32,
    ) -> Result<HasnMessageSendResp, HasnError> {
        let req = self
            .hasn_request(Method::POST, "/messages/send")
            .json(&serde_json::json!({
                "to": to_star_id,
                "content": content,
                "content_type": content_type,
            }));
        self.send_hasn(req).await
    }

    /// 离线消息补齐
    pub async fn sync_offline_messages(
        &self,
        conversation_id: &str,
        last_msg_id: Option<i64>,
    ) -> Result<Vec<HasnMessage>, HasnError> {
        let mut params = vec![];
        if let Some(id) = last_msg_id {
            params.push(("last_msg_id", id.to_string()));
        }

        let req = self
            .hasn_request(Method::GET, "/ws/sync")
            .query(&[("conversation_id", conversation_id)])
            .query(&params);
        self.send_hasn(req).await
    }

    // ═══════════════════════════════════
    // 联系人
    // ═══════════════════════════════════

    /// 获取联系人列表
    pub async fn list_contacts(&self, relation_type: &str) -> Result<Vec<HasnContact>, HasnError> {
        let req = self
            .hasn_request(Method::GET, "/contacts")
            .query(&[("relation_type", relation_type)]);
        self.send_hasn(req).await
    }

    /// 发送好友请求
    pub async fn send_friend_request(
        &self,
        target_star_id: &str,
        message: &str,
    ) -> Result<(), HasnError> {
        let req = self
            .hasn_request(Method::POST, "/contacts/request")
            .json(&serde_json::json!({
                "target_star_id": target_star_id,
                "message": message,
            }));
        let _: serde_json::Value = self.send_hasn(req).await?;
        Ok(())
    }

    /// 待处理好友请求
    pub async fn list_pending_requests(&self) -> Result<Vec<FriendRequest>, HasnError> {
        let req = self.hasn_request(Method::GET, "/contacts/requests");
        self.send_hasn(req).await
    }

    /// 接受/拒绝好友请求
    pub async fn respond_friend_request(
        &self,
        request_id: i64,
        action: &str,
    ) -> Result<(), HasnError> {
        let req = self
            .hasn_request(
                Method::PUT,
                &format!("/contacts/requests/{}/respond", request_id),
            )
            .json(&serde_json::json!({ "action": action }));
        let _: serde_json::Value = self.send_hasn(req).await?;
        Ok(())
    }

    /// WebSocket 连接 URL
    pub fn ws_native_url(&self, token: &str) -> String {
        let ws_base = self
            .base_url
            .replace("https://", "wss://")
            .replace("http://", "ws://");
        format!("{}/api/v1/hasn/ws/native?token={}", ws_base, token)
    }
}

/// 发送消息响应
#[derive(Debug, Clone, serde::Deserialize)]
pub struct HasnMessageSendResp {
    pub id: i64,
    pub conversation_id: String,
    pub from_id: String,
    pub from_type: i32,
    pub content: String,
    pub content_type: i32,
    pub status: i32,
    pub created_at: Option<String>,
}
