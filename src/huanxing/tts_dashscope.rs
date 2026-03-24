//! 百炼 DashScope TTS Provider (qwen3-tts)
//!
//! Uses DashScope's multimodal-generation API to synthesize speech with
//! qwen3-tts models. Non-streaming mode returns an audio file URL which is
//! then downloaded; streaming mode returns base64 PCM chunks via SSE.
//!
//! Supported models:
//! - `qwen3-tts-instruct-flash` (instruction-controlled, recommended)
//! - `qwen3-tts-flash` (standard, faster)
//!
//! Audio output: WAV (non-streaming) / PCM (streaming), 24kHz sample rate.
//!
//! This is a huanxing-specific provider — kept in `src/huanxing/` per dev conventions.

use anyhow::{bail, Context, Result};

/// DashScope TTS provider using qwen3-tts models.
pub struct DashScopeTtsProvider {
    api_key: String,
    model: String,
    base_url: String,
    default_voice: String,
    instructions: Option<String>,
    client: reqwest::Client,
}

/// Configuration for DashScope TTS provider.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct DashScopeTtsConfig {
    /// API key for DashScope. Falls back to `DASHSCOPE_API_KEY` env var.
    #[serde(default)]
    pub api_key: Option<String>,

    /// Model name. Default: `"qwen3-tts-instruct-flash"`.
    ///
    /// Options: `qwen3-tts-instruct-flash`, `qwen3-tts-flash`.
    #[serde(default = "default_dashscope_tts_model")]
    pub model: String,

    /// Default voice. See DashScope docs for available system voices.
    /// Common options: `Cherry`, `Ethan`, `Ryan`, `Serena`.
    /// Default: `"Cherry"`.
    #[serde(default = "default_dashscope_tts_voice")]
    pub default_voice: String,

    /// Base URL for DashScope API.
    /// Default: `"https://dashscope.aliyuncs.com"`.
    #[serde(default = "default_dashscope_tts_base_url")]
    pub base_url: String,

    /// Instruction for qwen3-tts-instruct models.
    /// Controls timbre, emotion, speed, and style via natural language.
    /// Example: `"用温柔甜美的声音朗读"`.
    /// Only effective with `qwen3-tts-instruct-*` models.
    #[serde(default)]
    pub instructions: Option<String>,
}

impl Default for DashScopeTtsConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            model: default_dashscope_tts_model(),
            default_voice: default_dashscope_tts_voice(),
            base_url: default_dashscope_tts_base_url(),
            instructions: None,
        }
    }
}

fn default_dashscope_tts_model() -> String {
    "qwen3-tts-instruct-flash".into()
}

fn default_dashscope_tts_voice() -> String {
    "Cherry".into()
}

fn default_dashscope_tts_base_url() -> String {
    "https://dashscope.aliyuncs.com".into()
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
            instructions: config.instructions.clone(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .context("Failed to build HTTP client for DashScope TTS")?,
        })
    }

    /// Synthesize text to audio bytes using qwen3-tts multimodal-generation API.
    ///
    /// Non-streaming mode: the API returns a JSON response containing an audio
    /// file URL. We download the audio from that URL.
    async fn synthesize_nonstream(&self, text: &str, voice: &str) -> Result<Vec<u8>> {
        let url = format!(
            "{}/api/v1/services/aigc/multimodal-generation/generation",
            self.base_url
        );

        let voice = if voice.is_empty() {
            &self.default_voice
        } else {
            voice
        };

        let mut input = serde_json::json!({
            "text": text,
            "voice": voice,
        });

        // Add instructions for instruct models
        if let Some(ref instructions) = self.instructions {
            if !instructions.is_empty() && self.model.contains("instruct") {
                input["instructions"] = serde_json::json!(instructions);
            }
        }

        let body = serde_json::json!({
            "model": self.model,
            "input": input,
        });

        tracing::info!(
            "DashScope TTS: synthesizing with model={}, voice={}",
            self.model, voice
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
        let resp_body: serde_json::Value = resp
            .json()
            .await
            .context("Failed to parse DashScope TTS response")?;

        if !status.is_success() {
            let err_msg = resp_body
                .pointer("/message")
                .or_else(|| resp_body.pointer("/error/message"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error");
            bail!("DashScope TTS API error ({status}): {err_msg}");
        }

        // Non-streaming response: extract audio URL from output.audio.url
        let audio_url = resp_body
            .pointer("/output/audio/url")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .context(
                "DashScope TTS response missing output.audio.url — \
                 ensure the model supports non-streaming TTS",
            )?;

        tracing::info!(
            "DashScope TTS: got audio URL, downloading (model={}, voice={})",
            self.model,
            voice
        );

        // Download the audio file
        let audio_resp = self
            .client
            .get(audio_url)
            .send()
            .await
            .context("Failed to download DashScope TTS audio")?;

        if !audio_resp.status().is_success() {
            bail!(
                "DashScope TTS audio download failed ({}): {}",
                audio_resp.status(),
                audio_url
            );
        }

        let audio_bytes = audio_resp
            .bytes()
            .await
            .context("Failed to read DashScope TTS audio bytes")?
            .to_vec();

        if audio_bytes.is_empty() {
            bail!("DashScope TTS: downloaded audio is empty");
        }

        tracing::info!(
            "DashScope TTS: synthesized {} bytes (model={}, voice={})",
            audio_bytes.len(),
            self.model,
            voice
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
        self.synthesize_nonstream(text, voice).await
    }

    fn supported_voices(&self) -> Vec<String> {
        // Common qwen3-tts system voices
        vec![
            "Cherry".to_string(),
            "Ethan".to_string(),
            "Ryan".to_string(),
            "Serena".to_string(),
            "Ava".to_string(),
        ]
    }

    fn supported_formats(&self) -> Vec<String> {
        // qwen3-tts outputs WAV (non-streaming) / PCM (streaming)
        vec!["wav".to_string(), "pcm".to_string()]
    }
}
