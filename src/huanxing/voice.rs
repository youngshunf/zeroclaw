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
    let resp_body: serde_json::Value =
        resp.json().await.context("ASR: failed to parse response")?;

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

    let response = client
        .get(audio_url)
        .send()
        .await
        .context("Failed to download voice audio")?;

    // Extract Content-Type before consuming the response body
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let audio_bytes = response
        .bytes()
        .await
        .context("Failed to read voice audio bytes")?;

    if audio_bytes.is_empty() {
        anyhow::bail!("Downloaded audio is empty");
    }

    tracing::info!(
        "ASR: downloaded {} bytes of audio, content_type={}",
        audio_bytes.len(),
        content_type
    );

    let file_name = derive_audio_filename(audio_url, &content_type, &audio_bytes);
    tracing::info!("ASR: derived filename={file_name}");

    crate::channels::transcription::transcribe_audio(audio_bytes.to_vec(), &file_name, config).await
}

/// Derive a filename with proper extension for audio format detection.
///
/// Priority:
/// 1. URL path extension (e.g. `voice.ogg`)
/// 2. HTTP `Content-Type` header (e.g. `audio/ogg` → `.ogg`)
/// 3. Audio magic bytes (OGG: `OggS`, AMR: `#!AMR`, etc.)
/// 4. Default: `voice.ogg`
fn derive_audio_filename(url: &str, content_type: &str, data: &[u8]) -> String {
    // 1. Try URL path extension
    let url_name = url
        .split('?')
        .next()
        .and_then(|path| path.rsplit('/').next())
        .unwrap_or("");

    if let Some((_, ext)) = url_name.rsplit_once('.') {
        let ext_lower = ext.to_ascii_lowercase();
        if matches!(
            ext_lower.as_str(),
            "flac"
                | "mp3"
                | "mp4"
                | "mpeg"
                | "mpga"
                | "m4a"
                | "ogg"
                | "oga"
                | "opus"
                | "wav"
                | "webm"
                | "amr"
        ) {
            return format!("voice.{ext_lower}");
        }
    }

    // 2. Try Content-Type header
    if let Some(ext) = extension_from_content_type(content_type) {
        return format!("voice.{ext}");
    }

    // 3. Try magic bytes
    if let Some(ext) = extension_from_magic_bytes(data) {
        return format!("voice.{ext}");
    }

    // 4. Default
    "voice.ogg".to_string()
}

/// Map MIME Content-Type to file extension.
fn extension_from_content_type(ct: &str) -> Option<&'static str> {
    let ct_lower = ct.to_ascii_lowercase();
    // Strip parameters (e.g. "audio/ogg; codecs=opus" -> "audio/ogg")
    let mime = ct_lower.split(';').next().unwrap_or("").trim();
    match mime {
        "audio/ogg" | "audio/vorbis" => Some("ogg"),
        "audio/opus" => Some("opus"),
        "audio/mpeg" | "audio/mp3" => Some("mp3"),
        "audio/mp4" | "audio/m4a" | "audio/x-m4a" => Some("m4a"),
        "audio/wav" | "audio/x-wav" | "audio/wave" => Some("wav"),
        "audio/flac" | "audio/x-flac" => Some("flac"),
        "audio/webm" => Some("webm"),
        "audio/amr" | "audio/amr-wb" => Some("amr"),
        "audio/silk" | "audio/x-silk" => Some("silk"),
        _ => None,
    }
}

/// Detect audio format from magic bytes in the file header.
fn extension_from_magic_bytes(data: &[u8]) -> Option<&'static str> {
    if data.len() < 4 {
        return None;
    }
    // OGG container (Vorbis/Opus)
    if data.starts_with(b"OggS") {
        return Some("ogg");
    }
    // RIFF/WAV
    if data.starts_with(b"RIFF") && data.len() >= 12 && &data[8..12] == b"WAVE" {
        return Some("wav");
    }
    // MP3 (ID3 tag or sync word)
    if data.starts_with(b"ID3") || (data[0] == 0xFF && (data[1] & 0xE0) == 0xE0) {
        return Some("mp3");
    }
    // FLAC
    if data.starts_with(b"fLaC") {
        return Some("flac");
    }
    // AMR
    if data.starts_with(b"#!AMR") {
        return Some("amr");
    }
    // WebM/Matroska
    if data.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        return Some("webm");
    }
    // SILK (used by Tencent/WeChat)
    if data.starts_with(b"\x02#!SILK") || data.starts_with(b"#!SILK") {
        return Some("silk");
    }
    None
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

