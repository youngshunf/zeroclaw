use serde::{Deserialize, Serialize};

/// 认证状态 (登录后持久化到本地)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthState {
    /// 平台 access_token
    pub access_token: String,

    /// HASN JWT
    pub hasn_token: String,

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
    pub jwt_token: String,
    pub refresh_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HasnAgentOut {
    pub hasn_id: String,
    pub star_id: String,
    pub name: String,
    pub api_key: String,
}
