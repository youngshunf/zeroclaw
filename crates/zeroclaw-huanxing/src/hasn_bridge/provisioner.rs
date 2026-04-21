//! Provisioner —— PROVISION 帧反向回调桥接
//!
//! hasn-node 收到 `hasn.node.provision` 帧时会调用注入的
//! `AgentProvisioner`，让宿主根据 agent 规格创建本地 agent 骨架。
//!
//! 当前 hasn-node 尚未定义 `AgentProvisioner` trait（计划 Phase 6+），
//! 所以本文件只占位。实装时：
//! - impl `hasn_node::AgentProvisioner`（trait 定义后）
//! - 内部调 `huanxing_agent_factory::AgentFactory::create_local_agent`
//! - 把返回的 agent 元信息（含 hasn_id）写回 `config.toml` + `users.db`
//!
//! 参考：`docs/架构设计/HASN-Node独立运行时/14-桌面端完整迁移实施计划.md` §14.3