/// Configuration for huanxing voice auto-synthesis.
///
/// Extracted from the tenant's config.toml at runtime, avoiding any
/// dependency on the upstream `TtsManager` or `TtsConfig` structs.
pub struct HxVoiceConfig {
    /// Gateway TTS endpoint (e.g. `https://llm.dcfuture.cn/v1/audio/speech`)
    pub api_url: String,
    /// Tenant API key (from root-level `api_key` in config.toml)
    pub api_key: String,
    /// TTS model name (e.g. `qwen3-tts-instruct-flash`)
    pub model: String,
    /// Default voice when none specified (e.g. `Cherry`)
    pub default_voice: String,
    /// Audio output format (e.g. `wav`)
    pub format: String,
}

impl HxVoiceConfig {
    /// Build from a tenant's `Config`.
    ///
    /// Uses `tts.generic_openai` settings with the root-level `api_key` as
    /// fallback (same logic as `tools/mod.rs` when registering `hx_tts`).
    pub fn from_config(config: &crate::config::Config) -> Option<Self> {
        if !config.tts.enabled {
            return None;
        }
        let generic = config.tts.generic_openai.as_ref()?;

        let api_key = generic
            .api_key
            .as_deref()
            .filter(|k| !k.trim().is_empty())
            .or(config.api_key.as_deref())
            .filter(|k| !k.trim().is_empty())?
            .trim()
            .to_string();

        Some(Self {
            api_url: generic.api_url.clone(),
            api_key,
            model: generic.model.clone(),
            default_voice: config.tts.default_voice.clone(),
            format: config.tts.default_format.clone(),
        })
    }
}

/// Heuristic: does this string look like speech text rather than a voice name?
///
/// Voice names are short ASCII identifiers like `Chelsie`, `Kai`, `Moon`.
/// Speech text is typically longer and/or contains CJK characters.
fn looks_like_speech_text(s: &str) -> bool {
    // Contains CJK characters → definitely speech text
    if s.chars().any(|c| ('\u{4E00}'..='\u{9FFF}').contains(&c)) {
        return true;
    }
    // Longer than typical voice name
    s.len() > 30
}

