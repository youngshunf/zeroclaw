//! HuanXing configuration types.
//!
//! Schema definitions live in `zeroclaw-config::huanxing` (upstream config
//! crate is the dependency leaf both zeroclaw-huanxing and zeroclaw-gateway
//! depend on). This module re-exports them so existing
//! `crate::config::HuanXingConfig` paths inside zeroclaw-huanxing continue
//! to resolve.
//!
//! Additionally re-exports `zeroclaw_config::config::*` and
//! `zeroclaw_config::security::*` so proc-macro-generated code from
//! `zeroclaw_macros::Configurable` derives (which reference `crate::config::*`
//! and `crate::security::*`) resolves within this crate.

pub use zeroclaw_config::config::*;
pub use zeroclaw_config::huanxing::*;
