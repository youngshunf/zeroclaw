# workspace-scaffold — 桌面端工作区模板

唤星桌面端用户登录后，自动从此目录复制文件到用户工作区 (`~/.huanxing/workspace/`)。

文件内容从 ZeroClaw 原版 `scaffold_workspace()` 原样翻译为中文。

## 文件清单（与原版一一对应）

| 文件 | 原版对应 | 占位符 |
|:--|:--|:--|
| `IDENTITY.md` | ✅ | `{{star_name}}` |
| `AGENTS.md` | ✅ | `{{star_name}}` |
| `HEARTBEAT.md` | ✅ | `{{star_name}}` |
| `SOUL.md` | ✅ | `{{star_name}}`, `{{comm_style}}` |
| `USER.md` | ✅ | `{{nickname}}`, `{{star_name}}`, `{{comm_style}}` |
| `TOOLS.md` | ✅ | 无 |
| `BOOTSTRAP.md` | ✅ | `{{nickname}}`, `{{star_name}}`, `{{comm_style}}` |
| `MEMORY.md` | ✅ | 无 |

## 占位符说明

| 占位符 | 来源 | 默认值 |
|:--|:--|:--|
| `{{nickname}}` | 用户昵称 | `主人` |
| `{{star_name}}` | 用户给 AI 起的名字 | `小星` |
| `{{comm_style}}` | 沟通风格 | `温暖、自然、简洁。适当使用 emoji（最多 1-2 个），避免机械化措辞。` |

## 自动创建的子目录

原版创建 5 个：`sessions/`, `memory/`, `state/`, `cron/`, `skills/`

## 与原版的差异

- 语言：英文 → 中文
- 默认时区：可选 → `Asia/Shanghai`
- 默认语言：English → 中文
- Emoji：🦀 → ⭐
- Agent 名：ZeroClaw → 用户自定义（默认小星）
- 其他结构和逻辑完全一致
