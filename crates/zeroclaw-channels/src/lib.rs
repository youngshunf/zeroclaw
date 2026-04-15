//! Channel implementations and orchestration for messaging platform integrations.

pub mod orchestrator;
pub mod util;

// 唤星扩展需要访问 orchestrator 内部符号（建 per-tenant 会话历史、调用
// build_system_prompt_with_mode 等），在顶层 re-export 方便 zeroclaw-huanxing。
pub use orchestrator::{
    ConversationHistoryMap, MAX_CONVERSATION_SENDERS, build_system_prompt_with_mode,
};

// Always-compiled channels and utilities (no feature gate)
pub mod cli;
pub mod link_enricher;
pub mod transcription;
pub mod tts;

// Feature-gated channels
#[cfg(feature = "channel-bluesky")]
pub mod bluesky;
#[cfg(feature = "channel-clawdtalk")]
pub mod clawdtalk;
#[cfg(feature = "channel-dingtalk")]
pub mod dingtalk;
#[cfg(feature = "channel-discord")]
pub mod discord;
#[cfg(feature = "channel-discord")]
pub mod discord_history;
#[cfg(feature = "channel-email")]
pub mod email_channel;
#[cfg(feature = "channel-email")]
pub mod gmail_push;
#[cfg(feature = "channel-imessage")]
pub mod imessage;
#[cfg(feature = "channel-irc")]
pub mod irc;
#[cfg(feature = "channel-lark")]
pub mod lark;
#[cfg(feature = "channel-line")]
pub mod line;
#[cfg(feature = "channel-linq")]
pub mod linq;
#[cfg(feature = "channel-matrix")]
pub mod matrix;
#[cfg(feature = "channel-mattermost")]
pub mod mattermost;
#[cfg(feature = "channel-mochat")]
pub mod mochat;
#[cfg(feature = "channel-nextcloud")]
pub mod nextcloud_talk;
#[cfg(feature = "channel-nostr")]
pub mod nostr;
#[cfg(feature = "channel-notion")]
pub mod notion;
#[cfg(feature = "channel-qq")]
pub mod qq;
#[cfg(feature = "channel-reddit")]
pub mod reddit;
#[cfg(feature = "channel-signal")]
pub mod signal;
#[cfg(feature = "channel-slack")]
pub mod slack;
#[cfg(feature = "channel-telegram")]
pub mod telegram;
#[cfg(feature = "channel-twitter")]
pub mod twitter;
#[cfg(feature = "channel-voice-call")]
pub mod voice_call;
#[cfg(feature = "voice-wake")]
pub mod voice_wake;
#[cfg(feature = "channel-wati")]
pub mod wati;
#[cfg(feature = "channel-webhook")]
pub mod webhook;
#[cfg(feature = "channel-wecom")]
pub mod wecom;
#[cfg(feature = "channel-whatsapp-cloud")]
pub mod whatsapp;
#[cfg(feature = "whatsapp-web")]
pub mod whatsapp_storage;
#[cfg(feature = "whatsapp-web")]
pub mod whatsapp_web;

// ─────────────────────────────────────────────────────────────
// 唤星扩展钩子（channels 层）
//
// 唤星在 napcat / wechat_pad / weixin 三个渠道上做 fork 扩展，这些渠道
// 构造和启动原本写在 src/channels/mod.rs 的 start_channels 里（带
// `#[cfg(feature = "huanxing")]`）。RFC D1 workspace 拆分后 start_channels
// 搬到 zeroclaw-channels::orchestrator，此处通过 OnceLock 钩子由根 crate
// 在启动前注册 huanxing 工厂，orchestrator 构建完核心渠道后调用本钩子
// 追加唤星渠道。
//
// 两个钩子：
// - `HuanxingChannelsFn`：构造阶段返回 (display_name, Arc<dyn Channel>) 列表
// - `HuanxingChannelsRegisteredFn`：所有渠道启动后回调，供 huanxing 的
//   channel_registry::register_live_channels + register_inbound_queue 使用
// ─────────────────────────────────────────────────────────────
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

