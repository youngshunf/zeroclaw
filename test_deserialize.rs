use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsMessagePayload {
    pub id: serde_json::Value,
    pub conversation_id: String,
    pub from_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_star_id: Option<String>,
    pub from_type: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_type: Option<i32>,
    pub content: serde_json::Value,
    pub content_type: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub self_sent: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_owner_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_owner_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageReceivedParams {
    pub to_id: String,
    pub message: WsMessagePayload,
}

fn main() {
    let json_str = r#"{
        "to_id": "a_12345",
        "message": {
            "id": 1,
            "conversation_id": "123",
            "from_id": "h_123",
            "from_type": 1,
            "to_id": "a_123",
            "to_type": 1,
            "content_type": 1,
            "content": {"text": "hello"},
            "msg_type": "text",
            "status": 1,
            "priority": "normal",
            "reply_to_id": null,
            "local_id": "local123",
            "created_time": "2023-01-01T00:00:00",
            "from_owner_id": "h_123",
            "to_owner_id": "h_123"
        }
    }"#;
    match serde_json::from_str::<MessageReceivedParams>(json_str) {
        Ok(v) => println!("Success: {:?}", v),
        Err(e) => println!("Error: {}", e),
    }
}
