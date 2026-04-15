# 上游同步 2026-04-15 交接备忘

> 工作分支：`feature/upstream-sync-20260415`
> 备份分支：`backup/pre-upstream-sync-20260415`（保底，切勿删）
> 基线：上游 `upstream/master` @ `9f0de18b` merged into 唤星 `huanxing-clean` @ `f794c233`

---

## 已完成（Phase 0 – 4）

1. **Phase 0** 创建 backup + feature 分支
2. **Phase 1** `Cargo.toml` 手工合并（保留上游 workspace 拆分 + 唤星依赖），`Cargo.lock` 重生成
3. **Phase 2** 25 个 `src/*` 壳文件全部 `git checkout --theirs`（接受上游 `pub use zeroclaw_xxx::*;` re-export 壳）
4. **Phase 3** 9 个 `crates/zeroclaw-*` 内部 rename+modify 冲突 → 派 9 个并行 subagent 逐个 3-way 合并
5. **Phase 4 编译修复**
   - 新增 task-local 块 (ACTIVE_SECURITY / ACTIVE_WORKSPACE) 到 `crates/zeroclaw-runtime/src/tools/mod.rs`
   - 新增 task-local + `load_skills_cascaded` 到 `crates/zeroclaw-runtime/src/skills/mod.rs`
   - `crates/zeroclaw-runtime/src/cron/mod.rs` `mod schedule` → `pub mod schedule`
   - 修 `ReadSkillTool::new` 签名调用、`SopTool::new` 5 处调用、`HeartbeatTask` 和 `Sop` 构造字段、`consolidate_turn` 调用签名
   - `cargo check` (no features / default / --features huanxing) 全部通过
   - `cargo clippy --all-targets` 全部通过

**关键提交**：
- `af9aa09f` merge commit (主合并)
- `1c5c5a90` fix(merge): Phase 4 编译错误
- `58c99138` fix(merge): clippy + Sop 测试构造

---

## 🚨 当前遗留问题（Phase 5 要解决的）

**`src/huanxing/*` 整棵树 40+ 文件处于"磁盘上存在但不参与编译"的孤立状态。**

原因：`src/lib.rs` 被 `--theirs` 后，`pub mod huanxing;` 丢失。没有任何 crate 引用这些代码。

**代价**：HASN 运行时（`/api/v1/hasn/connect`、`/hasn/chat/*`、WS 消息路由、hasn_chat.db 持久化、多租户注入）全部下线。如果直接 ff 合并回 `huanxing-clean`/`huanxing`，HASN 功能会回退到"代码存在但永不被执行"的状态。

---

## Phase 5 实施计划（下次对话）

### 策略：把 `src/huanxing/*` 提取为独立 crate `crates/zeroclaw-huanxing/`

**为什么独立 crate 而不是散到 zeroclaw-runtime 等**：
- 遵循上游 RFC D1 workspace 拆分范式
- 唤星逻辑零污染上游 crate —— 上游每次 merge 唤星这边不用改它们的内部
- `#[cfg(feature = "huanxing")]` 也不会渗到核心 crate（唤星开发规范硬性要求）
- 这个 crate 只出现在根 `zeroclawlabs` 的 `[dependencies]` 下，可以整体开关
- 未来可独立发版/独立 CI

### 目标目录结构

