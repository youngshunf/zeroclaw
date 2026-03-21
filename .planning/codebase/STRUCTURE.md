# 代码库目录结构

**分析日期：** 2026-03-21

## 目录总览

```
huanxing-zeroclaw/
├── src/                        # Rust 核心源码（主库 + 二进制）
│   ├── main.rs                 # CLI 入口，clap 命令路由
│   ├── lib.rs                  # 库入口，模块导出，CLI 枚举定义
│   ├── agent/                  # Agent 编排循环
│   ├── gateway/                # HTTP/WebSocket 网关（Axum）
│   ├── config/                 # 配置加载与 Schema
│   ├── providers/              # LLM 提供商抽象层
│   ├── channels/               # 消息渠道（Telegram/Discord/QQ 等）
│   ├── tools/                  # 工具执行面（50+ 工具）
│   ├── memory/                 # 记忆后端（SQLite/Markdown/向量）
│   ├── security/               # 安全策略、配对、秘密存储
│   ├── observability/          # 可观测性 trait + 实现
│   ├── runtime/                # 运行时适配器（native/wasm）
│   ├── peripherals/            # 硬件外设（STM32/RPi GPIO）
│   ├── nodes/                  # 分布式节点支持
│   ├── hands/                  # 硬件操控抽象
│   ├── rag/                    # RAG（检索增强生成）
│   ├── skills/                 # 技能系统
│   ├── hooks/                  # 生命周期钩子
│   ├── cron/                   # 定时任务
│   ├── tunnel/                 # 隧道（内网穿透）
│   ├── huanxing/               # 唤星多租户扩展层（feature-gated）
│   └── ...                     # auth, cost, daemon, doctor, health 等
├── clients/
│   ├── desktop/                # 唤星桌面端（Tauri + React + TypeScript）
│   │   ├── src/                # 前端 React 源码
│   │   │   ├── huanxing/       # 唤星专属前端模块
│   │   │   ├── components/     # 共享 UI 组件
│   │   │   ├── pages/          # 标准 ZeroClaw Web 页面（已废弃）
│   │   │   ├── hooks/          # React 自定义 Hook
│   │   │   ├── lib/            # 工具库（api/auth/ws/sse 等）
│   │   │   ├── types/          # TypeScript 类型定义
│   │   │   └── App.tsx         # React 应用入口
│   │   └── src-tauri/          # Tauri Rust 后端
│   │       └── src/
│   │           ├── lib.rs      # Tauri 命令注册 + Sidecar 启动
│   │           ├── sidecar.rs  # SidecarManager（进程生命周期）
│   │           ├── main.rs     # Tauri 应用入口
│   │           └── commands/   # Tauri IPC 命令（auth/hasn/zeroclaw）
│   └── android/                # Android 客户端（Java/Kotlin）
├── crates/
│   ├── hasn-client-core/       # HASN 协议客户端库（独立 crate）
│   └── robot-kit/              # 机器人工具包
├── web/                        # ZeroClaw 原版 Web UI（已废弃，桌面端用 clients/desktop/）
│   └── src/                    # React 源码（无唤星目录）
├── templates/                  # Agent 工作区模板
│   ├── default/                # 默认模板
│   ├── huanxing/               # 唤星专属模板
│   │   ├── _base/              # 基础模板（服务器端）
│   │   ├── _base_desktop/      # 基础模板（桌面端）
│   │   ├── assistant/          # 助手模板
│   │   ├── finance/            # 财务模板
│   │   ├── health/             # 健康模板
│   │   └── ...                 # 其他业务模板
│   ├── finance/                # 财务 Agent 模板
│   ├── guardian/               # 守护者 Agent 模板
│   └── ...                     # go/python/rust/typescript 等技术模板
├── server-config/              # 生产服务器配置参考
│   ├── config.toml             # 服务器级 ZeroClaw 配置
│   ├── agents/                 # Agent 工作区目录（生产）
│   ├── guardian/               # Guardian Agent 工作区
│   ├── common-skills/          # 共享技能目录
│   └── data/                   # 数据库文件（users.db）
├── docs/                       # 技术文档（主题式）
├── tests/                      # 集成与系统测试
│   ├── integration/            # 集成测试
│   ├── system/                 # 系统测试
│   ├── component/              # 组件测试
│   ├── live/                   # 在线测试
│   └── manual/                 # 手动测试用例
├── benches/                    # 性能基准测试
├── firmware/                   # 硬件固件（STM32 等）
├── example-plugin/             # 插件示例
├── extensions/                 # 扩展模块
├── python/                     # Python 绑定/工具
├── scripts/                    # 开发辅助脚本
├── dev/                        # 开发工具（ci.sh 等）
├── deploy/                     # 部署产物目录
├── tool_descriptions/          # 工具描述 JSON 文件
├── Cargo.toml                  # Rust 工作区清单
├── Cargo.lock                  # 锁定文件
├── build.rs                    # 构建脚本（嵌入 web dist 等）
├── Dockerfile                  # Docker 镜像（Alpine）
├── Dockerfile.debian           # Docker 镜像（Debian）
└── install.sh                  # 一键安装脚本
```

