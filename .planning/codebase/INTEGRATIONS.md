# 外部集成

**分析日期：** 2026-03-21

## LLM 提供商

**支持的提供商（15+）：**

| 提供商 | 实现文件 | 认证方式 | 特性 |
|--------|---------|---------|------|
| OpenAI | `src/providers/openai.rs` | API Key | 流式、函数调用、视觉 |
| Anthropic | `src/providers/anthropic.rs` | API Key | 流式、工具使用、提示缓存 |
| Google Gemini | `src/providers/gemini.rs` | API Key | 流式、多模态 |
| Ollama | `src/providers/ollama.rs` | 无（本地） | 流式、本地部署 |
| OpenAI 兼容 | `src/providers/compatible.rs` | API Key | 通用兼容层 |
| Groq | `src/providers/groq.rs` | API Key | 超快推理 |
| Mistral | `src/providers/mistral.rs` | API Key | 流式 |
| Cohere | `src/providers/cohere.rs` | API Key | 流式 |
| Perplexity | `src/providers/perplexity.rs` | API Key | 搜索增强 |
| Zhipu/GLM | `src/providers/zhipu.rs` | JWT（HMAC-SHA256） | 国内模型 |
| DeepSeek | `src/providers/deepseek.rs` | API Key | 国内模型 |
| Moonshot | `src/providers/moonshot.rs` | API Key | 国内模型 |
| Baichuan | `src/providers/baichuan.rs` | API Key | 国内模型 |
| Minimax | `src/providers/minimax.rs` | API Key | 国内模型 |
| Azure OpenAI | `src/providers/azure.rs` | API Key + Deployment | 企业部署 |

**弹性包装器（`src/providers/reliable.rs`）：**
- 自动重试（指数退避）
- 熔断器（连续失败阈值）
- 超时控制
- 错误分类（可重试 vs 不可重试）

**动态路由（`src/providers/router.rs`）：**
- 按模型名称路由到不同提供商
- 支持回退链（主提供商失败 → 备用提供商）

---

## 消息渠道（20+）

**即时通讯平台：**

| 渠道 | 实现文件 | 协议 | 认证 | 特性 |
|------|---------|------|------|------|
| Telegram | `src/channels/telegram.rs` | HTTP Long Polling | Bot Token | 消息、媒体、回复、编辑 |
| Discord | `src/channels/discord.rs` | WebSocket Gateway | Bot Token | 消息、线程、嵌入、反应 |
| Slack | `src/channels/slack.rs` | WebSocket RTM | Bot Token | 消息、线程、文件、反应 |
| Matrix | `src/channels/matrix.rs` | HTTP + Sync | 用户名/密码 | E2EE、消息、媒体 |
| Lark/飞书 | `src/channels/lark.rs` | WebSocket + HTTP | App ID/Secret | 消息、卡片、审批 |
| DingTalk | `src/channels/dingtalk.rs` | WebSocket + HTTP | App Key/Secret | 消息、卡片 |
| 企业微信 | `src/channels/wecom.rs` | HTTP Webhook | Corp ID/Secret | 消息、应用消息 |
| QQ（NapCat） | `src/channels/napcat.rs` | HTTP + WebSocket | 无（本地） | 消息、群聊、好友 |
| WhatsApp | `src/channels/whatsapp.rs` | HTTP Webhook | API Token | 消息、媒体、模板 |
| WhatsApp Web | `src/channels/whatsapp_web.rs` | WebSocket | QR 码 | 消息、媒体、群聊 |
| Signal | `src/channels/signal.rs` | HTTP（signal-cli） | 手机号 | 消息、群聊 |
| iMessage | `src/channels/imessage.rs` | AppleScript | 无 | 消息（仅 macOS） |
| Mattermost | `src/channels/mattermost.rs` | WebSocket | Token | 消息、线程 |
| IRC | `src/channels/irc.rs` | IRC 协议 | 昵称/密码 | 消息、频道 |
| MQTT | `src/channels/mqtt.rs` | MQTT | 用户名/密码 | 发布/订阅 |
| Nostr | `src/channels/nostr.rs` | Nostr 协议 | 私钥 | 去中心化消息 |
| Bluesky | `src/channels/bluesky.rs` | HTTP API | 用户名/密码 | 消息、帖子 |
| Twitter/X | `src/channels/twitter.rs` | HTTP API | OAuth | 推文、DM |
| Reddit | `src/channels/reddit.rs` | HTTP API | OAuth | 评论、私信 |
| Nextcloud Talk | `src/channels/nextcloud_talk.rs` | HTTP API | Token | 消息、房间 |

