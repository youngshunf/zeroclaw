# 架构文档

**分析日期：** 2026-03-21

## 模式概述

**总体模式：** 三层 Feature-Gated 插件架构，上层为唤星多租户扩展，下层为 ZeroClaw 自主 Agent 运行时

**核心特征：**
- Trait 驱动的模块化扩展点（Provider / Channel / Tool / Memory / Observer / Runtime / Peripheral）
- Feature Flag (`huanxing`) 完全隔离唤星扩展，ZeroClaw 核心可独立编译
- Axum HTTP 网关统一处理 Webhook、WebSocket、REST API
- 多租户路由层：(channel, sender_id) → TenantContext → 独立 Agent 循环
- Tokio 异步运行时，Arc + RwLock 共享状态

---

## 层次结构

**Layer 3 — 唤星桌面端 (`clients/desktop/`):**
- 用途：用户面向的桌面 GUI，Tauri + React 18 + TypeScript
- 位置：`clients/desktop/src/`（前端）、`clients/desktop/src-tauri/src/`（Tauri 后端）
- 包含：登录页面、Agent 管理页面、HASN 即时消息、Sidecar 生命周期管理
- 依赖于：ZeroClaw Sidecar（HTTP REST）、唤星云后端（`https://api.huanxing.dcfuture.cn`）
- 被使用：最终用户

**Layer 2 — 唤星扩展层 (`src/huanxing/`):**
- 用途：多租户路由、API 集成、唤星特有工具
- 位置：`src/huanxing/`
- 包含：`TenantRouter`、`TenantContext`、`TenantDb`（SQLite）、`ApiClient`、Agent CRUD API、会话 API、HASN 工具
- 依赖于：ZeroClaw 核心层（provider/memory/channels/tools）
- 被使用：桌面端通过 HTTP API；NapCat/Feishu 等渠道通过 TenantRouter 路由

**Layer 1 — ZeroClaw 核心 (`src/*` 非 `huanxing`):**
- 用途：通用 Agent 运行时基础设施
- 位置：`src/agent/`、`src/gateway/`、`src/providers/`、`src/channels/`、`src/tools/`、`src/memory/`、`src/security/`
- 包含：Agent 编排循环、HTTP 网关、多 Provider 抽象、20+ 渠道、50+ 工具、记忆后端
- 依赖于：外部 LLM API、SQLite/Postgres 数据库
- 被使用：唤星扩展层；CLI 命令

---

## 关键抽象（Trait 扩展点）

**Provider（LLM 提供商）：**
- Trait：`src/providers/traits.rs` 中的 `Provider`
- 核心方法：`chat(request) → Stream<ChatDelta>`
- 实现：`src/providers/openai.rs`、`anthropic.rs`、`compatible.rs`、`gemini.rs`、`ollama.rs` 等 15+ 个
- 弹性包装器：`src/providers/reliable.rs`（重试、熔断）

**Channel（消息渠道）：**
- Trait：`src/channels/traits.rs` 中的 `Channel`
- 核心消息类型：`ChannelMessage { id, sender, reply_target, content, channel, timestamp }`
- 实现：`telegram.rs`、`discord.rs`、`slack.rs`、`napcat.rs`（唤星专属，feature-gated）等 20+ 个
- 会话持久化：`session_backend.rs` / `session_sqlite.rs`

**Tool（工具）：**
- Trait：`src/tools/traits.rs` 中的 `Tool`
- 核心方法：`execute(args: serde_json::Value) → ToolResult`
- 实现：`src/tools/` 下 50+ 个工具（shell, file, web, memory, cron, mcp 等）
- 唤星专有工具：`src/huanxing/tools.rs`（`hx_register_user`、`hx_invalidate_cache` 等）

**Memory（记忆系统）：**
- Trait：`src/memory/traits.rs` 中的 `Memory`
- 分类：`MemoryCategory::Core`（长期）/ `Daily`（每日日志）/ `Conversation`（上下文）/ `Custom`
- 实现：`src/memory/sqlite.rs`、`markdown.rs`、`postgres.rs`、`qdrant.rs`（向量）、`mem0.rs`

**Observer（可观测性）：**
- Trait：`src/observability/traits.rs` 中的 `Observer`
- 事件：`AgentStart`、`LlmRequest`、`LlmResponse`、`ToolCallStart`、`ToolCall`、`AgentEnd`

