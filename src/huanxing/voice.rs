//! HuanXing voice processing module.
//!
//! Provides ASR (speech-to-text) and TTS (text-to-speech) capabilities
//! for NapCat and Lark channels, following the huanxing architecture pattern:
//! all huanxing-specific logic lives in `src/huanxing/`, not in upstream files.
//!
//! ## ASR (Speech-to-Text)
//! - Uses DashScope `qwen2.5-omni-7b` to transcribe audio from URLs
//! - Called from channel message processing hooks
//!
//! ## TTS (Text-to-Speech)
//! - Uses DashScope provider (registered in `tts_dashscope.rs`)
//! - Wraps `TtsManager` to provide `synthesize_with_voice()`
//!
//! ## Lark Voice Helpers
//! - Upload audio files to Feishu
//! - Send audio messages
//! - Parse voice message content (ASR recognition extraction)

use anyhow::{Context, Result};

// ── ASR via DashScope ─────────────────────────────────────────────

/// Transcribe audio from a URL using DashScope qwen2.5-omni-7b.
///
/// Sends the audio URL as an `input_audio` content part to the chat completions
/// endpoint, which returns the transcribed text.
pub async fn asr_via_dashscope(audio_url: &str) -> Result<String> {
    let api_key = std::env::var("DASHSCOPE_API_KEY")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .context("DASHSCOPE_API_KEY not set for ASR")?;

    // First download the audio file
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let audio_bytes = client
        .get(audio_url)
        .send()
        .await
        .context("Failed to download voice audio")?
        .bytes()
        .await
        .context("Failed to read voice audio bytes")?;

    if audio_bytes.is_empty() {
        anyhow::bail!("Downloaded audio is empty");
    }

    tracing::info!("ASR: downloaded {} bytes of audio", audio_bytes.len());

    // Convert to base64 for inline audio input
    use base64::Engine;
    let audio_b64 = base64::engine::general_purpose::STANDARD.encode(&audio_bytes);

    // Use qwen2.5-omni-7b with audio input for transcription
    let body = serde_json::json!({
        "model": "qwen2.5-omni-7b",
        "messages": [
            {
                "role": "system",
                "content": [{"type": "text", "text": "你是一个语音转文字助手。请将用户发送的语音内容精确转写为文字，只输出转写结果，不要添加任何解释。如果听不清楚，输出你最好的猜测。"}]
            },
            {
                "role": "user",
                "content": [
                    {
                        "type": "input_audio",
                        "input_audio": {
                            "data": format!("data:audio/ogg;base64,{audio_b64}"),
                            "format": "ogg"
                        }
                    },
                    {
                        "type": "text",
                        "text": "请转写这段语音内容。"
                    }
                ]
            }
        ],
        "modalities": ["text"],
        "max_tokens": 1024,
        "stream": false
    });

    let resp = client
        .post("https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions")
        .bearer_auth(&api_key)
        .json(&body)
        .send()
        .await
        .context("ASR: failed to send DashScope request")?;

    let status = resp.status();
    let resp_body: serde_json::Value = resp.json().await.context("ASR: failed to parse response")?;

    if !status.is_success() {
        let err_msg = resp_body["error"]["message"]
            .as_str()
            .unwrap_or("unknown error");
        anyhow::bail!("ASR API error ({status}): {err_msg}");
    }

    let text = resp_body["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string();

    Ok(text)
}

/// Scan message content for `[VOICE:url]` markers and attempt ASR transcription.
///
/// If a `[VOICE:url]` marker is found, downloads the audio and transcribes it
/// using the upstream `TranscriptionManager` (configured via `[transcription]`
/// in config.toml). The marker is replaced with the transcribed text prefixed
/// by 🎤.
pub async fn transcribe_voice_markers(
    content: String,
    transcription_config: &crate::config::TranscriptionConfig,
) -> String {
    let Some(start) = content.find("[VOICE:") else {
        return content;
    };
    let Some(end) = content[start..].find(']') else {
        return content;
    };
    let voice_url = &content[start + 7..start + end];
    if voice_url.is_empty() {
        return content;
    }

    tracing::info!("ASR: attempting to transcribe voice from {voice_url}");

    match transcribe_voice_url(voice_url, transcription_config).await {
        Ok(text) if !text.is_empty() => {
            tracing::info!("ASR: transcribed text: {text}");
            let before = &content[..start];
            let after = &content[start + end + 1..];
            format!("{before}🎤 {text}{after}").trim().to_string()
        }
        Ok(_) => {
            tracing::warn!("ASR: empty transcription result");
            content
        }
        Err(e) => {
            tracing::warn!("ASR: transcription failed: {e}");
            content
        }
    }
}

/// Download audio from a URL and transcribe using the upstream TranscriptionManager.
///
/// Uses the provided `TranscriptionConfig` (typically GroqProvider pointing at
/// DashScope qwen3-asr-flash OpenAI-compatible endpoint).
async fn transcribe_voice_url(
    audio_url: &str,
    config: &crate::config::TranscriptionConfig,
) -> Result<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let audio_bytes = client
        .get(audio_url)
        .send()
        .await
        .context("Failed to download voice audio")?
        .bytes()
        .await
        .context("Failed to read voice audio bytes")?;

    if audio_bytes.is_empty() {
        anyhow::bail!("Downloaded audio is empty");
    }

    tracing::info!("ASR: downloaded {} bytes of audio", audio_bytes.len());

    // Derive filename from URL for MIME type detection
    let file_name = audio_url
        .split('?')
        .next()
        .and_then(|path| path.rsplit('/').next())
        .unwrap_or("voice.ogg")
        .to_string();

    crate::channels::transcription::transcribe_audio(audio_bytes.to_vec(), &file_name, config)
        .await
}

