//! HASN (HuanXing Agent Social Network) tools (Phase 4).
//!
//! 5 tools for social networking between AI agents:
//! - hasn_send: Send message to another agent
//! - hasn_contacts: List contacts
//! - hasn_add_friend: Send friend request
//! - hasn_inbox: View pending friend requests
//! - hasn_respond_request: Accept/reject friend request
//!
//! HASN credentials are stored in agent workspace: `.hasn/api_key` + `.hasn/identity.json`

use crate::huanxing::api_client::ApiClient;
use crate::tools::traits::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::path::PathBuf;

/// Read HASN credentials from agent workspace.
fn read_hasn_creds(workspace: &std::path::Path) -> Option<(String, String, String)> {
    let api_key_path = workspace.join(".hasn").join("api_key");
    let identity_path = workspace.join(".hasn").join("identity.json");

    let api_key = std::fs::read_to_string(&api_key_path)
        .ok()?
        .trim()
        .to_string();
    let identity: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&identity_path).ok()?).ok()?;

    let hasn_id = identity["hasn_id"]
        .as_str()
        .or_else(|| identity["hasnId"].as_str())?
        .to_string();
    let star_id = identity["star_id"]
        .as_str()
        .or_else(|| identity["starId"].as_str())?
        .to_string();

    Some((api_key, hasn_id, star_id))
}

fn resolve_workspace(agents_dir: &std::path::Path, agent_id: &str) -> PathBuf {
    crate::tools::get_active_workspace().unwrap_or_else(|| agents_dir.join(agent_id))
}

// ═══════════════════════════════════════════════════════
// HASN Social Tools (5)
// ═══════════════════════════════════════════════════════

// ── hasn_send ────────────────────────────────────────

pub struct HasnSend {
    api: ApiClient,
    agents_dir: PathBuf,
    hasn_base_url: String,
}

impl HasnSend {
    pub fn new(api: ApiClient, agents_dir: PathBuf, hasn_base_url: String) -> Self {
        Self {
            api,
            agents_dir,
            hasn_base_url,
        }
    }
}

#[async_trait]
impl Tool for HasnSend {
    fn name(&self) -> &str {
        "hasn_send"
    }
    fn description(&self) -> &str {
        "通过HASN社交网络给指定唤星号发消息。需要双方已是好友。"
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "agent_id": { "type": "string", "description": "发送方 Agent ID" },
                "to": { "type": "string", "description": "目标唤星号（如 200001）" },
                "message": { "type": "string", "description": "消息内容" }
            },
            "required": ["agent_id", "to", "message"]
        })
    }
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let agent_id = args["agent_id"].as_str().unwrap_or_default();
        let to = args["to"].as_str().unwrap_or_default();
        let message = args["message"].as_str().unwrap_or_default();

        let workspace = resolve_workspace(&self.agents_dir, agent_id);
        let (api_key, _hasn_id, _star_id) = match read_hasn_creds(&workspace) {
            Some(c) => c,
            None => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("未找到 HASN 凭证。请先完成 HASN 身份注册。".to_string()),
                })
            }
        };

        let url = format!("{}/api/v1/hasn/messages/send", self.hasn_base_url);
        let resp = reqwest::Client::new()
            .post(&url)
            .header("Authorization", format!("ApiKey {api_key}"))
            .header("Content-Type", "application/json")
            .json(&json!({ "to": to, "content": message, "content_type": 1 }))
            .send()
            .await;

        match resp {
            Ok(r) => {
                let body: serde_json::Value = r.json().await.unwrap_or(json!({}));
                if body["code"].as_i64() == Some(200) {
                    Ok(ToolResult {
                        success: true,
                        output: json!({
                            "sent": true,
                            "to": to,
                            "msg_id": body["data"]["id"],
                        })
                        .to_string(),
                        error: None,
                    })
                } else {
                    Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!(
                            "HASN发送失败: {}",
                            body["msg"].as_str().unwrap_or("unknown")
                        )),
                    })
                }
            }
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("HASN请求失败: {e}")),
            }),
        }
    }
}

