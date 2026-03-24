# HEARTBEAT.md — Admin 管家定时任务

> 所有任务**必须**带 `schedule:cron` 标签，系统根据 cron 表达式自动触发。
> 不带 schedule 的任务会被系统忽略。
> 格式: `- [priority|schedule:分 时 日 月 周] 任务描述`
> ⚠️ cron 时间为 UTC。中国时间 = UTC + 8（如：早8点 → UTC 0:00，晚8点 → UTC 12:00）

## 系统巡检

- [high|schedule:0 */1 * * *] 每小时巡检：检查 ZeroClaw daemon 进程状态、磁盘空间使用率、最近 1 小时错误日志。一切正常回复 HEARTBEAT_OK，发现异常则用 `memory_store` 记录并简短汇报。
- [schedule:30 0 * * *] 每日数据库体检（早8:30）：检查 sessions.db 和 brain.db 文件大小，确保没有异常增长。结果用 `memory_store` 记录。

## QQ NapCat 监控

- [high|schedule:*/30 * * * *] 每半小时检查 NapCat 登录状态：用 `shell` 执行 `docker ps | grep napcat` 检查容器是否运行，再用 `curl -s http://localhost:6099/api/get_login_info` 检查 QQ 登录状态。如果容器未运行或 QQ 被踢下线/异常退出，立即汇报管理员并提醒重新登录。正常则回复 HEARTBEAT_OK。

## 运营统计

- [high|schedule:0 1 * * *] 每日运营快报（早9点）：用 `hx_local_stats` 统计昨日新注册用户数、活跃用户数，生成简要运营数据，用 `memory_store` 记录并汇报给管理员。

## 记忆维护

- [schedule:0 16 * * *] 记忆整理（每日0点）：用 `memory_recall` 回顾最近记忆，用 `memory_store` 保存重要信息；将提炼出的记忆精华（重大事件、运营洞察）用 `file_edit` 更新到 `MEMORY.md`；用 `memory_forget` 清除过时条目。完成后回复 HEARTBEAT_OK。
