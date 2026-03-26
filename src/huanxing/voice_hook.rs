use std::sync::Arc;
use async_trait::async_trait;

use crate::channels::context_resolver::MessageContextResolver;
use crate::config::Config;
use crate::hooks::{HookHandler, HookResult};
use crate::huanxing::voice::{auto_synthesize_voice_markers, HxVoiceConfig};

/// Hook that intercepts outgoing messages containing `[VOICE:...]` markers,
/// synthesizes the audio via TTS, and downloads the file into the tenant's workspace.
pub struct VoiceSynthesisHook {
    resolver: Arc<dyn MessageContextResolver>,
    voice_config: Option<HxVoiceConfig>,
}

impl VoiceSynthesisHook {
    pub fn new(resolver: Arc<dyn MessageContextResolver>, config: &Config) -> Self {
        let voice_config = HxVoiceConfig::from_config(config);
        Self {
            resolver,
            voice_config,
        }
    }
}

#[async_trait]
impl HookHandler for VoiceSynthesisHook {
    fn name(&self) -> &str {
        "huanxing_voice_synthesis"
    }

    async fn on_message_sending(
        &self,
        channel: String,
        recipient: String,
        content: String,
    ) -> HookResult<(String, String, String)> {
        // bail out quickly if no voice config is enabled or no target marker is found.
        if self.voice_config.is_none() || !content.contains("[VOICE:") {
            return HookResult::Continue((channel, recipient, content));
        }

        let voice_cfg = self.voice_config.as_ref().unwrap();

        // resolve context so we can get the tenant's workspace
        let msg_ctx = self.resolver.resolve(&channel, &recipient).await;

        // perform the synthesis using existing business logic
        let new_content = auto_synthesize_voice_markers(&content, voice_cfg, &msg_ctx.workspace_dir).await;

        HookResult::Continue((channel, recipient, new_content))
    }
}
