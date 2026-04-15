//! HuanXing channel implementations.
//!
//! Houses channel integrations specific to the HuanXing platform
//! (Napcat/QQ, WeChatPad, iLink Weixin). These are kept out of upstream
//! `zeroclaw-channels` to minimise merge conflicts.

pub mod napcat;
pub mod wechat_pad;
pub mod weixin;

pub use napcat::NapcatChannel;
pub use wechat_pad::WechatPadChannel;
pub use weixin::WeixinChannel;

use std::sync::Arc;
use zeroclaw_api::channel::{Channel, ChannelMessage};
use zeroclaw_config::schema::Config;

/// 构造所有唤星扩展渠道。由 main.rs 注册到 zeroclaw-channels 的渠道钩子，
/// orchestrator::start_channels 在构建默认渠道后调用。
///
/// 返回 `(display_name, Arc<dyn Channel>)` 列表：
/// - `napcat`  —— QQ via OneBot 协议
/// - `wechat_pad` —— WeChatPadPro 微信 iPad 协议
/// - `weixin`  —— iLink AI 微信扫码
///
/// 同时将 `inbound_tx` 注册到 `channel_registry` 的全局 inbound queue，
/// 供 tenant_heartbeat 等子系统投递合成消息。
pub fn build_huanxing_channels(
    config: &Config,
    inbound_tx: &tokio::sync::mpsc::Sender<ChannelMessage>,
) -> Vec<(&'static str, Arc<dyn Channel>)> {
    let mut result: Vec<(&'static str, Arc<dyn Channel>)> = Vec::new();

    // Napcat (QQ via OneBot)
    if let Some(ref napcat_cfg) = config.channels.napcat {
        match NapcatChannel::from_config_with_workspace(
            napcat_cfg.clone(),
            Some(&config.workspace_dir),
            Some(config.transcription.clone()),
        ) {
            Ok(channel) => {
                tracing::info!("[唤星] 已构造 Napcat 渠道");
                result.push(("Napcat (QQ)", Arc::new(channel)));
            }
            Err(e) => {
                tracing::warn!("[唤星] Napcat 渠道构造失败: {e}");
            }
        }
    }

    // WeChatPadPro
    if let Some(ref wechat_pad_cfg) = config.channels.wechat_pad {
        match WechatPadChannel::from_config(wechat_pad_cfg.clone()) {
            Ok(channel) => {
                tracing::info!("[唤星] 已构造 WeChatPad 渠道");
                result.push(("WeChatPad", Arc::new(channel)));
            }
            Err(e) => {
                tracing::warn!("[唤星] WeChatPad 渠道构造失败: {e}");
            }
        }
    }

    // iLink AI Weixin
    if let Some(ref weixin_cfg) = config.channels.weixin {
        if !weixin_cfg.bot_token.is_empty() {
            let channel = WeixinChannel::new(
                weixin_cfg.bot_token.clone(),
                weixin_cfg.bot_id.clone(),
                weixin_cfg.base_url.clone(),
            );
            tracing::info!("[唤星] 已构造 iLink 微信渠道");
            result.push(("iLink 微信", Arc::new(channel)));
        }
    }

    // 注册全局入站消息队列，供 tenant_heartbeat / 合成消息投递等子系统使用
    crate::channel_registry::register_inbound_queue(inbound_tx.clone());

    result
}

/// 所有渠道（含唤星扩展）启动完成后的回调：把 channels_by_name 注册到
/// 唤星全局 live_channels_registry，供 tenant_heartbeat 路由使用。
pub fn on_channels_registered(
    channels_by_name: &std::collections::HashMap<String, Arc<dyn Channel>>,
) {
    crate::channel_registry::register_live_channels(channels_by_name);
}