---

## 核心目录详解

**`src/agent/`：**
- 用途：Agent 编排主循环
- 关键文件：`loop_.rs`（`process_message()` / `run()` 入口）、`agent.rs`（`Agent` / `AgentBuilder`）、`prompt.rs`（系统提示构建）、`dispatcher.rs`（工具调度）、`classifier.rs`（意图分类）、`memory_loader.rs`（记忆加载）

**`src/gateway/`：**
- 用途：Axum HTTP/WebSocket 服务，接受 Webhook 和 WS 连接
- 关键文件：`mod.rs`（`AppState`、路由注册、速率限制）、`api.rs`（REST API）、`ws.rs`（WebSocket handler）、`sse.rs`（SSE 流式推送）、`api_pairing.rs`（配对 API）
- 注：唤星路由通过 `app.merge(huanxing_routes())` 注入，不在此文件直接注册

**`src/config/`：**
- 用途：配置 Schema 定义与加载
- 关键文件：`schema.rs`（`Config` 顶层结构，包含 `#[cfg(feature = "huanxing")] huanxing: HuanXingConfig`）、`traits.rs`（`ChannelConfig` trait）、`workspace.rs`（工作区路径解析）

**`src/providers/`：**
- 用途：LLM 提供商统一抽象
- 关键文件：`traits.rs`（`Provider` trait、`ChatMessage`、`ToolCall`、`TokenUsage`）、`reliable.rs`（弹性包装，重试/熔断）、`router.rs`（动态路由）、每个提供商一个文件（openai/anthropic/gemini/ollama/compatible 等）

**`src/channels/`：**
- 用途：消息渠道统一抽象
- 关键文件：`traits.rs`（`Channel` trait、`ChannelMessage`、`SendMessage`）、`session_backend.rs`（会话持久化接口）、`session_sqlite.rs`（SQLite 实现）；NapCat 渠道（`napcat.rs`）通过 `#[cfg(feature = "huanxing")]` 控制

**`src/tools/`：**
- 用途：工具执行面（Agent 可调用的所有能力）
- 关键文件：`traits.rs`（`Tool` trait、`ToolSpec`、`ToolResult`）、`schema.rs`（工具注册中心）、`mod.rs`（工具工厂函数）；每个工具一个文件

**`src/memory/`：**
- 用途：对话记忆与知识库
- 关键文件：`traits.rs`（`Memory` trait、`MemoryEntry`、`MemoryCategory`）、`sqlite.rs`（主要后端）、`markdown.rs`（文本后端）、`vector.rs` + `embeddings.rs`（向量检索）

**`src/security/`：**
- 用途：访问控制、安全策略、秘密管理
- 关键文件：`policy.rs`（`SecurityPolicy`、`AutonomyLevel`、`ActionTracker`）、`pairing.rs`（配对验证）、`secrets.rs`（ChaCha20Poly1305 加密存储）、`workspace_boundary.rs`（路径限制）、`prompt_guard.rs`（Prompt 注入防护）

