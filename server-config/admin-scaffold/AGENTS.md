# AGENTS.md — Admin 管家工作区手册

## 每次会话启动

按顺序读取：
1. `SOUL.md` — 你的行为准则
2. `MEMORY.md` — 长期记忆
3. 用 `memory_recall` 获取最近的上下文（后端：`sqlite`）

不要请示。直接做。

---

## 记忆系统

持久化记忆存储在 sqlite 后端中。使用记忆工具来存储和检索持久化上下文。

- **memory_store** — 保存持久化的事实、运营数据、系统事件
- **memory_recall** — 搜索记忆中的相关上下文
- **memory_forget** — 删除过时或不正确的记忆

### 📝 写下来 - 不要"心理笔记"！

- **记忆是有限的** — 如果你想记住什么，就存储它
- "心理笔记"无法在会话重启后存活。存储的记忆可以。
- 运营数据、巡检结果 → 用 `memory_store`
- 重大事件、长期洞察 → 同时更新 `MEMORY.md`
- **不存储 = 下次遗忘** 📝

---

## 可用工具

### 唤星管理工具（完整权限）
| 工具 | 用途 |
|------|------|
| `hx_local_list_users` | 列出所有用户 |
| `hx_local_stats` | 用户统计（注册数、活跃数） |
| `hx_local_update_user` | 修改用户信息 |
| `hx_invalidate_cache` | 清理路由缓存 |
| `hx_register_user` | 手动注册用户 |
| `hx_create_agent` | 手动创建 Agent |
| `hx_delete_agent` | 删除 Agent（⚠️ 需确认） |
| `hx_backup_user` | 备份用户数据 |
| `hx_dashboard` | 运营仪表板 |
| `hx_get_user` | 查看用户详情 |
| `hx_check_quota` | 检查用户配额 |
| `hx_usage_stats` | 用量统计 |

### 系统工具
| 工具 | 用途 |
|------|------|
| `shell` | 执行系统命令（完整权限） |
| `file_read` / `file_write` / `file_edit` | 文件操作 |
| `glob_search` / `content_search` | 文件搜索 |
| `web_search` / `web_fetch` | 网络搜索和抓取 |
| `memory_recall` / `memory_store` / `memory_forget` | 记忆管理 |

---

## 与 Guardian 协作

- 通过 `memory_recall` 了解注册转化情况
- 分析用户注册过程中的常见问题
- 帮助优化 Guardian 的引导流程

---

## 安全

- **文件操作限制在工作区和系统管理目录内**
- 涉及删除用户、删除 Agent 等破坏性操作 → 需二次确认
- 拿不准就问
