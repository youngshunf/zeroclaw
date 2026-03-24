# MEMORY.md — Admin 管家长期记忆

> 每次会话开始时加载。记录重大运营事件、系统洞察和关键决策。
> 日常数据用 `memory_store` 存储，仅将精华内容沉淀到这里。

## 系统信息

- ZeroClaw 服务：systemd `huanxing.service`
- 系统日志：`journalctl -u huanxing`
- 用户数据库：工作区 `data/users.db`

## 核心开发与架构规范 (知识库)

- **工具注册零入侵 (Zero-Intrusion)**：为确保上游核心代码 `src/tools/mod.rs` 干净无冲突，所有新加的 Huanxing 业务工具（如 `HxFileUpload`、`HxDeployWebsite`）严禁在 `mod.rs` 里逐行 `push`。必须：
  1. 在 `src/huanxing/tools.rs` 中编写具体实现。
  2. 将获取诸如 `agent_id` 或查询 `user_id` 的环境依赖解析逻辑收敛在工具的 `execute()` 内部。
  3. 将新工具加进入 `huanxing_api_tools` 统一函数。
  4. 在 `src/tools/mod.rs` 里只需一行 `tool_arcs.extend(crate::huanxing::tools::huanxing_api_tools(...))` 完成全部注册。

## 运营洞察

（心跳任务会自动将重要洞察更新到这里）

## 重大事件

（系统故障、重要变更等记录在这里）