**`src/huanxing/`：**
- 用途：唤星多租户扩展（仅 `--features huanxing` 编译）
- 关键文件：
  - `mod.rs` — 模块入口，架构注释
  - `config.rs` — `HuanXingConfig`（`[huanxing]` 配置节）
  - `router.rs` — `TenantRouter`（多租户消息路由，缓存 + SQLite）
  - `tenant.rs` — `TenantContext`（每租户 Agent 上下文）
  - `db.rs` — `TenantDb`（SQLite 用户数据库操作）
  - `api_agents.rs` — Agent CRUD HTTP API（`GET/POST/DELETE /api/agents`）
  - `api_sessions.rs` — 会话 HTTP API
  - `api_client.rs` — 唤星云后端 API 客户端
  - `tools.rs` — 唤星专有工具（`hx_register_user` 等）
  - `templates.rs` — Agent 工作区模板引擎
  - `permissions.rs` — 用户权限检查
  - `hub_sync.rs` — 技能市场同步
  - `tenant_heartbeat.rs` — 多租户定时心跳

**`clients/desktop/src/huanxing/`：**
- 用途：唤星桌面端专属前端模块
- 关键文件：
  - `pages/Login.tsx` — 登录页（手机号+验证码）
  - `pages/AgentManager.tsx` — Agent 管理
  - `pages/HasnChat.tsx` — HASN 即时消息
  - `pages/Engine.tsx` — 引擎（Sidecar 状态）
  - `config.ts` — 端点配置、会话管理（`HUANXING_CONFIG`）
  - `auth.ts` — 认证逻辑
  - `api.ts` — 云后端 API 调用
  - `session-manager.ts` — 会话状态管理
  - `ws.ts` / `sse.ts` — WebSocket/SSE 客户端
  - `i18n.ts` — 国际化

**`clients/desktop/src-tauri/src/`：**
- 用途：Tauri 原生层，IPC 命令 + Sidecar 进程管理
- 关键文件：`lib.rs`（`run()` 入口，命令注册）、`sidecar.rs`（`SidecarManager`，管理 zeroclaw 子进程，端口 42620）、`commands/`（`auth.rs` / `hasn.rs` / `zeroclaw.rs` 命令分组）

---

## 关键文件位置

**二进制入口：**
- `src/main.rs` — zeroclaw CLI 主入口
- `clients/desktop/src-tauri/src/main.rs` — 桌面应用入口

**配置相关：**
- `src/config/schema.rs` — `Config` 完整 Schema（TOML 对应结构）
- `src/huanxing/config.rs` — `HuanXingConfig`（`[huanxing]` 配置节）
- `clients/desktop/src/huanxing/config.ts` — 前端端点常量
- `server-config/config.toml` — 生产服务器配置参考

**核心业务逻辑：**
- `src/agent/loop_.rs` — Agent 主循环（`process_message`）
- `src/huanxing/router.rs` — 多租户路由（TenantRouter）
- `src/huanxing/tenant.rs` — 租户上下文（TenantContext）
- `clients/desktop/src-tauri/src/sidecar.rs` — Sidecar 生命周期管理

**Trait 定义（扩展点）：**
- `src/providers/traits.rs` — `Provider` trait
- `src/channels/traits.rs` — `Channel` trait、`ChannelMessage`
- `src/tools/traits.rs` — `Tool` trait
- `src/memory/traits.rs` — `Memory` trait
- `src/observability/traits.rs` — `Observer` trait
- `src/runtime/traits.rs` — `RuntimeAdapter` trait
- `src/peripherals/traits.rs` — `Peripheral` trait

**测试：**
- `tests/integration/` — 集成测试
- `tests/system/` — 系统测试
- `src/agent/tests.rs` — Agent 单元测试
- `clients/desktop/src/test/` — 前端测试

---

## 命名规范

