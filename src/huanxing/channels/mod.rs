//! HuanXing channel implementations.
//!
//! Houses channel integrations specific to the HuanXing platform
//! (Napcat/QQ, WeChatPad). These are kept out of `src/channels/`
//! to minimise merge conflicts with upstream ZeroClaw.

pub mod napcat;
pub mod wechat_pad;

pub use napcat::NapcatChannel;
pub use wechat_pad::WechatPadChannel;
