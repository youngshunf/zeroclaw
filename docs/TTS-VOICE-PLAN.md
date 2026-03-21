# 唤星 TTS 语音消息方案

> 文档路径: `docs/TTS-VOICE-PLAN.md`  
> 创建时间: 2026-03-21  
> 状态: **待审核**

---

## 一、需求

1. **Agent 主动发语音**：Agent 通过工具 `hx_tts(text, voice?)` 将文字转语音发送到当前会话
2. **语音消息自动回复语音**：用户发语音 → ASR 转文字 → Agent 处理 → TTS 转语音 → 回复语音
3. **NapCat 接收语音**：当前 NapCat 渠道不支持接收语音消息，需要补上
4. **渠道覆盖**：飞书 + NapCat (QQ)
5. **TTS 引擎**：阿里百炼 CosyVoice（OpenAI 兼容 HTTP API，零外部依赖）

---

## 二、技术方案

### 2.1 架构总览

```
用户发语音                         Agent 主动调 hx_tts
    │                                     │
    ▼                                     ▼
┌─────────────┐                  ┌─────────────────┐
│ 渠道接收语音  │                  │   hx_tts 工具    │
│ (ASR 转文字)  │                  │ (text + voice)  │
└──────┬──────┘                  └────────┬────────┘
       │ 文字                              │ 文字
       ▼                                   ▼
┌──────────────────────────────────────────────┐
│           TTS 引擎 (CosyVoice)                │
│  POST /compatible-mode/v1/audio/speech        │
│  → 直接输出 opus 音频 bytes                     │
└──────────────────────┬───────────────────────┘
                       │ opus bytes
                       ▼
┌──────────────────────────────────────────────┐
│             渠道发送语音                        │
│  飞书: 上传 opus → 发 audio 消息                │
│  NapCat: 保存临时文件 → [CQ:record] 发语音      │
└──────────────────────────────────────────────┘
```

### 2.2 百炼 CosyVoice API

**不需要 `dashscope` Python SDK，不需要 `ffmpeg`。**

百炼提供 OpenAI 兼容模式的 TTS HTTP API，与现有 `OpenAiTtsProvider` 代码几乎相同：

```
POST https://dashscope.aliyuncs.com/compatible-mode/v1/audio/speech
Authorization: Bearer sk-xxxxx
Content-Type: application/json

{
  "model": "cosyvoice-v3-flash",
  "input": "你好，我是唤星AI助手",
  "voice": "longanyang",
  "response_format": "opus"    // 直接输出 opus，飞书可直接用
}

→ 200 OK, Content-Type: audio/opus
→ Body: 二进制 opus 音频数据
```

**可选音色（cosyvoice-v3-flash）：**

| 音色 ID | 性别 | 风格 |
|---------|------|------|
| longanyang | 男 | 温暖 |
| longcheng | 男 | 沉稳 |
| longhua | 男 | 成熟 |
| longjing | 女 | 温柔 |
| longmiao | 女 | 甜美 |
| longshu | 男 | 儒雅 |
| longtong | 中性 | 童声 |
| longwan | 女 | 知性 |
| longxiaobai | 男 | 阳光 |
| longxiaochun | 女 | 活泼 |
| longxiaoxia | 女 | 可爱 |
| longyue | 女 | 大气 |
| longlaotie | 男 | 东北 |
| longjielidou | 男 | 搞笑 |
| longshuo | 男 | 播音 |

**费用：** CosyVoice-v3-flash 约 ¥0.0002/字符（极低）

---

## 三、代码改动清单

### 3.1 `src/config/schema.rs` — 配置 Schema 改动

**改动：`OpenAiTtsConfig` 增加 `base_url` 字段**

现有代码：
```rust
pub struct OpenAiTtsConfig {
    pub api_key: Option<String>,
    pub model: String,     // 默认 "tts-1"
    pub speed: f64,        // 默认 1.0
}
```