// ── NapCat Voice Helpers ──────────────────────────────────────────

/// Parse NapCat voice message segments: convert `"record"` type to `[VOICE:url]`.
///
/// Call this from a hook in NapCat message parsing.
pub fn parse_napcat_voice_segment(data: Option<&serde_json::Value>) -> Option<String> {
    if let Some(url) = data
        .and_then(|d| d.get("url"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        return Some(format!("[VOICE:{url}]"));
    }
    if let Some(file) = data
        .and_then(|d| d.get("file"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        return Some(format!("[VOICE:{file}]"));
    }
    None
}

/// Compose a NapCat `[CQ:record]` segment from a `[VOICE:marker]` line.
///
/// Returns `Some(cq_segment)` if the line is a VOICE marker, `None` otherwise.
pub fn compose_napcat_voice_segment(trimmed_line: &str) -> Option<String> {
    trimmed_line
        .strip_prefix("[VOICE:")
        .and_then(|v| v.strip_suffix(']'))
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|marker| format!("[CQ:record,file={marker}]"))
}

// ── Lark Voice Helpers ────────────────────────────────────────────

/// Parse Lark audio message content — extract ASR recognition text if available.
///
/// Feishu includes a "recognition" field when speech recognition is enabled.
/// Returns `(display_text, mentioned_open_ids)`.
pub fn parse_lark_audio_content(content_str: &str) -> (String, Vec<String>) {
    let recognition = serde_json::from_str::<serde_json::Value>(content_str)
        .ok()
        .and_then(|v| {
            v.get("recognition")
                .and_then(|r| r.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from)
        });

    match recognition {
        Some(text) => {
            tracing::info!("Lark: received voice message, ASR text: {text}");
            (format!("🎤 {text}"), Vec::new())
        }
        None => {
            let file_key = serde_json::from_str::<serde_json::Value>(content_str)
                .ok()
                .and_then(|v| v.get("file_key").and_then(|f| f.as_str()).map(String::from));
            tracing::info!(
                "Lark: received voice message without ASR (file_key: {:?})",
                file_key
            );
            (
                "🎤 [收到语音消息，暂不支持语音识别]".to_string(),
                Vec::new(),
            )
        }
    }
}

/// Upload audio bytes to Feishu and return the `file_key`.
pub async fn lark_upload_audio(
    client: &reqwest::Client,
    api_base: &str,
    token: &str,
    audio_bytes: &[u8],
    filename: &str,
) -> Result<String> {
    let url = format!("{api_base}/im/v1/files");

    let form = reqwest::multipart::Form::new()
        .text("file_type", "opus")
        .text("file_name", filename.to_string())
        .part(
            "file",
            reqwest::multipart::Part::bytes(audio_bytes.to_vec())
                .file_name(filename.to_string()),
        );

    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {token}"))
        .multipart(form)
        .send()
        .await
        .context("Lark: failed to upload audio file")?;

    let body: serde_json::Value = resp.json().await?;
    let code = body["code"].as_i64().unwrap_or(-1);
    if code != 0 {
        anyhow::bail!("Lark upload audio failed (code {code}): {body}");
    }

    body["data"]["file_key"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| anyhow::anyhow!("Lark upload audio: missing file_key in response"))
}

/// Send an audio message to a Lark recipient.
pub async fn lark_send_audio(
    client: &reqwest::Client,
    api_base: &str,
    token: &str,
    recipient: &str,
    file_key: &str,
) -> Result<()> {
    let receive_id_type = if recipient.starts_with("oc_") {
        "chat_id"
    } else {
        "open_id"
    };

    let body = serde_json::json!({
        "receive_id": recipient,
        "msg_type": "audio",
        "content": serde_json::json!({"file_key": file_key}).to_string(),
    });

    let url = format!("{api_base}/im/v1/messages?receive_id_type={receive_id_type}");

    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {token}"))
        .json(&body)
        .send()
        .await
        .context("Lark: failed to send audio message")?;

    let status = resp.status();
    let resp_body: serde_json::Value = resp.json().await.unwrap_or_default();
    let code = resp_body["code"].as_i64().unwrap_or(-1);

    if !status.is_success() || code != 0 {
        anyhow::bail!(
            "Lark send audio failed (status={status}, code={code}): {resp_body}"
        );
    }

    tracing::info!("Lark: sent audio message to {recipient}");
    Ok(())
}

// ── TTS Helper ────────────────────────────────────────────────────

/// Synthesize text with a specific voice using TtsManager.
///
/// This wraps `TtsManager::synthesize_with_provider` to provide a voice-specific
/// synthesis method, since upstream only has `synthesize()` (default voice).
pub async fn synthesize_with_voice(
    tts_config: &crate::config::TtsConfig,
    text: &str,
    voice: &str,
) -> Result<Vec<u8>> {
    let tts_manager = crate::channels::tts::TtsManager::new(tts_config)?;
    tts_manager
        .synthesize_with_provider(text, &tts_config.default_provider, voice)
        .await
}
