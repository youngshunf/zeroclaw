//! Discovery —— 第二条 agent 注册路径（crash 恢复）
//!
//! 设计意图（见 14 §14.3）：Tauri 启动时批量扫描 `~/.huanxing/users/*/config.toml`
//! 的 `owner_key`，对每个已登录用户的 agents：
//! - 已有 `hasn_id` → 调 `node.add_agent_online`
//! - 无 `hasn_id` → 调 `node.register_agent` + 回写 config.toml + users.db
//!
//! 与桌面端 `onboard.ts` 的 `/api/agents/{name}/hasn-id` 注册路径并存，
//! 两条路径汇聚到同一个 hasn-node。
//!
//! 当前桌面端 crash 恢复由 `onboard.ts` 覆盖，本文件占位待 Phase 6+ 实装。
