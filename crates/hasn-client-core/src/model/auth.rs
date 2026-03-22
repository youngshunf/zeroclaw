use serde::{Deserialize, Serialize};

/// 认证状态 (登录后持久化到本地)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthState {
    /// 平台 access_token
    pub access_token: String,

    /// HASN ID (h_xxx)
    pub hasn_id: String,

    /// 唤星号
    pub star_id: String,

    /// 显示名
    pub display_name: String,

    /// 头像URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,

    /// 过期时间戳 (秒)
    pub expires_at: i64,
}

impl AuthState {
    /// 是否已过期
    pub fn is_expired(&self) -> bool {
        let now = chrono::Utc::now().timestamp();
        now >= self.expires_at
    }

    /// 是否快过期 (距过期不到1小时)
    pub fn is_near_expiry(&self) -> bool {
        let now = chrono::Utc::now().timestamp();
        self.expires_at - now < 3600
    }
}

/// 客户端认证状态 (对齐 29/30 文档)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientAuth {
    /// 客户端 ID (c_{uuid_short})
    pub client_id: String,

    /// Client JWT (用于 WebSocket 连接)
    pub client_jwt: String,

    /// 客户端类型
    pub client_type: String,

    /// 设备名称
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_name: Option<String>,
}

/// 登录响应 (平台 /auth/phone-login)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub user: LoginUser,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginUser {
    pub id: i64,
    pub nickname: String,
    pub avatar_url: Option<String>,
}

/// HASN 注册响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HasnRegisterResponse {
    pub human: HasnHumanOut,
    pub agent: HasnAgentOut,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HasnHumanOut {
    pub hasn_id: String,
    pub star_id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HasnAgentOut {
    pub hasn_id: String,
    pub star_id: String,
    pub name: String,
    pub api_key: String,
}

/// 客户端注册响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterClientResponse {
    pub client_id: String,
    pub client_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_name: Option<String>,
}

/// Client JWT 签发响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientTokenResponse {
    pub client_jwt: String,
    pub client_id: String,
}

/// Agent 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub hasn_id: String,
    pub star_id: String,
    pub name: String,
    pub agent_name: String,
    #[serde(rename = "type")]
    pub agent_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_id: Option<String>,
    pub online: bool,
    pub created_via: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_time: Option<String>,
}
