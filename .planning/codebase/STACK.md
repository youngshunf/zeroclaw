# 技术栈

**分析日期：** 2026-03-21

## 核心语言与运行时

**主语言：** Rust 1.92.0 (edition 2021)
- 最低支持版本：1.87
- 编译器：rustc 1.92.0 (ded5c06cf 2025-12-08)
- 包管理：Cargo + workspace（3 成员：主库、robot-kit、desktop/src-tauri）

**异步运行时：** Tokio 1.50
- 特性：rt-multi-thread, macros, time, net, io-util, sync, process, io-std, fs, signal
- 辅助库：tokio-util 0.7, tokio-stream 0.1.18

**前端运行时（桌面端）：**
- Node.js：v22.22.0
- TypeScript：~5.7.2
- React：19.0.0
- Vite：6.0.7
- Tauri：2.10.1

---

## 核心依赖

**CLI 框架：**
- clap 4.5 (derive feature) — 命令行参数解析
- clap_complete 4.5 — shell 补全生成

**HTTP 客户端：**
- reqwest 0.12 (rustls-tls, json, blocking, multipart, stream, socks)
- flate2 1 — gzip 压缩
- tar 0.4 — tar 归档

**HTTP 服务器（网关）：**
- axum 0.8 (http1, json, tokio, query, ws, macros)
- tower 0.5 — 中间件层
- tower-http 0.6 (limit, timeout) — 速率限制、超时
- http-body-util 0.1

**序列化：**
- serde 1.0 (derive)
- serde_json 1.0
- serde_yaml 0.9
- toml 1.0 — 配置文件格式
- schemars 1.2 — JSON Schema 生成

**数据库：**
- rusqlite 0.37 (bundled) — SQLite（主要记忆后端）
- postgres 0.19 (with-chrono-0_4, optional) — PostgreSQL 后端

**时间处理：**
- chrono 0.4 (clock, std, serde)
- chrono-tz 0.10 — 时区支持
- cron 0.15 — cron 表达式解析

**加密与安全：**
- chacha20poly1305 0.10 — AEAD 加密（秘密存储）
- hmac 0.12 + sha2 0.10 — HMAC-SHA256 签名验证
- ring 0.17 — HMAC-SHA256（Zhipu/GLM JWT）
- hex 0.4 — 十六进制编解码
- rand 0.10 — CSPRNG

**WebSocket：**
- tokio-tungstenite 0.29 (rustls-tls-webpki-roots)
- futures-util 0.3 (sink)

**日志与可观测性：**
- tracing 0.1 — 结构化日志
- tracing-subscriber 0.3 (fmt, ansi, env-filter)
- prometheus 0.14 (optional) — Prometheus 指标
- opentelemetry 0.31 (trace, metrics, optional) — OTLP 导出

**并发原语：**
- parking_lot 0.12 — 快速 Mutex（不 poison）
- async-trait 0.1 — 异步 trait
- portable-atomic 1 — 32 位平台原子操作回退

**其他核心库：**
- anyhow 1.0 — 错误处理
- thiserror 2.0 — 自定义错误类型
- uuid 1.22 (v4, std) — UUID 生成
- base64 0.22 — Base64 编解码
- urlencoding 2.1 — URL 编码
- regex 1.10 — 正则表达式

---

## 渠道集成

**消息平台客户端：**
- matrix-sdk 0.16 (e2e-encryption, rustls-tls, markdown, sqlite, optional) — Matrix
- nostr-sdk 0.44 (nip04, nip59, optional) — Nostr
- prost 0.14 (derive, optional) — Protobuf（Lark WS、WhatsApp 存储）

**邮件：**
- lettre 0.11.19 (builder, smtp-transport, rustls-tls) — SMTP 发送
- mail-parser 0.11.2 — 邮件解析
- async-imap 0.11 (runtime-tokio) — IMAP 接收

---

## 前端技术栈（桌面端）

**框架与库：**
- React 19.0.0 + react-dom 19.0.0
- react-router-dom 7.1.1 — 路由
- Tauri 2.10.1 — 桌面应用框架

**UI 组件：**
- @chatscope/chat-ui-kit-react 2.1.1 — 聊天 UI
- lucide-react 0.468.0 — 图标库
- react-easy-crop 5.5.6 — 图片裁剪

**代码编辑器：**
- @uiw/react-codemirror 4.25.5
- @codemirror/language 6.12.2
- @codemirror/theme-one-dark 6.1.3

