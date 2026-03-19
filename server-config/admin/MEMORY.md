# MEMORY.md — Admin Agent 记忆索引

每次会话开始，读取这个文件了解历史上下文。

## 运维日志格式
- 按日期记录在 `memory/YYYY-MM-DD.md`
- 包含：系统状态、用户操作、异常事件

## 关键数据位置
- 用户数据库：`../../data/users.db`
- Guardian 工作区：`../../guardian/`
- 用户 Agent 工作区：`../../agents/`
- 系统日志：`/tmp/zeroclaw-local.log`
- 全局配置：`../../config.toml`