```
crates/zeroclaw-huanxing/
├── Cargo.toml
└── src/
    ├── lib.rs                     # 顶层 re-exports + pub mod 声明
    ├── config.rs                  # HuanXingConfig（对齐旧 src/huanxing/config.rs）
    ├── tenant.rs                  # TenantContext
    ├── router.rs                  # TenantRouter（多租户 axum 路由）
    ├── multi_tenant_resolver.rs   # MessageContextResolver 实现
    ├── db.rs                      # TenantDb (users.db)
    ├── permissions.rs
    ├── tools/                     # 唤星专属工具
    │   ├── mod.rs
    │   ├── secret_tools.rs
    │   ├── skill_market_tools.rs
    │   ├── hx_image_gen.rs
    │   ├── hx_web_search.rs
    │   └── hx_ws.rs
    ├── api/                       # REST API
    │   ├── mod.rs
    │   ├── agents.rs              # api_agents.rs
    │   ├── sessions.rs            # api_sessions.rs
    │   ├── user_config.rs         # api_user_config.rs
    │   └── api_client.rs          # 后端 API 客户端
    ├── hasn/                      # HASN 协议集成（核心）
    │   ├── mod.rs
    │   ├── api.rs                 # hasn_api.rs
    │   ├── connector.rs           # hasn_connector.rs
    │   ├── router.rs              # hasn_router.rs（消息路由分发器）
    │   ├── chat_db.rs             # hasn_chat_db.rs（本地 IM 数据库）
    │   ├── chat_api.rs            # hasn_chat_api.rs（/chat/* REST API）
    │   ├── agent_bridge.rs        # hasn_agent_bridge.rs（来源标注 + 注入）
    │   ├── sync.rs                # hasn_sync.rs（云端-本地同步）
    │   ├── invoke.rs              # hasn_invoke.rs
    │   └── tools.rs               # hasn_tools.rs
    ├── channels/                  # 唤星渠道扩展
    │   ├── mod.rs
    │   └── registry.rs            # channel_registry.rs
    ├── bootstrap.rs
    ├── device_fingerprint.rs
    ├── register.rs
    ├── templates.rs
    ├── voice.rs
    ├── voice_hook.rs
    ├── tts_dashscope.rs
    ├── ws_observer.rs
    ├── sop_api.rs
    ├── doc_tools.rs
    ├── hub_sync.rs
    ├── tenant_heartbeat.rs
    ├── registry.rs
    └── agent_bridge.rs
```

### 依赖关系

```toml
# crates/zeroclaw-huanxing/Cargo.toml
[package]
name = "zeroclaw-huanxing"
version.workspace = true
edition.workspace = true

[dependencies]
zeroclaw-api.workspace = true
zeroclaw-infra.workspace = true
zeroclaw-config.workspace = true
zeroclaw-memory.workspace = true
zeroclaw-providers.workspace = true
zeroclaw-runtime.workspace = true    # 需要 Agent、TenantContext 等
zeroclaw-tools.workspace = true      # 需要 Tool trait
zeroclaw-gateway.workspace = true    # 需要 AppState
zeroclaw-channels.workspace = true   # 需要 MessageContext、session_backend

hasn-client-core = { path = "../hasn-client-core" }
huanxing-agent-factory = { path = "../huanxing-agent-factory" }

axum = { version = "0.8", features = [...] }
tokio = { version = "1.50", features = [...] }
serde = ...
serde_json = ...
rusqlite = ...
# etc
```

### 根 `Cargo.toml` 改动

```toml
[workspace.dependencies]
zeroclaw-huanxing = { path = "crates/zeroclaw-huanxing", version = "0.6.9" }

[dependencies]
zeroclaw-huanxing = { workspace = true, optional = true }

[features]
huanxing = ["dep:zeroclaw-huanxing", "dep:hasn-client-core"]
```

### `src/lib.rs` / `src/main.rs` / `src/gateway/mod.rs` 等挂载点

- `src/lib.rs` 加一行 `#[cfg(feature = "huanxing")] pub use zeroclaw_huanxing;`
- `src/gateway/mod.rs` 保持上游壳状态，但唤星路由 merge 在根 crate 的 gateway 启动函数里做（或者在 zeroclaw-huanxing 里 pub 一个 `huanxing_routes() -> axum::Router` 然后根 crate 调用）
- `src/daemon/mod.rs` 加唤星初始化调用（通过 feature gate）

### Phase 5 执行步骤

1. **创建 crate 骨架**：`cargo new --lib crates/zeroclaw-huanxing`，写 `Cargo.toml`
2. **批量搬文件**：把 `src/huanxing/*` 全部移到 `crates/zeroclaw-huanxing/src/`，按新目录分类重组（hasn/、api/、tools/ 等子模块）
3. **改 use 路径**（这是最大的工作量 —— 预计 40+ 文件，每个文件多处）：
   - `use crate::memory::*` → `use zeroclaw_memory::*`
   - `use crate::providers::*` → `use zeroclaw_providers::*`
   - `use crate::config::schema::*` → `use zeroclaw_config::schema::*`
   - `use crate::channels::session_backend::*` → `use zeroclaw_infra::session_backend::*`
   - `use crate::tools::*` → `use zeroclaw_tools::*` / `use zeroclaw_runtime::tools::*`
   - `use crate::agent::*` → `use zeroclaw_runtime::agent::*`
   - `use crate::gateway::AppState` → `use zeroclaw_gateway::AppState`
   - `use crate::huanxing::*` (内部交叉引用) → `use crate::*`
