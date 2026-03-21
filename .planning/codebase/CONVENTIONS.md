# 编码约定

**分析日期：** 2026-03-21

## 命名规范

**文件与模块：**
- 模块名使用 `snake_case`，如 `loop_.rs`、`session_store.rs`、`whatsapp_web.rs`
- 特殊情况：避免与 Rust 关键字冲突时加下划线后缀（`loop_.rs`）
- 唤星扩展模块统一放在 `src/huanxing/` 下，不散布到上游代码

**函数与方法：**
- 公共函数：`snake_case`，如 `verify_whatsapp_signature`、`build_agent`
- 私有辅助函数：同样 `snake_case`，如 `webhook_memory_key`、`glob_match`
- 构造器惯例：`new(...)` 用于简单构建，`from_xxx(...)` 用于转换构建
- Builder 模式：链式调用 `.provider(...).tools(...).build()`

**类型与结构体：**
- `PascalCase`，如 `AgentBuilder`、`SecurityPolicy`、`TenantContext`
- 特征（Trait）名称简洁且语义明确：`Channel`、`Provider`、`Tool`、`Memory`
- 枚举变体：`PascalCase`，如 `AutonomyLevel::Supervised`、`CommandRiskLevel::High`

**常量：**
- `SCREAMING_SNAKE_CASE`，如 `MAX_BODY_SIZE`、`REQUEST_TIMEOUT_SECS`、`RATE_LIMIT_WINDOW_SECS`
- 模块级常量放在文件顶部，具有清晰注释说明用途

**泛型参数：**
- 使用语义名，如 `impl Into<String>` 而非 `T`（对字符串参数特别常见）

## 代码格式

**工具配置（`rustfmt.toml`）：**
- 最大行宽：100 字符
- 缩进：4 个空格，不使用硬制表符
- 开启 `use_field_init_shorthand = true`（字段简写）
- 开启 `use_try_shorthand = true`（`?` 运算符）
- 自动重排 imports 和模块（`reorder_imports = true`）
- `match_arm_leading_pipes = "Never"`

**Clippy 配置（`clippy.toml`）：**
- 认知复杂度上限：30
- 函数参数上限：10
- 函数行数上限：200
- 数组大小阈值：65536（照顾测试代码中的大缓冲区）

**全局 Clippy 启用（`src/lib.rs`）：**
```rust
#![warn(clippy::all, clippy::pedantic)]
```
同时通过 `#![allow(...)]` 豁免了若干过于严格的 lint（见 `src/lib.rs` 第 2–36 行）。

## Import 组织

**标准顺序（rustfmt 自动排序）：**
1. `crate::` 内部模块引用
2. 外部 crate（`anyhow`、`async_trait`、`axum`、`serde` 等）
3. 标准库（`std::collections`、`std::sync` 等）

**实际示例（`src/gateway/mod.rs`）：**
```rust
use crate::channels::{...};
use crate::config::Config;
use anyhow::{Context, Result};
use axum::{body::Bytes, extract::..., routing::...};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
```

**路径别名：** 无全局 `use` 别名约定，但部分模块用 `use crate::xxx as xxx` 消歧义。

## 错误处理

**策略：**
- 几乎所有函数使用 `anyhow::Result` 返回错误（`anyhow = "1.0"` + `thiserror = "2.0"`）
- 失败时优先 `anyhow::bail!(...)` 或 `return Err(anyhow::anyhow!(...))` 立即退出
- 上下文追加：`.context("...")` 或 `.with_context(|| format!("...", ...))`
- 不使用 `unwrap()`（测试代码除外），生产路径均显式处理错误

**实际示例：**
```rust
// bail! 快速失败
anyhow::bail!("napcat.websocket_url cannot be empty");

// context 追加
let raw = fs::read_to_string(path).await
    .with_context(|| format!("Failed to read {}", path.display()))?;

// 工具执行返回结构化结果而非 Err
Ok(ToolResult {
    success: false,
    output: String::new(),
    error: Some("Service unavailable: connection timeout".into()),
})
```

**工具执行的双重模式：**
- 网络/IO 层：返回 `anyhow::Result<T>`，可冒泡
- 工具结果层：返回 `anyhow::Result<ToolResult>`，通过 `ToolResult.success`/`ToolResult.error` 传递语义错误，不走 Err 通道

## 日志

**框架：** `tracing` crate（`tracing = "0.1"`）

**使用级别：**
- `tracing::info!` — 正常生命周期事件（启动、监听、连接）
- `tracing::warn!` — 可恢复异常（解析失败、无效签名、请求失败）
- `tracing::debug!` — 详细内部状态（仅调试时开启）
- `tracing::error!` — 严重错误（极少直接使用，通常冒泡为 `?`）

**格式：** 宏内联变量使用格式参数，如：
```rust
tracing::warn!("Napcat HTTP request failed ({status}): {sanitized}");
tracing::info!("iMessage channel listening (AppleScript bridge)...");
```

## 注释与文档

**模块文档（`//!`）：**
- 每个模块文件顶部有 `//!` 注释说明职责
- 架构决策用 ASCII 流程图说明（见 `src/huanxing/mod.rs`、`src/gateway/mod.rs`）

**公共 API（`///`）：**
- 公共结构体、枚举、trait 均有 `///` 文档注释
- 字段注释说明语义边界（如 `thread_ts` 与 `interruption_scope_id` 的区别）

**行内注释（`//`）：**
- 解释非显而易见的逻辑
- 安全性备注标记为 `// NOTE:` 或 `// HUANXING:` 格式

**测试注释：**
- 每个测试函数有单行 `///` 说明验证了什么
- 测试文件顶部 `//!` 列举测试覆盖的所有用例（见 `src/agent/tests.rs`）

## Trait 与接口设计

**核心模式：** trait 驱动 + 工厂注册

所有扩展点通过 trait 定义，由 `#[async_trait]` 支持异步：
```rust
// 定义 trait（src/tools/traits.rs）
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> serde_json::Value;
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult>;
    // 默认实现
    fn spec(&self) -> ToolSpec { ... }
}
```

**Builder 模式（`src/agent/agent.rs`）：**
- 复杂对象用 `XxxBuilder` 构建
- `builder()` 关联函数返回 Builder
- 所有字段 `Option<T>` 起步，`.build()` 时验证必填项

**构造器辅助函数（`impl Into<String>`）：**
```rust
// 统一接受 &str 和 String
pub fn new(content: impl Into<String>, recipient: impl Into<String>) -> Self
```

## 可见性规范

**原则：** 最小可见性
- `pub mod`：对外 API、可扩展接口
- `pub(crate) mod`：内部共享但不对外暴露（`src/security/`、`src/daemon/` 等）
- 无 `pub`：模块内部实现细节

**唤星扩展可见性：**
- 所有唤星代码用 `#[cfg(feature = "huanxing")]` feature-gate
- 上游文件只加 `cfg` 注解，不加字段

## 并发模式

**共享状态：**
- `Arc<T>` 用于跨线程共享所有权
- `Arc<Mutex<T>>` 用于可变共享状态（测试 mock 中频繁使用）
- `parking_lot::Mutex` 优先于 `std::sync::Mutex`（性能更好，不 poison）
- `tokio::sync::RwLock` 用于读多写少场景

**实际用法（测试 mock 中）：**
```rust
pub struct MockProvider {
    responses: Mutex<Vec<ChatResponse>>,
}
// CountingTool 通过 Arc<Mutex<usize>> 跨线程记录调用次数
let count = Arc::new(Mutex::new(0));
(Self { count: count.clone() }, count)
```

---

*约定分析：2026-03-21*
