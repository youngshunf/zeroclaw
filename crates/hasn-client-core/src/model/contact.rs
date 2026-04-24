use serde::{Deserialize, Serialize};

/// HASN 联系人 (对齐后端 hasn_contacts 表)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HasnContact {
    pub id: i64,
    pub peer_hasn_id: String,
    pub peer_star_id: String,
    pub peer_name: String,
    pub peer_type: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer_avatar_url: Option<String>,

    /// 关系类型: social / commerce / service / professional
    #[serde(default = "default_relation")]
    pub relation_type: String,

    /// 信任等级: 0=blocked 1=stranger 2=normal 3=trusted 4=owner
    #[serde(default = "default_trust")]
    pub trust_level: i32,

    /// 备注名
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nickname: Option<String>,

    /// 标签
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,

    /// 状态: pending / connected / blocked / archived
    #[serde(default = "default_contact_status")]
    pub status: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub connected_at: Option<String>,

    /// Agent 归属的 owner hasn_id（human 联系人为 None）。US-004：对齐服务端。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peer_owner_id: Option<String>,

    /// 自定义权限覆盖。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_permissions: Option<serde_json::Value>,

    /// 当前作用域。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<serde_json::Value>,

    /// 订阅标记。
    #[serde(default)]
    pub subscription: bool,

    /// 好友请求附言。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_message: Option<String>,

    /// 关系自动过期时间（RFC3339）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_expire: Option<String>,

    /// 最近互动时间（RFC3339）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_interaction_at: Option<String>,

    /// 互动次数累计。
    #[serde(default)]
    pub interaction_count: i32,
}

fn default_relation() -> String {
    "social".to_string()
}
fn default_trust() -> i32 {
    1
}
fn default_contact_status() -> String {
    "pending".to_string()
}

/// 好友请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendRequest {
    pub id: i64,
    pub from_hasn_id: String,
    pub from_star_id: String,
    pub from_name: String,
    pub message: Option<String>,
    pub status: String,
    pub created_at: Option<String>,
}