/// Auto-synthesize `[VOICE:voice_name]text...` markers in the response text.
///
/// When the LLM outputs `[VOICE:Cherry]你好呀...` as text (instead of calling
/// the `hx_tts` tool), this function intercepts the pattern, calls TTS to
/// synthesize audio, saves it to a temp file, and replaces the original line
/// with `[VOICE:/path/to/audio.wav]` so the channel layer sends it as voice.
///
/// Pattern: `[VOICE:voice_name]text to speak`
/// If the marker looks like a file path or URL, it is left unchanged.
pub async fn auto_synthesize_voice_markers(
    response: &str,
    voice_config: &HxVoiceConfig,
    workspace_dir: &std::path::Path,
) -> String {
    // Quick bail-out if no VOICE markers present
    if !response.contains("[VOICE:") {
        return response.to_string();
    }

    let mut result = String::new();
    for line in response.lines() {
        let trimmed = line.trim();

        // Match pattern: [VOICE:...]
        if let Some(after_prefix) = trimmed.strip_prefix("[VOICE:") {
            // Case 1: [VOICE:content] — content is everything up to closing bracket
            if after_prefix.ends_with(']') {
                let marker = &after_prefix[..after_prefix.len() - 1];

                // 1a. If marker is a file path or URL, leave it alone
                if marker.starts_with('/')
                    || marker.starts_with("http")
                    || marker.starts_with("file:")
                    || marker.starts_with("localfile:")
                {
                    // Normalize the marker, removing localfile: so it's a standard path
                    let marker_normalized = marker.strip_prefix("localfile:").unwrap_or(marker);
                    result.push_str(&line.replace(marker, marker_normalized));
                    result.push('\n');
                    continue;
                }

                // 1b. If marker looks like speech text (not a voice name),
                //     synthesize with default voice
                if looks_like_speech_text(marker) {
                    match synthesize_and_save(
                        voice_config,
                        marker,
                        &voice_config.default_voice,
                        workspace_dir,
                    )
                    .await
                    {
                        Ok(file_path) => {
                            tracing::info!(
                                voice = %voice_config.default_voice,
                                text_len = marker.len(),
                                file = %file_path,
                                "Auto-synthesized VOICE marker (default voice) to audio file"
                            );
                            result.push_str(&format!("[VOICE:{file_path}]"));
                            result.push('\n');
                            continue;
                        }
                        Err(e) => {
                            tracing::warn!("Auto-TTS synthesis failed: {e}; falling back to text");
                            result.push_str(marker);
                            result.push('\n');
                            continue;
                        }
                    }
                }
            }

            // Case 2: [VOICE:voice_name]text... — voice name + text after bracket
            if let Some(bracket_pos) = after_prefix.find(']') {
                let voice_name = after_prefix[..bracket_pos].trim();
                let text = after_prefix[bracket_pos + 1..].trim();

                if !text.is_empty() && !voice_name.is_empty() {
                    match synthesize_and_save(voice_config, text, voice_name, workspace_dir).await {
                        Ok(file_path) => {
                            tracing::info!(
                                voice = voice_name,
                                text_len = text.len(),
                                file = %file_path,
                                "Auto-synthesized VOICE marker to audio file"
                            );
                            result.push_str(&format!("[VOICE:{file_path}]"));
                            result.push('\n');
                            continue;
                        }
                        Err(e) => {
                            tracing::warn!(
                                voice = voice_name,
                                "Auto-TTS synthesis failed: {e}; falling back to text"
                            );
                            result.push_str(text);
                            result.push('\n');
                            continue;
                        }
                    }
                }
            }
        }

        result.push_str(line);
        result.push('\n');
    }

    // Remove trailing newline if original didn't have one
    if !response.ends_with('\n') {
        result.truncate(result.trim_end_matches('\n').len());
    }

    result
}

/// Synthesize text to audio by calling the LLM gateway directly.
///
/// POST to `/v1/audio/speech` with the tenant's api_key — no upstream
/// `TtsManager` dependency.
pub async fn synthesize_and_save(
    config: &HxVoiceConfig,
    text: &str,
    voice: &str,
    workspace_dir: &std::path::Path,
) -> Result<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    let body = serde_json::json!({
        "model": config.model,
        "input": text,
        "voice": voice,
    });

    let resp = client
        .post(&config.api_url)
        .bearer_auth(&config.api_key)
        .json(&body)
        .send()
        .await
        .context("HxVoice TTS: failed to send request")?;

    let status = resp.status();

    // If the API returns a 302 Redirect (e.g. from New-API minimax/ali TTS direct OSS links),
    // capture the Location header and return it as the audio URL, avoiding local cache.
    if status.is_redirection() {
        if let Some(loc) = resp.headers().get(reqwest::header::LOCATION) {
            if let Ok(url_str) = loc.to_str() {
                return Ok(url_str.to_string());
            }
        }
    }

    if !status.is_success() {
        let err_body = resp.text().await.unwrap_or_default();
        anyhow::bail!("HxVoice TTS API error ({status}): {err_body}");
    }

    let audio_bytes = resp
        .bytes()
        .await
        .context("HxVoice TTS: failed to read audio bytes")?;

    if audio_bytes.is_empty() {
        anyhow::bail!("HxVoice TTS: returned empty audio");
    }

    // Save to tenant workspace
    let audio_dir = workspace_dir.join("tts_cache");
    tokio::fs::create_dir_all(&audio_dir).await?;

    let file_name = format!("voice_{}.{}", uuid::Uuid::new_v4(), config.format);
    let file_path = audio_dir.join(&file_name);
    tokio::fs::write(&file_path, &audio_bytes).await?;

    Ok(format!("file://{}", file_path.to_string_lossy()))
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
            reqwest::multipart::Part::bytes(audio_bytes.to_vec()).file_name(filename.to_string()),
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
        anyhow::bail!("Lark send audio failed (status={status}, code={code}): {resp_body}");
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
