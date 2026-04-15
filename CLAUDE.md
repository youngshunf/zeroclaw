# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 必读文档

动工前先读这三份，不要重复它们的内容：

1. **[`AGENTS.md`](./AGENTS.md)** — 上游 ZeroClaw 共享规范（命令、trait 扩展点、仓库结构、风险等级、反模式）。
2. **[`唤星开发规范.md`](./唤星开发规范.md)** — 唤星多租户抽象层 v2.0 完整规范（目录分层、代码修改规则、冲突预防）。
3. **父目录 [`../CLAUDE.md`](../CLAUDE.md)** — 唤星项目全景（后端/管理端/官网/HASN/服务器/部署）。

本文件只放 **zeroclaw 仓库特有** 且 AGENTS.md 没覆盖的增量信息。

---

## 常用命令（唤星 fork 补充）

上游命令见 `AGENTS.md`。唤星 fork 的关键区别：**大部分工作都要加 `--features huanxing`**。

```bash
# 编译
cargo build --bin zeroclaw                                # 纯上游构建（验证无 cfg 泄漏）
cargo build --bin zeroclaw --features huanxing            # 唤星功能（日常）
cargo build --release --bin zeroclaw --features huanxing  # 发版
cargo build --features "huanxing,channel-lark"            # 加渠道

# 检查
cargo clippy --all-targets --features huanxing -- -D warnings
cargo test --features huanxing
cargo test --features huanxing <test_name>                # 单测

# 独立 sidecar 调试（通常不需要手动启动——桌面端由 Tauri 管理 zeroclaw 子进程）
./target/debug/zeroclaw gateway -p 42620

# PR 前完整验证
./dev/ci.sh all
```

**唤星桌面端**（独立 Vite + Tauri，不依赖 `web/`）：

```bash
cd clients/desktop
pnpm install              # 独立 node_modules
pnpm dev                  # 1420 端口，proxy: /api/v1→8020(后端), /api→42620(sidecar)
pnpm tauri dev            # Tauri 拉起 zeroclaw 子进程作为 sidecar（端口 42620）
pnpm tauri build          # 打包桌面端
```

> ⚠️ `cargo build`（不带 feature）**必须也能通过**——这是唤星抽象层的硬性契约，用来检测 `cfg(feature = "huanxing")` 泄漏到核心模块。

---

## 架构要点（多租户抽象层）

```
Layer 4  clients/desktop/          纯唤星桌面端，零冲突
Layer 3  src/huanxing/*            唤星 Rust 扩展层（feature-gated）
Layer 2  src/channels/context_resolver.rs   MessageContextResolver trait（上游通用）
Layer 1  src/*                     ZeroClaw core（不改，与上游同步）
```

### 为什么要用 trait 注入

核心上游文件（`src/channels/mod.rs`、`src/gateway/ws.rs`、`src/tools/*.rs`）**禁止** 出现 `#[cfg(feature = "huanxing")]`。多租户行为通过 `MessageContextResolver` 和 task-local security 注入，否则每次上游同步都会有大量冲突。

**唯一允许 cfg 的上游文件**（且只能少量）：
- `src/config/schema.rs`（serde 字段声明）
- `src/main.rs` / `src/lib.rs`（1 行模块声明）
- `src/gateway/mod.rs`（`.merge(huanxing::router::huanxing_routes(...))`，一行）
- `src/daemon/mod.rs`（启动入口，一处）
- `src/channels/{napcat,lark,tts}.rs`（渠道特有逻辑，少量）

### 唤星扩展层关键模块（`src/huanxing/`）

| 模块 | 职责 |
|------|------|
| `config.rs` | `HuanXingConfig`（`[huanxing]` section） |
| `router.rs` | `TenantRouter`（多租户 axum 路由） |
| `tenant.rs` | `TenantContext`（每请求租户上下文） |
| `multi_tenant_resolver.rs` | `MessageContextResolver` 唤星实现（注入 model/memory/prompt） |
| `db.rs` / `TenantDb` | SQLite 租户数据库 |
| `permissions.rs` | 工具权限检查 |
| `tools.rs` / `skill_market_tools.rs` | 多租户管理工具 + task-local security |
| `api_agents.rs` / `api_sessions.rs` / `api_user_config.rs` | REST API |
| `hasn_*.rs` | HASN 协议集成（当前活跃开发区） |
| `channels/` / `channel_registry.rs` | 唤星专属渠道扩展 |

### 添加新功能的决策树

- **新业务逻辑** → 放 `src/huanxing/`，不碰上游。
- **要在消息处理链注入行为** → 扩展 `MessageContext` 字段，在 `MultiTenantResolver::resolve()` 填充，上游 `channels/mod.rs` 无需改动。
- **新工具** → 注册到 `src/huanxing/tools.rs`，权限走 `permissions.rs`，安全策略用 `get_active_security()` 读 task-local。
- **新 HTTP 路由** → 在 `src/huanxing/` 定义 `Router`，通过 `gateway/mod.rs` 的 `.merge()` 单行接入。
- **新 AppState 字段** → **不要** 改 `AppState`，用 `axum::Extension<HuanxingState>` 注入。

---

## 当前活跃开发

**HASN 协议集成**（`src/huanxing/hasn_*.rs`）正在演进。相关新文件（git 未追踪）：

- `hasn_agent_bridge.rs` — Agent 消息桥接
- `hasn_chat_api.rs` — HASN 聊天 REST API
- `hasn_chat_db.rs` — HASN 聊天存储
- `hasn_router.rs` — HASN 路由
- `hasn_sync.rs` — HASN 同步逻辑

修改 HASN 时确认相关 API 在 `router.rs` 里有注册，桌面端对应改动在 `clients/desktop/src/pages/hasn/` 和 `clients/desktop/src/lib/hasn-api.ts`。

---

## Git 分支（重要）

| 分支 | 用途 |
|------|------|
| `master` | 只跟上游 ZeroClaw 同步，**从不提交** |
| `huanxing` | 唤星主开发分支 |
| `huanxing-clean` | 上游合并的工作分支（当前分支） |
| `feature/*` | 功能分支 |

⚠️ `main` 分支已废弃。上游同步流程、冲突预期点、commit scope 规范见 `../CLAUDE.md` 的 "Git 工作流" 一节。

---

## 桌面端前端独立原则

`clients/desktop/` 与上游 `web/` 完全隔离：

- `vite` alias `@` 指向 `./src`（不是 `../../web/src`）
- 独立 `node_modules`、独立 `tsconfig paths`、Tailwind `@source "."`
- 需要复用 `web/` 组件时 **复制进来**，不要跨目录 import
- 桌面端新增 npm 依赖必须加在 `clients/desktop/package.json`

---

## 提交前自检

- [ ] `cargo build`（无 feature）通过 — 证明没有 cfg 泄漏
- [ ] `cargo build --features huanxing` 通过
- [ ] `cargo clippy --all-targets --features huanxing -- -D warnings` 通过
- [ ] 没有在 `channels/mod.rs` / `gateway/ws.rs` / `tools/*.rs` 里加 `#[cfg(feature = "huanxing")]`
- [ ] 新业务逻辑在 `src/huanxing/` 内
- [ ] 新路由通过 `gateway/mod.rs` 的 `.merge()` 接入
- [ ] 桌面端改动在 `clients/desktop/`，没有碰 `web/`
- [ ] commit 用中文 + conventional format（如 `feat(hasn): ...`）
