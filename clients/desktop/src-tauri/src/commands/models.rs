use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HasnClientInfo {
    pub hasn_id: String,
    pub star_id: String,
    pub client_id: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub peer_id: String,
    pub peer_name: String,
    pub peer_type: String,
    pub last_message: Option<String>,
    pub last_message_at: Option<String>,
    pub unread_count: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    pub id: i64,
    pub local_id: String,
    pub conversation_id: String,
    pub from_id: String,
    pub from_type: i32,
    pub content: String,
    pub content_type: i32,
    pub status: i32,
    pub send_status: String,
    pub created_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Contact {
    pub hasn_id: String,
    pub star_id: String,
    pub name: String,
    pub peer_type: String,
    pub relation_type: String,
    pub trust_level: i32,
    pub status: String,
}
