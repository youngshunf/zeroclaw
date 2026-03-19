//! HASN IM 命令 — 消息/会话/联系人

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub peer_name: String,
    pub last_message: Option<String>,
    pub unread_count: u32,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub sender_id: String,
    pub content: String,
    pub sent_at: String,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Contact {
    pub hasn_uuid: String,
    pub star_id: String,
    pub nickname: String,
    pub relation_type: String,
    pub is_online: bool,
}

#[tauri::command]
pub async fn get_conversations() -> Result<Vec<Conversation>, String> {
    // TODO: Phase 2 — 调用 hasn-client-core
    Ok(vec![])
}

#[tauri::command]
pub async fn get_messages(conversation_id: String) -> Result<Vec<Message>, String> {
    // TODO: Phase 2
    Ok(vec![])
}

#[tauri::command]
pub async fn send_message(to: String, content: String) -> Result<Message, String> {
    // TODO: Phase 2
    tracing::info!("send_message to={} content_len={}", to, content.len());
    Err("HASN 消息发送待实现 (Phase 2)".into())
}

#[tauri::command]
pub async fn mark_conversation_read(conversation_id: String) -> Result<(), String> {
    // TODO: Phase 2
    Ok(())
}

#[tauri::command]
pub async fn get_contacts() -> Result<Vec<Contact>, String> {
    // TODO: Phase 2
    Ok(vec![])
}

#[tauri::command]
pub async fn send_friend_request(star_id: String, message: Option<String>) -> Result<(), String> {
    // TODO: Phase 2
    tracing::info!("send_friend_request to={}", star_id);
    Err("好友请求待实现 (Phase 2)".into())
}

#[tauri::command]
pub async fn respond_friend_request(request_id: String, accept: bool) -> Result<(), String> {
    // TODO: Phase 2
    tracing::info!("respond_friend_request id={} accept={}", request_id, accept);
    Ok(())
}
