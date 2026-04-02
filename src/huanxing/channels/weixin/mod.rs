use crate::channels::traits::{Channel, SendMessage};
use async_trait::async_trait;
use base64::Engine;

use std::sync::Arc;

pub mod api;
pub mod auth;
pub mod cdn;
pub mod types;

#[derive(Clone)]
pub struct WeixinChannel {
    pub bot_token: String,
    pub bot_id: String,
    pub base_url: String,
}

impl WeixinChannel {
    pub fn new(bot_token: String, bot_id: String, base_url: String) -> Self {
        Self {
            bot_token,
            bot_id,
            base_url,
        }
    }
}

#[async_trait]
impl Channel for WeixinChannel {
    fn name(&self) -> &str {
        "weixin"
    }

    async fn send(&self, message: &SendMessage) -> anyhow::Result<()> {
        if message.attachments.is_empty() {
            return api::send_message(self, message).await;
        }

        if !message.content.is_empty() {
            api::send_message(self, message).await?;
        }

        for attachment in &message.attachments {
            let media_type = match attachment.mime_type.as_deref().unwrap_or_default() {
                m if m.starts_with("image/") => types::UPLOAD_MEDIA_TYPE_IMAGE,
                m if m.starts_with("video/") => types::UPLOAD_MEDIA_TYPE_VIDEO,
                _ => types::UPLOAD_MEDIA_TYPE_FILE,
            };

            let upload =
                cdn::upload_media_to_cdn(self, &attachment.data, &message.recipient, media_type)
                    .await?;

            let cdn_media = types::CdnMedia {
                encrypt_query_param: Some(upload.download_encrypted_query_param),
                aes_key: Some(
                    base64::engine::general_purpose::STANDARD
                        .encode(hex::decode(&upload.aeskey_hex).unwrap_or_default()),
                ),
                encrypt_type: Some(1),
                full_url: None,
            };

            let media_item = if media_type == types::UPLOAD_MEDIA_TYPE_IMAGE {
                types::MessageItem {
                    item_type: Some(types::MESSAGE_ITEM_TYPE_IMAGE),
                    image_item: Some(types::ImageItem {
                        media: Some(cdn_media),
                        mid_size: Some(upload.file_size_ciphertext),
                        ..Default::default()
                    }),
                    ..Default::default()
                }
            } else if media_type == types::UPLOAD_MEDIA_TYPE_VIDEO {
                types::MessageItem {
                    item_type: Some(types::MESSAGE_ITEM_TYPE_VIDEO),
                    video_item: Some(types::VideoItem {
                        media: Some(cdn_media),
                        video_size: Some(upload.file_size_ciphertext),
                        ..Default::default()
                    }),
                    ..Default::default()
                }
            } else {
                types::MessageItem {
                    item_type: Some(types::MESSAGE_ITEM_TYPE_FILE),
                    file_item: Some(types::FileItem {
                        media: Some(cdn_media),
                        file_name: Some(attachment.file_name.clone()),
                        len: Some(upload.file_size.to_string()),
                        ..Default::default()
                    }),
                    ..Default::default()
                }
            };

            api::send_media_message(self, &message.recipient, media_item).await?;
        }

        Ok(())
    }

    async fn listen(
        &self,
        tx: tokio::sync::mpsc::Sender<crate::channels::traits::ChannelMessage>,
    ) -> anyhow::Result<()> {
        let channel = Arc::new(self.clone());
        tokio::spawn(async move {
            api::get_updates_loop(channel, tx).await;
        });
        Ok(())
    }

    async fn health_check(&self) -> bool {
        true
    }
}