改为：
```rust
pub struct OpenAiTtsConfig {
    pub api_key: Option<String>,
    pub model: String,
    pub speed: f64,
    /// Base URL for OpenAI-compatible TTS API.
    /// Default: "https://api.openai.com" (原版 OpenAI)
    /// 百炼: "https://dashscope.aliyuncs.com/compatible-mode"
    #[serde(default = "default_openai_tts_base_url")]
    pub base_url: String,
}
```

### 3.2 `src/channels/tts.rs` — TTS 引擎改动

**改动：`OpenAiTtsProvider` 使用 `base_url` 而非硬编码 URL**

现有代码（第 93 行）：
```rust
.post("https://api.openai.com/v1/audio/speech")
```

改为：
```rust
.post(format!("{}/v1/audio/speech", self.base_url))
```

`OpenAiTtsProvider::new()` 中初始化 `base_url`：
```rust
base_url: config.base_url.trim_end_matches('/').to_string(),
```

**这样百炼 CosyVoice 就能直接复用 OpenAI TTS provider，零新增代码。**

### 3.3 `src/channels/lark.rs` — 飞书语音发送

**新增 3 个方法：**

#### 3.3.1 `upload_audio_file()` — 上传音频到飞书

```rust
/// 上传音频文件到飞书，返回 file_key
async fn upload_audio_file(
    &self,
    token: &str,
    audio_bytes: &[u8],
    filename: &str,
) -> anyhow::Result<String> {
    let form = reqwest::multipart::Form::new()
        .text("file_type", "opus")
        .text("file_name", filename.to_string())
        .part("file", reqwest::multipart::Part::bytes(audio_bytes.to_vec())
            .file_name(filename.to_string()));

    let resp = self.http_client()
        .post("https://open.feishu.cn/open-apis/im/v1/files")
        .header("Authorization", format!("Bearer {token}"))
        .multipart(form)
        .send()
        .await?;

    let body: serde_json::Value = resp.json().await?;
    let code = body["code"].as_i64().unwrap_or(-1);
    if code != 0 {
        anyhow::bail!("Lark upload audio failed: {body}");
    }
    body["data"]["file_key"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| anyhow::anyhow!("Lark upload: missing file_key"))
}
```

#### 3.3.2 `send_audio_message()` — 发送语音消息

```rust
/// 发送语音消息到指定会话
async fn send_audio_message(
    &self,
    recipient: &str,
    file_key: &str,
) -> anyhow::Result<()> {
    let token = self.get_tenant_access_token().await?;
    
    // 判断 receive_id_type
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

    let url = format!(
        "https://open.feishu.cn/open-apis/im/v1/messages?receive_id_type={receive_id_type}"
    );

    let resp = self.http_client()
        .post(&url)
        .header("Authorization", format!("Bearer {token}"))
        .json(&body)
        .send()
        .await?;

    let resp_body: serde_json::Value = resp.json().await?;
    if resp_body["code"].as_i64().unwrap_or(-1) != 0 {
        anyhow::bail!("Lark send audio failed: {resp_body}");
    }
    Ok(())
}
```

#### 3.3.3 飞书语音接收（ASR）

飞书自带语音转文字功能。当用户发送语音消息时，event payload 的 `message.message_type` 为 `"audio"`。

现有代码（lark.rs ~1157 行）：
```rust
_ => {
    tracing::debug!("Lark: skipping unsupported message type: {msg_type}");
    return messages;
}
```

新增 `"audio"` 分支：
```rust
"audio" => {
    // 飞书语音消息，content 包含 file_key
    // 需要下载音频 → ASR 转文字（或使用飞书自带 ASR 回传的识别结果）
    // 如果飞书 event 中包含 recognition 字段，直接用
    let recognition = serde_json::from_str::<serde_json::Value>(content_str)
        .ok()
        .and_then(|v| v.get("recognition").and_then(|r| r.as_str()).map(String::from));
    match recognition {
        Some(text) if !text.is_empty() => (format!("🎤 {text}"), Vec::new()),
        _ => {
            tracing::debug!("Lark: audio message without recognition text, skipping");
            return messages;
        }
    }
}
```

> **注意**：需要在飞书开发者后台开启「语音识别」权限，事件回调中才会包含 `recognition` 字段。如果飞书不提供 ASR 结果，则需要额外接入 ASR 服务（如百炼 Paraformer）。

