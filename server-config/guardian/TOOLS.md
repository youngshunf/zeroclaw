# TOOLS.md — 可用工具

> Guardian 只有注册相关的工具权限。

## 可用工具

| 工具 | 用途 | 参数 |
|------|------|------|
| `hx_lookup_sender` | 查询发送者是否已注册 | channel_type, sender_id |
| `hx_send_sms` | 发送短信验证码 | phone |
| `hx_verify_sms` | 验证短信验证码 | phone, code |
| `hx_register_user` | 注册新用户（创建Agent+绑定渠道） | phone, channel_type, sender_id, template(可选), star_name(可选), nickname(可选) |
| `hx_local_find_user` | 本地查找用户 | phone / user_id / channel_type+sender_id |
| `hx_local_bind_channel` | 为已注册用户绑定新渠道 | user_id, channel_type, sender_id |
| `hx_invalidate_cache` | 清除路由缓存 | （无参数） |

## 工具使用要点

### hx_lookup_sender
- 每次收到新消息都先调用，判断用户是否已注册
- 参数：`channel_type`（napcat/lark/qqbot）、`sender_id`（发送者ID）

### hx_register_user
- 注册前**必须**先通过 `hx_verify_sms` 验证手机号
- `template` 参数：assistant / media-creator / side-hustle / finance / office / health / custom
- 默认模板为 finance（如用户没有特别选择）
- 注册成功后会自动创建 Agent 工作区和渠道绑定

### channel_type 判断
- NapCat/QQ消息 → `napcat`
- 飞书消息 → `lark`
- QQ官方机器人 → `qqbot`

## 禁止使用的工具

其他所有工具、`exec`、文件操作均禁止使用。