**Rust 文件：**
- 模块目录：小写下划线（`agent/`、`huanxing/`）
- 文件名：小写下划线（`loop_.rs`、`api_agents.rs`）
- 包含平台关键字的文件加前缀：`api_pairing.rs`、`api_plugins.rs`

**Rust 结构体/类型：**
- 结构体：`PascalCase`（`TenantContext`、`HuanXingConfig`）
- Trait：`PascalCase`（`Provider`、`Channel`、`Tool`）
- 枚举变体：`PascalCase`（`AutonomyLevel::Supervised`）

**TypeScript 文件：**
- 组件：`PascalCase`（`Login.tsx`、`AgentManager.tsx`）
- 工具/配置：`camelCase`（`config.ts`、`auth.ts`、`session-manager.ts`）
- 类型定义：`PascalCase` 接口（`HuanxingLoginData`、`HuanxingSession`）

---

## 新代码放置指引

**新唤星后端功能（Rust）：**
- 业务逻辑：`src/huanxing/` 下新建文件（如 `src/huanxing/my_feature.rs`）
- 在 `src/huanxing/mod.rs` 中 `pub mod my_feature;`
- HTTP 路由：在 `src/huanxing/router.rs` 中注册（`Router::new().route(...)`）
- 新工具：`src/huanxing/tools.rs` 或新建 `src/huanxing/my_tools.rs`

**新唤星前端页面：**
- 页面文件：`clients/desktop/src/huanxing/pages/MyPage.tsx`
- 路由注册：更新 `clients/desktop/src/App.tsx`
- API 调用：`clients/desktop/src/huanxing/api.ts`

**新 Tauri IPC 命令：**
- 命令实现：`clients/desktop/src-tauri/src/commands/` 下对应文件
- 注册：在 `clients/desktop/src-tauri/src/lib.rs` 的 `invoke_handler!` 中添加

**新 LLM 提供商：**
- 实现文件：`src/providers/my_provider.rs`（实现 `Provider` trait）
- 注册：`src/providers/mod.rs` 工厂函数
- 参考：`src/providers/compatible.rs`（OpenAI 兼容格式最简实现）

**新消息渠道：**
- 实现文件：`src/channels/my_channel.rs`（实现 `Channel` trait）
- 注册：`src/channels/mod.rs`（`pub mod`、`pub use`）
- 注意唤星专有渠道需要 `#[cfg(feature = "huanxing")]`

**新 Agent 工具：**
- 实现文件：`src/tools/my_tool.rs`（实现 `Tool` trait）
- 注册：`src/tools/schema.rs` 或 `src/tools/mod.rs`

**工作区模板：**
- 通用模板：`templates/huanxing/` 下新建目录
- 模板文件：`SOUL.md`（系统提示）+ `config.toml`（可选覆盖）

**测试：**
- 单元测试：与源文件同目录（`src/huanxing/my_feature.rs` 末尾 `#[cfg(test)] mod tests { ... }`）
- 集成测试：`tests/integration/`

---

## 特殊目录说明

**`.planning/`：**
- 用途：GSD 规划文档
- 包含：`codebase/`（代码库分析文档）、`phases/`（实施计划）
- 是否生成：手动维护
- 是否提交：是

**`target/`：**
- 用途：Cargo 编译产物
- 是否生成：是
- 是否提交：否（`.gitignore`）

**`server-config/`：**
- 用途：生产服务器配置参考，包含实际 Agent 工作区目录结构
- 包含：`config.toml`（服务器配置）、`agents/`（Agent 工作区）、`guardian/`（Guardian）、`data/users.db`
- 是否提交：是（敏感字段用占位符）

**`templates/huanxing/_base_desktop/`：**
- 用途：桌面端用户的 Agent 工作区基础模板（新增，未提交到上游）
- 与 `_base/` 的区别：针对单用户桌面场景调整的系统提示

**`tool_descriptions/`：**
- 用途：工具描述 JSON（i18n 多语言工具说明）
- 是否生成：部分生成
- 是否提交：是

---

*结构分析：2026-03-21*
