// Legacy protocol module — replaced by model/
// Kept for backward compatibility during transition

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HasnMessageLegacy {
    pub message_id: Option<String>,
    pub sender_uuid: String,
    pub receiver_uuid: String,
    pub content: String,
    pub timestamp: Option<i64>,
}