**Markdown 渲染：**
- react-markdown 10.1.0
- remark-gfm 4.0.1 — GitHub Flavored Markdown
- rehype-raw 7.0.0 — 原始 HTML 支持
- shiki 4.0.2 — 语法高亮

**样式：**
- Tailwind CSS 4.0.0
- @tailwindcss/vite 4.0.0
- tailwind-merge 3.5.0 — 类名合并
- clsx 2.1.1 — 条件类名

**构建工具：**
- Vite 6.0.7
- @vitejs/plugin-react 4.3.4
- TypeScript 5.7.2

**Tauri 插件：**
- @tauri-apps/plugin-notification 2.2.0
- @tauri-apps/plugin-process 2.2.0
- @tauri-apps/plugin-shell 2.2.0

---

## 开发工具

**测试：**
- 内置 Rust 测试框架（`cargo test`）
- tokio::test 宏（异步测试）
- tempfile 3.26 — 临时文件/目录
- wiremock 0.6 — HTTP mock 服务器
- scopeguard 1.2 — 测试清理

**性能基准：**
- criterion 0.8 (async_tokio) — benchmark 框架

**代码质量：**
- rustfmt — 代码格式化（配置：rustfmt.toml）
- clippy — lint 工具（配置：clippy.toml）

**CI/CD：**
- GitHub Actions（`.github/workflows/`）
- Docker（Dockerfile, Dockerfile.debian）

---

## 可选特性（Feature Flags）

**主要 features（Cargo.toml）：**
- `huanxing` — 唤星多租户扩展（默认关闭）
- `channel-lark` — 飞书/Lark 渠道
- `channel-matrix` — Matrix 渠道
- `channel-nostr` — Nostr 渠道
- `channel-whatsapp` — WhatsApp 渠道
- `postgres` — PostgreSQL 记忆后端
- `prometheus` — Prometheus 指标导出
- `opentelemetry` — OTLP trace/metrics
- `fantoccini` — Rust 原生浏览器自动化

**编译示例：**
```bash
cargo build --features huanxing                      # 唤星功能
cargo build --features "huanxing,channel-lark"       # 唤星 + 飞书
cargo build --release --features huanxing            # 生产版本
```

---

## 配置文件格式

**主配置：** TOML
- 位置：`~/.zeroclaw/config.toml` 或 `~/.huanxing/config.toml`（桌面端）
- Schema：`src/config/schema.rs` 中的 `Config` 结构体
- 环境变量覆盖：`ZEROCLAW_*` 前缀

**前端配置：** TypeScript 常量
- 位置：`clients/desktop/src/huanxing/config.ts`
- 端点配置：`HUANXING_CONFIG`

---

## 外部服务依赖

**LLM 提供商（15+ 支持）：**
- OpenAI API
- Anthropic API
- Google Gemini
- Ollama（本地）
- OpenAI 兼容端点（通用）
- 其他：Zhipu/GLM, Groq, Mistral, Cohere, Perplexity 等

**向量数据库（可选）：**
- Qdrant（HTTP REST API）
- Mem0（HTTP API）

**可观测性（可选）：**
- Prometheus（指标拉取）
- OpenTelemetry Collector（OTLP 推送）

**隧道服务（可选）：**
- ngrok
- Cloudflare Tunnel
- Tailscale
- Pinggy
- OpenVPN
- 自定义隧道

**唤星云后端（唤星专属）：**
- API 端点：`https://api.huanxing.dcfuture.cn`
- 认证：JWT Bearer Token
- 功能：用户注册、订阅管理、短信验证、Agent 同步

---

## 硬件支持（可选）

**外设抽象（`src/peripherals/`）：**
- STM32 微控制器（串口通信）
- Raspberry Pi GPIO（sysfs）

**机器人工具包（`crates/robot-kit/`）：**
- 驱动、视觉、听觉、语音、传感、情感表达工具

**固件（`firmware/`）：**
- ESP32（Rust + esp-idf）
- STM32 Nucleo（embedded-hal）

---

## 部署环境

**支持平台：**
- Linux（主要生产环境）
- macOS（开发环境）
- Windows（部分支持）

**容器化：**
- Docker（Alpine 基础镜像，最小体积）
- Docker（Debian 基础镜像，兼容性优先）

**系统服务：**
- systemd（Linux）
- OpenRC（Alpine Linux）

---

*技术栈分析：2026-03-21*
