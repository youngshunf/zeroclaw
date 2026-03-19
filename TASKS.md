# ZeroClaw 多租户改造 — 任务列表

> 自动执行，完成一个 check 一个，直到全部完成。

## Phase 1: 核心路由层（已完成）
- [x] `src/huanxing/mod.rs` — 模块入口
- [x] `src/huanxing/config.rs` — HuanXingConfig 配置
- [x] `src/huanxing/db.rs` — SQLite 查询 + 注册
- [x] `src/huanxing/tenant.rs` — TenantContext（SOUL.md 加载）
- [x] `src/huanxing/router.rs` — TenantRouter（缓存 → DB → Guardian）
- [x] `src/huanxing/tools.rs` — 注册工具（lookup, register, invalidate_cache）
- [x] `src/channels/mod.rs` — 路由集成 + system_prompt 切换
- [x] `src/channels/mod.rs` — model/provider per-tenant 覆盖
- [x] `src/config/schema.rs` — Config 加 huanxing 字段
- [x] `src/lib.rs` + `src/main.rs` — mod 声明
- [x] `src/onboard/wizard.rs` — Default 值
- [x] `examples/huanxing-config.toml` — 配置示例
- [x] 编译通过（cargo check ✅）

## Phase 2: 工具注册 + 模板（进行中）
- [x] T1: 把 huanxing tools 注册到 all_tools_with_runtime()
- [x] T2: Guardian SOUL.md 模板
- [x] T3: Finance SOUL.md 模板
- [x] T4: 工具条件注册（Guardian 专属 vs 通用）
- [x] T5: 编译验证

## Phase 3: 端到端测试准备
- [x] T6: 写测试用 config.toml
- [x] T7: 创建 Guardian workspace 目录结构
- [x] T8: 创建 Finance 模板目录结构
- [x] T9: cargo build --release（21MB, 11m53s）
- [x] T10: 更新迁移方案文档
- [x] T11: DB schema 兼容性修复（匹配旧 users.db 的 PRIMARY KEY 结构）
- [x] T12: 渠道确认（napcat.rs + lark.rs 已内置，sender_id 映射确认）

## Phase 4: 部署与测试
- [ ] T13: 交叉编译 linux x86_64（或在服务器编译）
- [ ] T14: 部署脚本（传文件 + config + workspace）
- [ ] T15: 端到端消息测试
