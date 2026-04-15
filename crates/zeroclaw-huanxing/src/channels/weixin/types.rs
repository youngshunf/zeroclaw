use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseInfo {
    pub channel_version: Option<String>,
}

pub const UPLOAD_MEDIA_TYPE_IMAGE: u8 = 1;
pub const UPLOAD_MEDIA_TYPE_VIDEO: u8 = 2;
pub const UPLOAD_MEDIA_TYPE_FILE: u8 = 3;
pub const UPLOAD_MEDIA_TYPE_VOICE: u8 = 4;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetUploadUrlReq {
    pub filekey: Option<String>,
    pub media_type: Option<u8>,
    pub to_user_id: Option<String>,
    pub rawsize: Option<u64>,
    pub rawfilemd5: Option<String>,
    pub filesize: Option<u64>,
    pub thumb_rawsize: Option<u64>,
    pub thumb_rawfilemd5: Option<String>,
    pub thumb_filesize: Option<u64>,
    pub no_need_thumb: Option<bool>,
    pub aeskey: Option<String>,
    pub base_info: Option<BaseInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetUploadUrlResp {
    pub upload_param: Option<String>,
    pub thumb_upload_param: Option<String>,
    pub upload_full_url: Option<String>,
}

pub const MESSAGE_TYPE_NONE: u8 = 0;
pub const MESSAGE_TYPE_USER: u8 = 1;
pub const MESSAGE_TYPE_BOT: u8 = 2;

pub const MESSAGE_ITEM_TYPE_NONE: u8 = 0;
pub const MESSAGE_ITEM_TYPE_TEXT: u8 = 1;
pub const MESSAGE_ITEM_TYPE_IMAGE: u8 = 2;
pub const MESSAGE_ITEM_TYPE_VOICE: u8 = 3;
pub const MESSAGE_ITEM_TYPE_FILE: u8 = 4;
pub const MESSAGE_ITEM_TYPE_VIDEO: u8 = 5;

pub const MESSAGE_STATE_NEW: u8 = 0;
pub const MESSAGE_STATE_GENERATING: u8 = 1;
pub const MESSAGE_STATE_FINISH: u8 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextItem {
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CdnMedia {
    pub encrypt_query_param: Option<String>,
    pub aes_key: Option<String>,
    pub encrypt_type: Option<u8>,
    pub full_url: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImageItem {
    pub media: Option<CdnMedia>,
    pub thumb_media: Option<CdnMedia>,
    pub aeskey: Option<String>,
    pub url: Option<String>,
    pub mid_size: Option<u64>,
    pub thumb_size: Option<u64>,
    pub thumb_height: Option<u32>,
    pub thumb_width: Option<u32>,
    pub hd_size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceItem {
    pub media: Option<CdnMedia>,
    pub encode_type: Option<u8>,
    pub bits_per_sample: Option<u8>,
    pub sample_rate: Option<u32>,
    pub playtime: Option<u32>,
    pub text: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileItem {
    pub media: Option<CdnMedia>,
    pub file_name: Option<String>,
    pub md5: Option<String>,
    pub len: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VideoItem {
    pub media: Option<CdnMedia>,
    pub video_size: Option<u64>,
    pub play_length: Option<u32>,
    pub video_md5: Option<String>,
    pub thumb_media: Option<CdnMedia>,
    pub thumb_size: Option<u64>,
    pub thumb_height: Option<u32>,
    pub thumb_width: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefMessage {
    pub message_item: Option<Box<MessageItem>>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessageItem {
    #[serde(rename = "type")]
    pub item_type: Option<u8>,
    pub create_time_ms: Option<u64>,
    pub update_time_ms: Option<u64>,
    pub is_completed: Option<bool>,
    pub msg_id: Option<String>,
    pub ref_msg: Option<RefMessage>,
    pub text_item: Option<TextItem>,
    pub image_item: Option<ImageItem>,
    pub voice_item: Option<VoiceItem>,
    pub file_item: Option<FileItem>,
    pub video_item: Option<VideoItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeixinMessage {
    pub seq: Option<u64>,
    pub message_id: Option<u64>,
    pub from_user_id: Option<String>,
    pub to_user_id: Option<String>,
    pub client_id: Option<String>,
    pub create_time_ms: Option<u64>,
    pub update_time_ms: Option<u64>,
    pub delete_time_ms: Option<u64>,
    pub session_id: Option<String>,
    pub group_id: Option<String>,
    pub message_type: Option<u8>,
    pub message_state: Option<u8>,
    pub item_list: Option<Vec<MessageItem>>,
    pub context_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetUpdatesReq {
    pub sync_buf: Option<String>,
    pub get_updates_buf: Option<String>,
    pub base_info: Option<BaseInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetUpdatesResp {
    pub ret: Option<i32>,
    pub errcode: Option<i32>,
    pub errmsg: Option<String>,
    pub msgs: Option<Vec<WeixinMessage>>,
    pub sync_buf: Option<String>,
    pub get_updates_buf: Option<String>,
    pub longpolling_timeout_ms: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageReqMsg {
    pub from_user_id: Option<String>,
    pub to_user_id: Option<String>,
    pub client_id: Option<String>,
    pub message_type: Option<u8>,
    pub message_state: Option<u8>,
    pub item_list: Option<Vec<MessageItem>>,
    pub context_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageReq {
    pub msg: Option<SendMessageReqMsg>,
    pub base_info: Option<BaseInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageResp {
    // Empty usually
}
