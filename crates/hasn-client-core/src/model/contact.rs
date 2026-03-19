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
}

fn default_relation() -> String { "social".to_string() }
fn default_trust() -> i32 { 1 }
fn default_contact_status() -> String { "pending".to_string() }

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