### 3.4 `src/channels/napcat.rs` — NapCat 语音收发

#### 3.4.1 接收语音消息

`parse_message_segments()` 函数新增 `"record"` 消息段类型：

```rust
match seg_type {
    "text" => { /* 现有逻辑 */ }
    "image" => { /* 现有逻辑 */ }
    // ── 新增：语音消息 ──
    "record" => {
        // NapCat 语音消息结构:
        // {"type": "record", "data": {"file": "xxx.silk", "url": "http://..."}}
        if let Some(url) = data
            .and_then(|d| d.get("url"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            parts.push(format!("[VOICE:{url}]"));
        } else if let Some(file) = data
            .and_then(|d| d.get("file"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            parts.push(format!("[VOICE:{file}]"));
        }
    }
    _ => {}
}
```

**语音 ASR 处理**：NapCat (OneBot) 协议本身不提供语音转文字。需要在收到 `[VOICE:url]` 后：
1. 下载语音文件（silk/amr 格式）
2. 调 ASR 服务转文字（推荐百炼 Paraformer：`POST /compatible-mode/v1/audio/transcriptions`）

> 第一期可以先只标记 `[VOICE:url]`，让 Agent 知道收到了语音但暂时无法识别。后续接入 ASR 时再完善。

#### 3.4.2 发送语音消息

`compose_onebot_content()` 函数新增 `[VOICE:...]` 标记解析：

```rust
// 现有 [IMAGE:...] 解析逻辑后面，新增:
if let Some(marker) = trimmed
    .strip_prefix("[VOICE:")
    .and_then(|v| v.strip_suffix(']'))
    .map(str::trim)
    .filter(|v| !v.is_empty())
{
    parts.push(format!("[CQ:record,file={marker}]"));
    continue;
}
```

NapCat 支持的 `[CQ:record]` 格式：
- `[CQ:record,file=http://xxx/voice.mp3]` — URL 发送（推荐）
- `[CQ:record,file=file:///tmp/voice.mp3]` — 本地文件发送
- `[CQ:record,file=base64://...]` — Base64 编码发送

### 3.5 `src/huanxing/tools.rs` — 新增 `hx_tts` 工具

```rust
/// hx_tts — 文字转语音并发送到当前会话
///
/// 参数:
///   text: String     — 要转换的文字内容（必填）
///   voice: String    — 音色（可选，默认使用配置的 default_voice）
///
/// 流程:
///   1. 调用 TTS 引擎（CosyVoice）生成 opus 音频
///   2. 根据当前渠道类型选择发送方式:
///      - 飞书: upload_audio_file → send_audio_message
///      - NapCat: 保存临时文件 → compose [VOICE:file_path] 发送
///   3. 返回结果: "语音已发送" 或错误信息
```

工具定义：
```rust
Tool {
    name: "hx_tts",
    description: "将文字转换为语音消息并发送到当前会话。用于主动发送语音播报、提醒等。",
    parameters: json!({
        "type": "object",
        "properties": {
            "text": {
                "type": "string",
                "description": "要转换为语音的文字内容"
            },
            "voice": {
                "type": "string",
                "description": "音色选择（可选）。可选值: longanyang(温暖男声), longjing(温柔女声), longmiao(甜美女声), longxiaobai(阳光男声) 等",
                "default": "longanyang"
            }
        },
        "required": ["text"]
    }),
}
```

### 3.6 `src/channels/mod.rs` — 集成自动语音回复

在消息处理流程中，当检测到用户发送的是语音消息（包含 `🎤` 前缀或 `[VOICE:]` 标记）时，设置 `is_voice_chat = true`。Agent 回复后，如果 `is_voice_chat && tts.enabled`：

1. 调用 `TtsManager::synthesize()` 生成 opus 音频
2. 根据渠道类型调用对应的语音发送方法
3. 如果 TTS 失败，降级为普通文字发送

---

## 四、配置

### 4.1 `server-config/config.toml` 新增