**特殊渠道：**
- CLI (`src/channels/cli.rs`) — 命令行交互
- Webhook (`src/channels/webhook.rs`) — 通用 HTTP Webhook
- Email (`src/channels/email_channel.rs`) — SMTP + IMAP
- TTS (`src/channels/tts.rs`) — 语音合成输出
- Notion (`src/channels/notion.rs`) — Notion 页面评论

**会话持久化：**
- SQLite 后端（`src/channels/session_sqlite.rs`）
- 抽象接口（`src/channels/session_backend.rs`）
- 会话存储（`src/channels/session_store.rs`）

---

## 数据库

**SQLite（主要后端）：**
- 库：rusqlite 0.37 (bundled)
- 用途：记忆存储、会话历史、租户数据库
- 位置：`~/.zeroclaw/memory.db`, `data/users.db`（唤星）
- 实现：`src/memory/sqlite.rs`, `src/channels/session_sqlite.rs`

**PostgreSQL（可选）：**
- 库：postgres 0.19
- 用途：企业级记忆后端
- 实现：`src/memory/postgres.rs`
- 特性：schema 隔离、连接池、超时控制

**向量数据库（可选）：**
- Qdrant（`src/memory/qdrant.rs`）— HTTP REST API
- Mem0（`src/memory/mem0.rs`）— HTTP API
- 用途：语义搜索、记忆检索

---

## 认证提供商

**OAuth 2.0 / OpenID Connect：**
- Microsoft 365（`src/tools/microsoft365/auth.rs`）
  - 流程：设备码流（Device Code Flow）
  - 范围：Mail.ReadWrite, Calendars.ReadWrite, Files.ReadWrite.All
  - Token 存储：加密存储在 `~/.zeroclaw/secrets/`

**JWT 认证：**
- Zhipu/GLM（`src/providers/zhipu.rs`）— HMAC-SHA256 签名
- 唤星云后端 — Bearer Token（`src/huanxing/api_client.rs`）

**API Key 认证：**
- 大部分 LLM 提供商 — `Authorization: Bearer <key>`
- 渠道平台 — 各自的 Token/Secret 机制

---

## 外部 API 集成

**Microsoft 365（`src/tools/microsoft365/`）：**
- Graph API 客户端（`graph_client.rs`）
- 功能：邮件、日历、OneDrive、Teams
- 认证：OAuth 2.0 设备码流

**Notion（`src/tools/notion_tool.rs`）：**
- Notion API v1
- 功能：页面读写、数据库查询
- 认证：Integration Token

**Web 搜索：**
- Tavily API（`src/tools/web_search.rs`）
- Exa API（可选）
- 自定义搜索引擎（可配置）

**短信服务（唤星专属）：**
- 阿里云短信 API
- 实现：`src/huanxing/api_client.rs` 中的 `send_sms` / `verify_sms`
- 用途：手机号验证、用户注册

**支付服务（唤星专属）：**
- 微信支付 API
- 实现：唤星云后端代理
- 用途：订阅购买、积分充值

---

## Webhook 集成

**入站 Webhook（`src/gateway/mod.rs`）：**
- 端点：`http://localhost:42620/` 或自定义端口
- 支持的平台：
  - WhatsApp Business API
  - Telegram Bot API
  - Discord Interactions
  - Slack Events API
  - Lark/飞书事件订阅
  - 自定义 Webhook

**签名验证：**
- HMAC-SHA256（WhatsApp, Slack, Lark）
- 实现：`src/gateway/mod.rs` 中的 `verify_whatsapp_signature` 等
- 防重放：幂等性检查（`src/gateway/mod.rs` 中的 `idempotency_check`）

**速率限制：**
- 滑动窗口算法
- 配置：`gateway.rate_limit_window_secs`, `gateway.rate_limit_max_requests`
- 实现：`src/gateway/mod.rs`

---

## 隧道服务

**支持的隧道类型（`src/tunnel/`）：**