**RuntimeAdapter（运行时适配）：**
- Trait：`src/runtime/traits.rs`
- 方法：`has_shell_access()`、`has_filesystem_access()`、`supports_long_running()`
- 实现：`src/runtime/native.rs`（生产）、`src/runtime/wasm.rs`（WebAssembly 沙箱）

---

## 核心数据流

**渠道消息处理流程（单租户）：**

1. 外部平台（Telegram/Discord/QQ 等）推送消息
2. `Channel::listen()` 接收 → 生成 `ChannelMessage { sender, content }`
3. `channels/mod.rs` 的 `start_channels()` 调度到 Agent 处理函数
4. `agent/loop_.rs::process_message()` 构建 ChatRequest（系统提示 + 历史 + 用户消息）
5. `providers/` 的 Provider 调用 LLM API，流式接收 token
6. Agent 循环检测 ToolCall → 调用 `tools/` 执行工具 → 追加 ToolResult
7. 最终文本响应通过 `Channel::send()` 发回用户
8. 对话历史写入 `memory/`（SQLite 或 Markdown）

**唤星多租户路由流程：**

1. 渠道消息到达 `TenantRouter::resolve(channel, sender_id)`
2. 检查内存缓存（`RwLock<HashMap<cache_key, Arc<TenantContext>>>`）
3. 缓存未命中 → 查询 SQLite (`data/users.db`) 的 `users` 和 `channels` 表
4. 找到记录 → 加载 `TenantContext`（SOUL.md + USER.md + 记忆 + 安全策略）
5. 未找到记录 → 回退到 Guardian 上下文
6. 管理员渠道（如 Feishu）→ 直接路由到 Admin 上下文
7. 得到 `TenantContext` → 执行独立的 Agent 循环

**Webhook/HTTP 触发流程：**

1. 外部 POST `http://localhost:42620/` 或 WebSocket 连接
2. `gateway/mod.rs` 的 Axum 路由器接收（64KB body 限制、30s 超时）
3. 速率限制 + 幂等性检查
4. 触发 `process_message()` 进入 Agent 循环
5. SSE 或 WebSocket 流式推送响应

**桌面端 Sidecar 流程：**

1. 用户在桌面端登录 → 唤星云后端返回 `llm_token` + `gateway_token`
2. 桌面端 Tauri 触发 `onboard_zeroclaw` 命令
3. `SidecarManager::start()` 写入配置到 `~/.huanxing/config.toml`，启动 zeroclaw 子进程（端口 42620）
4. 前端通过 HTTP `localhost:42620` 与 Sidecar 通信
5. App 退出时 Sidecar 后台常驻；再次打开时 `adopt_existing()` 检测并复用

---

## 安全层

**策略：`src/security/policy.rs`**
- `AutonomyLevel::ReadOnly` / `Supervised` / `Full`
- `SecurityPolicy` 按租户隔离（唤星中每个 TenantContext 有独立策略）
- 工具操作分类：`ToolOperation::Read` / `Act`
- 滑动窗口速率限制：`ActionTracker`

**配对机制：`src/security/pairing.rs`**
- 配对码验证（`constant_time_eq` 防时序攻击）
- 可通过 `require_pairing = false` 关闭（桌面端场景）

**其他安全模块：**
- `src/security/secrets.rs` — 加密秘密存储（ChaCha20Poly1305）
- `src/security/workspace_boundary.rs` — 工作区边界限制
- `src/security/prompt_guard.rs` — Prompt 注入防护
- `src/security/audit.rs` — 审计日志

---

## 错误处理策略

**方式：** 全链路 `anyhow::Result`，在渠道层降级（fallback to guardian）

**模式：**
- 提供商故障 → `reliable.rs` 重试 + 熔断，不暴露原始错误给用户
- 租户 DB 查询失败 → `tracing::warn!` 日志 + 回退到 Guardian 上下文
- Sidecar 启动失败 → Tauri 事件 `sidecar://status-changed` 通知前端
- 工具执行失败 → `ToolResult { success: false, error: Some(...) }` 返回 LLM 继续推理

---

## 跨切面关注点

**日志：** `tracing` crate，`tracing-subscriber` 格式化，支持 `RUST_LOG` 环境变量
**配置：** TOML 文件（`~/.zeroclaw/config.toml` 或 `~/.huanxing/config.toml`），`ZEROCLAW_*` 环境变量覆盖
**认证：** 桌面端 JWT Bearer Token（`Authorization` header）；Agent 服务端 `X-Agent-Key` header
**多并发：** Tokio `rt-multi-thread`；通道消息并发通过 `Semaphore` 控制

---

*架构分析：2026-03-21*
