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

/// 节点认证状态（v5.0 统一节点模型）
///
/// 对齐协议: Node Key (hasn_nk_) 认证，取代旧 Client JWT
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeAuth {
    /// 节点 ID (n_{uuid_short})
    pub node_id: String,

    /// Node Key (hasn_nk_ 前缀)
    pub node_key: String,

    /// 节点类型 (desktop / mobile / web / cloud)
    pub node_type: String,

    /// 设备名称
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_name: Option<String>,
}

/// 兼容别名（过渡期）
#[deprecated(note = "使用 NodeAuth 代替")]
pub type ClientAuth = NodeAuth;

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

/// HASN 注册响应（幂等）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HasnRegisterResponse {
    pub human: HasnHumanOut,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<HasnAgentOut>,
    #[serde(default)]
    pub already_exists: bool,
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
    /// Agent Key (hasn_ak_ 前缀)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_key: Option<String>,
}

/// 节点注册响应（v5.0）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterNodeResponse {
    pub node_id: String,
    pub node_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_name: Option<String>,
}

/// 兼容别名
#[deprecated(note = "使用 RegisterNodeResponse 代替")]
pub type RegisterClientResponse = RegisterNodeResponse;

/// Node Key 签发响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeKeyResponse {
    pub node_key: String,
    pub node_id: String,
}

/// 兼容别名
#[deprecated(note = "使用 NodeKeyResponse 代替")]
pub type ClientTokenResponse = NodeKeyResponse;

/// Agent 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub hasn_id: String,
    pub star_id: String,
    pub name: String,
    pub agent_name: String,
    #[serde(rename = "type")]
    pub agent_type: String,
    #[serde(default)]
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<serde_json::Value>,
    pub online: bool,
    pub created_via: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_time: Option<String>,
}

/// Agent HASN 注册响应（幂等）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterAgentResponse {
    pub hasn_id: String,
    pub star_id: String,
    pub name: String,
    pub agent_name: String,
    /// Agent Key (首次创建时返回明文)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_key: Option<String>,
    #[serde(default)]
    pub already_exists: bool,
}