```toml
[tts]
enabled = true
default_provider = "openai"          # 复用 OpenAI 兼容格式
default_voice = "longanyang"         # 默认音色：温暖男声
default_format = "opus"              # opus 格式，飞书直接可用
max_text_length = 4096               # 最大文字长度

[tts.openai]
# 百炼 CosyVoice（OpenAI 兼容模式）
base_url = "https://dashscope.aliyuncs.com/compatible-mode"
model = "cosyvoice-v3-flash"        # 快速版，低延迟
speed = 1.0
# api_key 从 .env 的 DASHSCOPE_API_KEY 读取
```

### 4.2 `.env` 新增

```bash
DASHSCOPE_API_KEY=sk-36802b41aa02462ca128e4cfc5c328f1
```

### 4.3 飞书权限

需要在飞书开发者后台开启：
- `im:message:send_as_bot` — 发送消息（已有）
- `im:file` — 上传/下载文件（发语音需要）
- `im:message.audio:readonly` — 接收语音消息（可选，用于 ASR）

---

## 五、外部依赖

**零新增依赖。** 所有实现均使用现有的：
- `reqwest` — HTTP 请求（已有）
- `serde_json` — JSON 处理（已有）
- `tokio` — 异步运行时（已有）
- `uuid` — 临时文件命名（已有）

不需要：`dashscope`❌、`ffmpeg`❌、`edge-tts`❌、任何 Python 包 ❌

---

## 六、实现优先级

| 阶段 | 内容 | 工作量 |
|------|------|--------|
| **P0** | `tts.rs` 加 `base_url` + 配置改动 | 0.5h |
| **P0** | `lark.rs` 语音发送（upload + send_audio） | 1h |
| **P0** | `napcat.rs` 语音接收（`record` 消息段解析） | 0.5h |
| **P0** | `napcat.rs` 语音发送（`[CQ:record]`） | 0.5h |
| **P0** | `tools.rs` 新增 `hx_tts` 工具 | 1h |
| **P1** | `mod.rs` 自动语音回复集成 | 1.5h |
| **P1** | `lark.rs` 语音接收（audio message → ASR） | 1h |
| **P2** | NapCat 语音 ASR（下载 + 百炼 Paraformer） | 2h |

**P0 总计约 3.5h，可一次完成编码 + 编译 + 部署。**

---

## 七、测试验证

### 7.1 TTS 引擎测试
```bash
# 直接 curl 测试百炼 API
curl -X POST "https://dashscope.aliyuncs.com/compatible-mode/v1/audio/speech" \
  -H "Authorization: Bearer sk-36802b41aa02462ca128e4cfc5c328f1" \
  -H "Content-Type: application/json" \
  -d '{"model":"cosyvoice-v3-flash","input":"你好","voice":"longanyang","response_format":"opus"}' \
  --output /tmp/test.opus
# 检查文件大小 > 0
ls -la /tmp/test.opus
```

### 7.2 飞书语音发送测试
- Agent 调用 `hx_tts("今天市场表现不错")` → 飞书群收到语音消息

### 7.3 NapCat 语音测试
- 在 QQ 发送语音 → Agent 收到 `[VOICE:url]` 标记
- Agent 调用 `hx_tts("收到你的语音")` → QQ 收到语音回复

---

## 八、风险与回退

| 风险 | 应对 |
|------|------|
| 百炼 API 不支持 `opus` 输出格式 | 降级为 `mp3`，飞书端需 ffmpeg 转码或用 mp3 直传 |
| 百炼 API 不稳定 | 配置 Edge TTS 作为免费备用（已安装在服务器） |
| 飞书 audio 消息发送权限不足 | 在开发者后台申请 `im:file` 权限 |
| NapCat 不支持 `[CQ:record]` 发语音 | 使用 NapCat HTTP API `/send_private_msg` + record segment JSON 格式 |

---

## 九、百炼 API 兼容性验证（待执行）

在开始编码前，先执行 7.1 的 curl 测试，确认：
1. ✅ 百炼 OpenAI 兼容 API 支持 `cosyvoice-v3-flash` 模型
2. ✅ 支持 `response_format: "opus"` 输出
3. ✅ `DASHSCOPE_API_KEY` 有效

确认通过后即可开始编码。
