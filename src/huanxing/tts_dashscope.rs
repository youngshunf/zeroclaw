//! 百炼 DashScope TTS Provider
//!
//! Uses DashScope Omni models (qwen-omni-turbo, qwen2.5-omni-7b) via streaming
//! SSE to synthesize speech. Audio data is returned as base64 chunks in
//! `choices[0].delta.audio.data`.
//!
//! Supported models:
//! - `qwen-omni-turbo` / `qwen-omni-turbo-latest` (recommended, faster)
//! - `qwen2.5-omni-7b` (higher quality)
//!
//! Supported voices: `Chelsie`, `Ethan`
//! Supported audio formats: `wav`, `pcm`, `mp3`
//!
//! This is a huanxing-specific provider — kept in `src/huanxing/` per dev conventions.

use anyhow::{bail, Context, Result};
use base64::Engine;

/// DashScope TTS provider using Omni models with streaming audio output.
pub struct DashScopeTtsProvider {
    api_key: String,
    model: String,
    base_url: String,
    default_voice: String,
    audio_format: String,
    client: reqwest::Client,
}

/// Configuration for DashScope TTS provider.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct DashScopeTtsConfig {
    /// API key for DashScope. Falls back to `DASHSCOPE_API_KEY` env var.
    #[serde(default)]
    pub api_key: Option<String>,

    /// Model name. Supported: `qwen-omni-turbo`, `qwen-omni-turbo-latest`,
    /// `qwen2.5-omni-7b`. Default: `"qwen-omni-turbo"`.
    #[serde(default = "default_dashscope_tts_model")]
    pub model: String,

    /// Default voice. Options: `Chelsie` (female), `Ethan` (male).
    /// Default: `"Chelsie"`.
    #[serde(default = "default_dashscope_tts_voice")]
    pub default_voice: String,

    /// Audio output format. Options: `wav`, `pcm`, `mp3`.
    /// Default: `"mp3"`.
    #[serde(default = "default_dashscope_tts_format")]
    pub audio_format: String,

    /// Base URL for DashScope API.
    /// Default: `"https://dashscope.aliyuncs.com/compatible-mode"`.
    #[serde(default = "default_dashscope_tts_base_url")]
    pub base_url: String,
}

impl Default for DashScopeTtsConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            model: default_dashscope_tts_model(),
            default_voice: default_dashscope_tts_voice(),
            audio_format: default_dashscope_tts_format(),
            base_url: default_dashscope_tts_base_url(),
        }
    }
}

fn default_dashscope_tts_model() -> String {
    "qwen-omni-turbo".into()
}

fn default_dashscope_tts_voice() -> String {
    "Chelsie".into()
}

fn default_dashscope_tts_format() -> String {
    "mp3".into()
}

fn default_dashscope_tts_base_url() -> String {
    "https://dashscope.aliyuncs.com/compatible-mode".into()
}

impl DashScopeTtsProvider {
    /// Create a new DashScope TTS provider.
    pub fn new(config: &DashScopeTtsConfig) -> Result<Self> {
        let api_key = config
            .api_key
            .as_deref()
            .map(str::trim)
            .filter(|k| !k.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| {
                std::env::var("DASHSCOPE_API_KEY")
                    .ok()
                    .map(|v| v.trim().to_string())
                    .filter(|v| !v.is_empty())
            })
            .context(
                "Missing DashScope TTS API key: set [tts.dashscope].api_key or DASHSCOPE_API_KEY",
            )?;

        let base_url = config.base_url.trim_end_matches('/').to_string();

        Ok(Self {
            api_key,
            model: config.model.clone(),
            base_url,
            default_voice: config.default_voice.clone(),
            audio_format: config.audio_format.clone(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .context("Failed to build HTTP client for DashScope TTS")?,
        })
    }

    /// Synthesize text to audio bytes via streaming SSE.
    ///
    /// Omni models require `stream: true` with `modalities: ["text", "audio"]`
    /// to produce audio output. Audio is returned as base64-encoded chunks in
    /// `choices[0].delta.audio.data`.
    async fn synthesize_stream(&self, text: &str, voice: &str) -> Result<Vec<u8>> {
        let url = format!("{}/v1/chat/completions", self.base_url);

        let voice = if voice.is_empty() {
            &self.default_voice
        } else {
            voice
        };

        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {
                    "role": "system",
                    "content": "你是一个语音朗读助手。请将用户提供的文字内容原样朗读出来，不要添加、删除或修改任何内容。直接朗读文字即可。"
                },
                {
                    "role": "user",
                    "content": format!("请朗读以下内容：\n\n{text}")
                }
            ],
            "modalities": ["text", "audio"],
            "audio": {
                "voice": voice,
                "format": self.audio_format
            },
            "stream": true,
            "stream_options": {"include_usage": true}
        });

        tracing::info!(
            "DashScope TTS: synthesizing with model={}, voice={}, format={}",
            self.model, voice, self.audio_format
        );

        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Failed to send DashScope TTS request")?;

        let status = resp.status();
        if !status.is_success() {
            let error_body = resp.text().await.unwrap_or_default();
            bail!("DashScope TTS API error ({}): {}", status, error_body);
        }

        // Parse SSE stream and collect audio chunks
        let mut audio_chunks: Vec<String> = Vec::new();
        let full_body = resp.bytes().await.context("Failed to read SSE stream")?;
        let body_str = String::from_utf8_lossy(&full_body);

        for line in body_str.lines() {
            if !line.starts_with("data: ") {
                continue;
            }
            let payload = &line[6..];
            if payload == "[DONE]" {
                break;
            }

            if let Ok(chunk) = serde_json::from_str::<serde_json::Value>(payload) {
                // Extract audio data from delta.audio.data
                if let Some(data) = chunk
                    .pointer("/choices/0/delta/audio/data")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                {
                    audio_chunks.push(data.to_string());
                }
            }
        }

        if audio_chunks.is_empty() {
            bail!(
                "DashScope TTS ({}) returned no audio data. \
                 Ensure the model supports audio output with modalities=[\"text\",\"audio\"].",
                self.model
            );
        }

        // Concatenate all base64 chunks and decode
        let full_b64 = audio_chunks.join("");
        let audio_bytes = base64::engine::general_purpose::STANDARD
            .decode(&full_b64)
            .context("Failed to decode DashScope audio base64 data")?;

        tracing::info!(
            "DashScope TTS: synthesized {} chunks → {} bytes (model={}, voice={}, format={})",
            audio_chunks.len(),
            audio_bytes.len(),
            self.model,
            voice,
            self.audio_format
        );

        Ok(audio_bytes)
    }
}

#[async_trait::async_trait]
impl crate::channels::tts::TtsProvider for DashScopeTtsProvider {
    fn name(&self) -> &str {
        "dashscope"
    }

    async fn synthesize(&self, text: &str, voice: &str) -> Result<Vec<u8>> {
        self.synthesize_stream(text, voice).await
    }

    fn supported_voices(&self) -> Vec<String> {
        vec!["Chelsie".to_string(), "Ethan".to_string()]
    }

    fn supported_formats(&self) -> Vec<String> {
        vec![
            "wav".to_string(),
            "pcm".to_string(),
            "mp3".to_string(),
        ]
    }
}