// ── hasn_contacts ────────────────────────────────────

pub struct HasnContacts {
    agents_dir: PathBuf,
    hasn_base_url: String,
}

impl HasnContacts {
    pub fn new(agents_dir: PathBuf, hasn_base_url: String) -> Self {
        Self {
            agents_dir,
            hasn_base_url,
        }
    }
}

#[async_trait]
impl Tool for HasnContacts {
    fn name(&self) -> &str {
        "hasn_contacts"
    }
    fn description(&self) -> &str {
        "查看HASN社交网络上的联系人列表。"
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "agent_id": { "type": "string", "description": "Agent ID" },
                "relation_type": { "type": "string", "description": "关系类型: social(默认) / commerce / service" }
            },
            "required": ["agent_id"]
        })
    }
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let agent_id = args["agent_id"].as_str().unwrap_or_default();
        let rt = args["relation_type"].as_str().unwrap_or("social");

        let workspace = resolve_workspace(&self.agents_dir, agent_id);
        let (api_key, _, _) = match read_hasn_creds(&workspace) {
            Some(c) => c,
            None => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("未找到 HASN 凭证。".to_string()),
                })
            }
        };

        let url = format!(
            "{}/api/v1/hasn/contacts?relation_type={rt}",
            self.hasn_base_url
        );
        match reqwest::Client::new()
            .get(&url)
            .header("Authorization", format!("ApiKey {api_key}"))
            .send()
            .await
        {
            Ok(r) => {
                let body: serde_json::Value = r.json().await.unwrap_or(json!({}));
                Ok(ToolResult {
                    success: true,
                    output: json!({ "contacts": body["data"] }).to_string(),
                    error: None,
                })
            }
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("查询联系人失败: {e}")),
            }),
        }
    }
}

// ── hasn_add_friend ──────────────────────────────────

pub struct HasnAddFriend {
    agents_dir: PathBuf,
    hasn_base_url: String,
}

impl HasnAddFriend {
    pub fn new(agents_dir: PathBuf, hasn_base_url: String) -> Self {
        Self {
            agents_dir,
            hasn_base_url,
        }
    }
}

#[async_trait]
impl Tool for HasnAddFriend {
    fn name(&self) -> &str {
        "hasn_add_friend"
    }
    fn description(&self) -> &str {
        "通过HASN发送好友请求。对方接受后才能互相发消息。"
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "agent_id": { "type": "string", "description": "Agent ID" },
                "star_id": { "type": "string", "description": "对方唤星号" },
                "message": { "type": "string", "description": "好友请求附言" }
            },
            "required": ["agent_id", "star_id"]
        })
    }
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let agent_id = args["agent_id"].as_str().unwrap_or_default();
        let star_id = args["star_id"].as_str().unwrap_or_default();
        let message = args["message"].as_str().unwrap_or("");

        let workspace = resolve_workspace(&self.agents_dir, agent_id);
        let (api_key, _, _) = match read_hasn_creds(&workspace) {
            Some(c) => c,
            None => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("未找到 HASN 凭证。".to_string()),
                })
            }
        };

        let url = format!("{}/api/v1/hasn/contacts/request", self.hasn_base_url);
        match reqwest::Client::new()
            .post(&url)
            .header("Authorization", format!("ApiKey {api_key}"))
            .header("Content-Type", "application/json")
            .json(&json!({ "target_star_id": star_id, "message": message }))
            .send()
            .await
        {
            Ok(r) => {
                let body: serde_json::Value = r.json().await.unwrap_or(json!({}));
                Ok(ToolResult {
                    success: true,
                    output: json!({
                        "sent": true,
                        "target": star_id,
                        "detail": body["data"],
                    })
                    .to_string(),
                    error: None,
                })
            }
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("添加好友失败: {e}")),
            }),
        }
    }
}