4. **修 struct/trait 名冲突**：比如 `Config` 可能需要显式 `zeroclaw_config::schema::Config`
5. **派并行 subagent**：按子模块分工 —— 5-7 个 agent，每个负责一个子模块的 use 路径迁移
   - Agent 1: `hasn/` 所有文件
   - Agent 2: `api/` 所有文件
   - Agent 3: `tools/` 所有文件
   - Agent 4: `channels/` 和 `db.rs` / `tenant.rs` / `router.rs`
   - Agent 5: `bootstrap.rs` / `register.rs` / `templates.rs` / 其它
6. **根 crate 挂载**：改 `src/lib.rs`、`src/main.rs`、`src/gateway/mod.rs`、`src/daemon/mod.rs` 几处启动挂钩
7. **编译迭代**：`cargo check --features huanxing` 暴露残留错误，主代理逐个修
8. **clippy + 规范校验**：确认无 `#[cfg(feature="huanxing")]` 泄漏到任何 `crates/zeroclaw-{runtime,channels,gateway,...}/` 内（是否需要用 `grep -rn 'cfg(feature = "huanxing")' crates/` 批量核查）
9. **桌面端冒烟（Phase 5.5）**：`pnpm tauri dev` 启动，登录 → HASN 连接 → 发消息验证
10. **Phase 6 合并回 `huanxing-clean` 和 `huanxing`**

### 预计工作量

- 创建 crate 骨架：10 分钟
- 批量搬文件 + 子目录重组：15 分钟
- use 路径迁移（5 个并行 agent）：30-60 分钟
- 根 crate 挂载：15 分钟
- 编译迭代：30-90 分钟（难以预估，取决于唤星代码对上游 API 的依赖深度）
- 冒烟：30 分钟

合计 2-4 小时深度工作。

---

## 已知需要同步处理的遗留问题

1. **`#[cfg(feature="huanxing")]` 泄漏清理**
   - `crates/zeroclaw-runtime/src/agent/agent.rs` 的 `set_observer`（agent 合并时带入）
   - `crates/zeroclaw-runtime/src/tools/read_skill.rs` 的 `derive_global_skills_dir` / `derive_user_skills_dir`
   - `crates/zeroclaw-runtime/src/tools/sop_*.rs` 的 `huanxing_api_base` 分支
   - `crates/zeroclaw-runtime/src/skills/mod.rs` 可能还有一处（见 Phase 3 read_skill agent 报告）

   **处理方式**：这些分支调用的唤星功能必须能通过 trait 或 hook 从 `zeroclaw-huanxing` 外部注入，而不是在 core crate 里 cfg 分支。可能需要在 core crate 里引入新的 trait 接口，然后 zeroclaw-huanxing 实现并注册。

2. **consolidate_turn workspace_dir 参数**
   当前 `zeroclaw-channels/orchestrator/mod.rs` 调用传 `None`。唤星多租户情况下需要从 TenantContext 注入真实 workspace。

3. **HASN_MESSAGING.md 模板文件**
   HASN 消息体系重构方案里提到每个 Agent 模板要加 `HASN_MESSAGING.md`，这个文件在 `huanxing-hub/templates/*/workspace/` 下；本次合并未涉及，但 Phase 5 搬代码时要确认模板路径解析仍然对齐。

4. **桌面端前端路径**
   `clients/desktop/src/lib/hasn-api.ts` 已改为调用 sidecar `/api/v1/hasn/chat/*`。这些 REST endpoint 必须在 Phase 5 挂载阶段通过 `zeroclaw_huanxing::api::router()` 合并到 gateway。

---

## 回退方案

如果 Phase 5 失败想回退：

```bash
# 放弃 feature 分支所有工作
git checkout huanxing-clean
git branch -D feature/upstream-sync-20260415
git push origin :feature/upstream-sync-20260415
# backup 分支仍然保底，可随时查
git log backup/pre-upstream-sync-20260415
```

`huanxing-clean` 和 `huanxing` 均未被污染（停在 `f794c233`），仍是可工作的生产分支。