/// 唤星渠道构造钩子。由 main.rs 在 daemon 启动前注册。
///
/// 入参：
/// - `config`: 全局 Config 引用，用于读取 `[channels.napcat/wechat_pad/weixin]`
/// - `inbound_tx`: 渠道入站消息总线，huanxing 可通过它注册自己的 inbound queue
///
/// 返回：extra channels 列表，格式 `(display_name, channel)`。
/// `display_name` 仅用于启动日志，`channel.name()` 是真正的路由键。
pub type HuanxingChannelsFn = Box<
    dyn Fn(
            &zeroclaw_config::schema::Config,
            &tokio::sync::mpsc::Sender<zeroclaw_api::channel::ChannelMessage>,
        ) -> Vec<(&'static str, Arc<dyn zeroclaw_api::channel::Channel>)>
        + Send
        + Sync,
>;

static HUANXING_CHANNELS_FN: OnceLock<HuanxingChannelsFn> = OnceLock::new();

/// 渠道构造完成后的注册回调。供 huanxing channel_registry 使用。
pub type HuanxingChannelsRegisteredFn = Box<
    dyn Fn(&HashMap<String, Arc<dyn zeroclaw_api::channel::Channel>>) + Send + Sync,
>;

static HUANXING_CHANNELS_REGISTERED_FN: OnceLock<HuanxingChannelsRegisteredFn> = OnceLock::new();

/// 注册唤星渠道构造函数（main.rs 在 daemon 启动前调用一次）。
pub fn register_huanxing_channels_fn(f: HuanxingChannelsFn) {
    let _ = HUANXING_CHANNELS_FN.set(f);
}

/// 注册渠道构造完成后的回调（main.rs 在 daemon 启动前调用一次）。
pub fn register_huanxing_channels_registered_fn(f: HuanxingChannelsRegisteredFn) {
    let _ = HUANXING_CHANNELS_REGISTERED_FN.set(f);
}

/// 内部使用：收集唤星额外渠道。
pub(crate) fn build_huanxing_channels(
    config: &zeroclaw_config::schema::Config,
    inbound_tx: &tokio::sync::mpsc::Sender<zeroclaw_api::channel::ChannelMessage>,
) -> Vec<(&'static str, Arc<dyn zeroclaw_api::channel::Channel>)> {
    if let Some(f) = HUANXING_CHANNELS_FN.get() {
        f(config, inbound_tx)
    } else {
        Vec::new()
    }
}

/// 内部使用：通知唤星侧所有渠道已上线。
pub(crate) fn notify_huanxing_channels_registered(
    channels_by_name: &HashMap<String, Arc<dyn zeroclaw_api::channel::Channel>>,
) {
    if let Some(f) = HUANXING_CHANNELS_REGISTERED_FN.get() {
        f(channels_by_name);
    }
}

/// Lark audio 消息解析钩子。由 `huanxing` 注册唤星版 `parse_lark_audio_content`，
/// 提取音频消息里的 `(text, mentioned_open_ids)`。未注册时 lark.rs 的 audio
/// 分支返回 None，走默认 transcription_manager 流程。
pub type LarkAudioParserFn = Box<
    dyn Fn(&str) -> Option<(String, Vec<String>)> + Send + Sync,
>;

static LARK_AUDIO_PARSER_FN: OnceLock<LarkAudioParserFn> = OnceLock::new();

pub fn register_lark_audio_parser_fn(f: LarkAudioParserFn) {
    let _ = LARK_AUDIO_PARSER_FN.set(f);
}

pub(crate) fn parse_lark_audio_with_hook(content_str: &str) -> Option<(String, Vec<String>)> {
    LARK_AUDIO_PARSER_FN.get().and_then(|f| f(content_str))
}

/// 【唤星】DashScope/Generic TTS provider 构造钩子。
///
/// 返回 `(provider_name, Box<dyn TtsProvider>)`。由 `huanxing` crate 实现，
/// `TtsManager::new` 初始化时调用。
pub type TtsProviderBuilderFn = Box<
    dyn Fn(&zeroclaw_config::schema::TtsConfig) -> Option<(String, Box<dyn tts::TtsProvider>)>
        + Send
        + Sync,
>;

static TTS_DASHSCOPE_BUILDER: OnceLock<TtsProviderBuilderFn> = OnceLock::new();
static TTS_GENERIC_OPENAI_BUILDER: OnceLock<TtsProviderBuilderFn> = OnceLock::new();

pub fn register_tts_dashscope_builder(f: TtsProviderBuilderFn) {
    let _ = TTS_DASHSCOPE_BUILDER.set(f);
}

pub fn register_tts_generic_openai_builder(f: TtsProviderBuilderFn) {
    let _ = TTS_GENERIC_OPENAI_BUILDER.set(f);
}

pub(crate) fn build_huanxing_tts_dashscope(
    config: &zeroclaw_config::schema::TtsConfig,
) -> Option<(String, Box<dyn tts::TtsProvider>)> {
    TTS_DASHSCOPE_BUILDER.get().and_then(|f| f(config))
}

pub(crate) fn build_huanxing_tts_generic_openai(
    config: &zeroclaw_config::schema::TtsConfig,
) -> Option<(String, Box<dyn tts::TtsProvider>)> {
    TTS_GENERIC_OPENAI_BUILDER.get().and_then(|f| f(config))
}