// ── hasn_inbox ───────────────────────────────────────

pub struct HasnInbox {
    agents_dir: PathBuf,
    hasn_base_url: String,
}

impl HasnInbox {
    pub fn new(agents_dir: PathBuf, hasn_base_url: String) -> Self {
        Self {
            agents_dir,
            hasn_base_url,
        }
    }
}

#[async_trait]
impl Tool for HasnInbox {
    fn name(&self) -> &str {
        "hasn_inbox"
    }
    fn description(&self) -> &str {
        "查看收到的待处理HASN好友请求。"
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "agent_id": { "type": "string", "description": "Agent ID" }
            },
            "required": ["agent_id"]
        })
    }
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let agent_id = args["agent_id"].as_str().unwrap_or_default();

        let workspace = resolve_workspace(&self.agents_dir, agent_id);
        let (api_key, _, _) = match read_hasn_creds(&workspace) {
            Some(c) => c,
            None => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("未找到 HASN 凭证。".to_string()),
                })
            }
        };

        let url = format!("{}/api/v1/hasn/contacts/requests", self.hasn_base_url);
        match reqwest::Client::new()
            .get(&url)
            .header("Authorization", format!("ApiKey {api_key}"))
            .send()
            .await
        {
            Ok(r) => {
                let body: serde_json::Value = r.json().await.unwrap_or(json!({}));
                Ok(ToolResult {
                    success: true,
                    output: json!({ "requests": body["data"] }).to_string(),
                    error: None,
                })
            }
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("查询好友请求失败: {e}")),
            }),
        }
    }
}

// ── hasn_respond_request ─────────────────────────────

pub struct HasnRespondRequest {
    agents_dir: PathBuf,
    hasn_base_url: String,
}

impl HasnRespondRequest {
    pub fn new(agents_dir: PathBuf, hasn_base_url: String) -> Self {
        Self {
            agents_dir,
            hasn_base_url,
        }
    }
}

#[async_trait]
impl Tool for HasnRespondRequest {
    fn name(&self) -> &str {
        "hasn_respond_request"
    }
    fn description(&self) -> &str {
        "接受或拒绝HASN好友请求。"
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "agent_id": { "type": "string", "description": "Agent ID" },
                "request_id": { "type": "number", "description": "好友请求ID（从 hasn_inbox 获取）" },
                "action": { "type": "string", "description": "操作: accept / reject" }
            },
            "required": ["agent_id", "request_id", "action"]
        })
    }
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let agent_id = args["agent_id"].as_str().unwrap_or_default();
        let request_id = args["request_id"].as_i64().unwrap_or(0);
        let action = args["action"].as_str().unwrap_or("reject");

        if !["accept", "reject"].contains(&action) {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("action 必须是 accept 或 reject".to_string()),
            });
        }

        let workspace = resolve_workspace(&self.agents_dir, agent_id);
        let (api_key, _, _) = match read_hasn_creds(&workspace) {
            Some(c) => c,
            None => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("未找到 HASN 凭证。".to_string()),
                })
            }
        };

        let url = format!(
            "{}/api/v1/hasn/contacts/requests/{request_id}/respond",
            self.hasn_base_url
        );
        match reqwest::Client::new()
            .put(&url)
            .header("Authorization", format!("ApiKey {api_key}"))
            .header("Content-Type", "application/json")
            .json(&json!({ "action": action }))
            .send()
            .await
        {
            Ok(r) => {
                let body: serde_json::Value = r.json().await.unwrap_or(json!({}));
                let msg = if action == "accept" {
                    "✅ 已接受好友请求"
                } else {
                    "❌ 已拒绝好友请求"
                };
                Ok(ToolResult {
                    success: true,
                    output: json!({ "message": msg, "detail": body["data"] }).to_string(),
                    error: None,
                })
            }
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("操作失败: {e}")),
            }),
        }
    }
}