| 类型 | 实现文件 | 用途 | 配置 |
|------|---------|------|------|
| ngrok | `ngrok.rs` | 快速公网暴露 | Auth Token |
| Cloudflare | `cloudflare.rs` | 企业级隧道 | Tunnel Token |
| Tailscale | `tailscale.rs` | 私有网络 | Auth Key |
| Pinggy | `pinggy.rs` | 免费隧道 | 无需注册 |
| OpenVPN | `openvpn.rs` | VPN 隧道 | 配置文件 |
| 自定义 | `custom.rs` | 自定义命令 | Shell 命令 |
| 无 | `none.rs` | 直接暴露 | 无 |

**用途：**
- 本地开发时接收 Webhook
- 内网部署时对外暴露服务
- 多地域部署时统一入口

---

## 可观测性集成

**Prometheus（可选）：**
- 库：prometheus 0.14
- 端点：`http://localhost:42620/metrics`
- 指标：请求计数、延迟、错误率、工具调用次数
- 实现：`src/observability/prometheus.rs`

**OpenTelemetry（可选）：**
- 库：opentelemetry 0.31
- 协议：OTLP（HTTP）
- 导出：Trace + Metrics
- 实现：`src/observability/otel.rs`
- 目标：Jaeger, Grafana Tempo, Honeycomb 等

**日志：**
- 框架：tracing + tracing-subscriber
- 格式：结构化 JSON 或人类可读
- 输出：stdout/stderr
- 配置：`RUST_LOG` 环境变量

---

## 文件存储

**本地文件系统：**
- 配置：`~/.zeroclaw/` 或 `~/.huanxing/`（桌面端）
- Agent 工作区：`~/.zeroclaw/agents/<name>/`
- 记忆：`~/.zeroclaw/memory.db`
- 秘密：`~/.zeroclaw/secrets/` (ChaCha20Poly1305 加密)

**云存储（通过工具）：**
- OneDrive（Microsoft 365 集成）
- Google Drive（可扩展）
- S3 兼容存储（可扩展）

---

## 硬件集成（可选）

**串口设备（`src/peripherals/`）：**
- STM32 微控制器
- Arduino
- 自定义串口设备

**GPIO（`src/peripherals/`）：**
- Raspberry Pi GPIO（sysfs）
- 其他 Linux GPIO

**机器人工具包（`crates/robot-kit/`）：**
- 驱动控制（电机、舵机）
- 传感器读取（距离、温度、陀螺仪）
- 视觉（摄像头）
- 听觉（麦克风）
- 语音（TTS）
- 情感表达（LED、屏幕）

---

## 唤星云后端集成（唤星专属）

**API 端点：** `https://api.huanxing.dcfuture.cn`

**功能模块：**

| 功能 | 端点 | 方法 | 认证 |
|------|------|------|------|
| 发送短信 | `/api/v1/sms/send` | POST | Agent Key |
| 验证短信 | `/api/v1/sms/verify` | POST | Agent Key |
| 用户注册 | `/api/v1/users/register` | POST | Agent Key |
| 获取用户信息 | `/api/v1/users/{phone}` | GET | Agent Key |
| 检查配额 | `/api/v1/users/{phone}/quota` | GET | Agent Key |
| 获取订阅 | `/api/v1/users/{phone}/subscription` | GET | Agent Key |
| 使用统计 | `/api/v1/users/{phone}/usage` | GET | Agent Key |
| Agent 同步 | `/api/v1/agents/sync` | POST | JWT Bearer |

**认证方式：**
- Agent Key：`X-Agent-Key` header（服务端 Agent 调用）
- JWT Bearer：`Authorization: Bearer <token>` header（桌面端用户调用）

**实现：** `src/huanxing/api_client.rs`

---

## 技能市场集成（可选）

**ClawHub（OpenClaw 官方）：**
- 端点：`https://clawhub.com/api/`
- 功能：技能搜索、安装、更新
- 实现：`src/integrations/registry.rs`

**唤星技能市场（唤星专属）：**
- 端点：`https://api.huanxing.dcfuture.cn/api/v1/skills/`
- 功能：技能同步、版本管理
- 实现：`src/huanxing/hub_sync.rs`

---

*集成分析：2026-03-21*
