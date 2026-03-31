# workspace-scaffold — 桌面端工作区模板

唤星桌面端用户首次启动或登录后，会自动从此目录复制文件来初始化本地工作区（`~/.huanxing/`）。

## 目录结构说明

- `config.toml.template`: 全局配置模板（写入 `~/.huanxing/config.toml`），用于维护所有 Agent 共享的网关地址和基础设置。
- `default/`: 默认 Agent（预设助手）的工作区模板目录。包含其 `config.toml` 模板及所有的 markdown 控制文件（进入 `~/.huanxing/agents/default/` 和宿主配置区）。
- `guardian/`: （如果有）存放系统级守护进程的相关配置模板。

## 文件清单（位于 default/ 目录）

| 文件 | 原版对应 | 作用说明 | 占位符 |
|:--|:--|:--|:--|
| `config.toml.template` | | Default Agent 的独立配置模板 | `{{star_name}}`, `{{default_model}}` |
| `IDENTITY.md` | ✅ | 角色设定 | `{{star_name}}` |
| `AGENTS.md` | ✅ | 子 Agent 配置 | `{{star_name}}` |
| `HEARTBEAT.md` | ✅ | 定时任务 | `{{star_name}}` |
| `SOUL.md` | ✅ | 核心性格与心智规则 | `{{star_name}}`, `{{comm_style}}` |
| `USER.md` | ✅ | 宿主全局画像 | `{{nickname}}`, `{{star_name}}` |
| `TOOLS.md` | ✅ | 允许使用的工具列表 | 无 |
| `BOOTSTRAP.md` | ✅ | 全局启动提示词钩子 | `{{nickname}}`, `{{star_name}}` |
| `MEMORY.md` | ✅ | 核心长期记忆 | 无 |

## 占位符常规说明

| 占位符 | 来源 | 默认值 |
|:--|:--|:--|
| `{{nickname}}` | 用户昵称 | `主人` |
| `{{star_name}}` | 用户给 AI 起的名字 | `小星` |
| `{{comm_style}}` | 沟通风格 | `温暖、自然、简洁。适当使用 emoji（最多 1-2 个），避免机械化措辞。` |

## 与原版 ZeroClaw 的差异

- 架构脱钩：全局网关配置（`config.toml`）与单个 Agent 的配置实行了分离。
- 语言：英文 → 中文
- 默认时区：可选 → `Asia/Shanghai`
- 默认语言：English → 中文
- Emoji：🦀 → ⭐
- Agent 名：ZeroClaw → 用户自定义（默认小星）
