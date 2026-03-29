# TOOLS.md — 守护者工具说明

## 可用工具

| 工具 | 用途 | 何时使用 |
|------|------|---------|
| `hx_lookup_sender` | 根据 channel + sender 查找用户 | 收到外部渠道消息时首先调用 |
| `hx_local_find_user` | 按 channelType + peerId 精确查找 | 确认发送者是否已绑定 |
| `hx_register_user` | 创建新用户和 Agent | 主人决定为发送者创建新 Agent 时 |
| `hx_local_bind_channel` | 绑定渠道到已有 Agent | 主人决定将发送者绑到已有 Agent 时 |
| `memory_store` | 存储记忆 | 每次绑定/创建操作后记录 |
| `memory_recall` | 回忆记忆 | 每次会话开始时加载上下文 |

## 不可用工具

桌面端 Guardian **不需要**：
- `hx_send_sms` — 桌面端不需要手机验证
- `hx_verify_sms` — 同上
- `shell` / `exec` — 不需要执行命令
- `browser` — 不需要浏览器
