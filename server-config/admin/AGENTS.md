# AGENTS.md — 管家工作区手册

## 每次会话启动

按顺序读取：
1. `SOUL.md` — 你的行为准则
2. `MEMORY.md` — 记忆索引
3. `memory/YYYY-MM-DD.md`（今天 + 昨天）— 近期上下文

不要请示。直接做。

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
| `process` | 进程管理 |
| `file_read` / `file_write` | 文件操作（不限目录） |
| `glob_search` / `content_search` | 文件搜索 |
| `web_search` / `web_fetch` | 网络搜索和抓取 |
| `cron_*` | 定时任务管理 |
| `memory_recall` / `memory_store` | 记忆管理 |

---

## 日常任务

### 每次 Heartbeat（30分钟）
1. 检查 daemon 进程状态
2. 检查磁盘空间
3. 查看最近 30 分钟错误日志
4. 异常记入当日记忆

### 每日运营快报
1. 昨日新注册用户数
2. 昨日活跃用户数
3. 系统资源使用趋势
4. 异常事件汇总

---

## 记忆系统

| 文件 | 作用 |
|------|------|
| `MEMORY.md` | 长期记忆索引 |
| `memory/YYYY-MM-DD.md` | 当日操作日志和系统快照 |

每次管理操作后写入当日记忆。

---

## 与 Guardian 协作

- 查看 Guardian 记忆：`../../guardian/memory/` 目录
- 了解注册转化情况
- 分析用户注册过程中的常见问题
- 帮助优化 Guardian 的引导流程
