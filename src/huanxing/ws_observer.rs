//! WsObserver — 收集工具调用事件，供 ws.rs 在 turn 完成后批量推送给前端。
//!
//! Observer::record_event 是同步调用，发生在 agent.turn() 内部。
//! WsObserver 将事件缓冲到 Mutex<Vec<…>> 中；turn() 返回后，ws.rs
//! 调用 take_events() 取出所有事件并转换为 WS 帧发送给客户端。

use crate::observability::{Observer, ObserverEvent};
use crate::observability::traits::ObserverMetric;
use parking_lot::Mutex;
use std::any::Any;
use std::time::Duration;

/// 一次 turn 中发生的工具调用记录
#[derive(Debug, Clone)]
pub struct ToolCallRecord {
    /// 工具内部名称（如 "shell", "read_file"）
    pub name: String,
    /// 前端展示名称（中文友好）
    pub display_name: String,
    /// 调用参数预览（截取前 200 字符）
    pub args_preview: String,
    /// 执行耗时
    pub duration: Duration,
    /// 是否成功
    pub success: bool,
}

/// 缓冲式 Observer，收集工具调用事件
pub struct WsObserver {
    records: Mutex<Vec<ToolCallRecord>>,
}

impl WsObserver {
    pub fn new() -> Self {
        Self {
            records: Mutex::new(Vec::new()),
        }
    }

    /// 取出并清空已收集的记录
    pub fn take_records(&self) -> Vec<ToolCallRecord> {
        std::mem::take(&mut *self.records.lock())
    }
}

impl Default for WsObserver {
    fn default() -> Self {
        Self::new()
    }
}

impl Observer for WsObserver {
    fn record_event(&self, event: &ObserverEvent) {
        // 只关心工具调用完成事件
        if let ObserverEvent::ToolCall { tool, duration, success } = event {
            let display_name = tool_display_name(tool);
            self.records.lock().push(ToolCallRecord {
                name: tool.clone(),
                display_name,
                args_preview: String::new(), // ToolCallStart 未触发时为空
                duration: *duration,
                success: *success,
            });
        }
    }

    fn record_metric(&self, _metric: &ObserverMetric) {}

    fn name(&self) -> &str {
        "ws-observer"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// 工具名称 → 前端中文展示名
fn tool_display_name(name: &str) -> String {
    match name {
        "shell" | "bash" | "run_command" => "执行命令",
        "read_file" | "read_skill" => "读取文件",
        "write_file" => "写入文件",
        "search_memory" | "recall_memory" => "查询记忆",
        "save_memory" => "保存记忆",
        "web_fetch" | "fetch_url" => "获取网页",
        "web_search" | "search" => "网络搜索",
        "list_directory" | "ls" => "列出目录",
        "glob" => "文件搜索",
        "grep" => "内容搜索",
        _ => name,
    }
    .to_string()
}
