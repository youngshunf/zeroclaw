# 测试模式

**分析日期：** 2026-03-21

## 测试框架

**运行器：**
- Rust 内置测试框架（`cargo test`）
- 异步测试使用 `#[tokio::test]` 宏（`tokio` 运行时）
- Benchmark 使用 `criterion = "0.8"` + `async_tokio` feature

**辅助库（`[dev-dependencies]`）：**
- `tempfile = "3.26"` — 临时目录/文件
- `wiremock = "0.6"` — HTTP mock 服务器（live 测试用）
- `scopeguard = "1.2"` — 测试清理保障

**运行命令：**
```bash
cargo test                                    # 运行所有测试
cargo test --test component                   # 仅组件测试
cargo test --test integration                 # 仅集成测试
cargo test --test system                      # 仅系统测试
cargo test --test live -- --ignored           # 需要凭证的 live 测试
cargo test --features huanxing               # 含唤星功能测试
./dev/ci.sh test                              # Docker CI 容器中运行全套
./dev/ci.sh all                               # lint + test + build + security
```

## 测试文件组织

**分层目录（`tests/`）：**
```
tests/
├── test_component.rs       # 组件测试入口（引入 component/ + support/）
├── test_integration.rs     # 集成测试入口（引入 integration/ + support/）
├── test_system.rs          # 系统测试入口（引入 system/ + support/）
├── test_live.rs            # Live 测试入口（需要外部凭证，#[ignore]）
├── component/              # 组件边界测试
│   ├── mod.rs
│   ├── config_schema.rs    # 配置 schema 验证、默认值、TOML 兼容性
│   ├── security.rs         # 安全策略、自治等级
│   ├── gateway.rs          # 网关常量、HMAC 签名验证
│   ├── provider_schema.rs
│   ├── provider_resolution.rs
│   ├── config_persistence.rs
│   └── ...
├── integration/            # 集成测试（跨模块边界）
│   ├── mod.rs
│   ├── agent.rs            # Agent 完整编排循环（E2E）
│   ├── agent_robustness.rs # 错误恢复、边界条件
│   ├── memory_restart.rs   # SQLite 内存持久化
│   ├── memory_comparison.rs
│   ├── hooks.rs
│   └── channel_routing.rs
├── system/                 # 系统测试（真实后端）
│   ├── mod.rs
│   └── full_stack.rs       # MockProvider + 真实 SQLite + 全组件
└── support/                # 共享测试基础设施
    ├── mod.rs
    ├── helpers.rs          # Agent 构建工厂函数、response 工厂
    ├── mock_provider.rs    # MockProvider / RecordingProvider / TraceLlmProvider
    ├── mock_tools.rs       # EchoTool / CountingTool / FailingTool / RecordingTool
    ├── mock_channel.rs     # TestChannel（实现 Channel trait）
    ├── assertions.rs       # trace fixture 断言辅助
    └── trace.rs            # LlmTrace fixture 结构
```

**单元测试位置：** 与源文件同目录，在 `#[cfg(test)] mod tests { ... }` 块中。
- 示例：`src/agent/mod.rs` 引用 `mod tests;`，对应 `src/agent/tests.rs`（独立文件）
- 示例：`src/tools/traits.rs` 内有内联 `#[cfg(test)] mod tests { ... }`

**命名约定：**
- 测试函数名：`<模块>_<被测行为>_<预期结果>`，如：
  - `gateway_whatsapp_valid_signature_accepted`
  - `sqlite_memory_store_same_key_deduplicates`
  - `e2e_multi_turn_history_fidelity`
  - `security_default_autonomy_is_supervised`

## 测试结构模式

**组件测试套件组织（以 `tests/component/gateway.rs` 为参考）：**
```rust
//! Gateway component tests.
//!
//! 顶部 `//!` 注释说明测试覆盖范围和测试边界。

// ═══════════════════════════════════════════════════════
// 用 Unicode 分隔符将测试分组
// ═══════════════════════════════════════════════════════

/// 单行 `///` 说明测试验证的具体行为。
#[test]
fn gateway_whatsapp_valid_signature_accepted() {
    // ... 测试实现
}
```

**集成测试模式（以 `tests/integration/agent.rs` 为参考）：**
```rust
/// Validates the simplest happy path: user message → LLM text response.
#[tokio::test]
async fn e2e_simple_text_response() {
    let provider = Box::new(MockProvider::new(vec![text_response("Hello")]));
    let mut agent = build_agent(provider, vec![Box::new(EchoTool)]);

    let response = agent.turn("hi").await.unwrap();
    assert!(!response.is_empty(), "Expected non-empty text response");
}
```

**系统测试模式（以 `tests/system/full_stack.rs` 为参考）：**
```rust
#[tokio::test]
async fn system_simple_text_response() {
    let provider = Box::new(MockProvider::new(vec![text_response("...")]));

    // 系统测试使用真实 SQLite 内存，通过 tempfile 隔离
    let temp_dir = tempfile::tempdir().unwrap();
    let mut agent = build_agent_with_sqlite_memory(
        provider, vec![Box::new(EchoTool)], temp_dir.path()
    );

    let response = agent.turn("hello system").await.unwrap();
    assert_eq!(response, "...");
}
```

## Mock 实现

**Provider Mock（`tests/support/mock_provider.rs`）：**

```rust
/// FIFO 脚本化响应 Provider
pub struct MockProvider {
    responses: Mutex<Vec<ChatResponse>>,
}

/// 记录请求 + FIFO 响应 Provider（用于验证消息历史）
pub struct RecordingProvider {
    responses: Mutex<Vec<ChatResponse>>,
    recorded_requests: Arc<Mutex<Vec<Vec<ChatMessage>>>>,
}

// RecordingProvider 工厂：返回 provider 和共享录制句柄
let (provider, recorded) = RecordingProvider::new(vec![text_response("answer")]);
// 测试后断言
let requests = recorded.lock().unwrap();
assert_eq!(requests[0][1].content, "expected user message");
```

**Tool Mock（`tests/support/mock_tools.rs`）：**

| Mock 类型 | 用途 |
|-----------|------|
| `EchoTool` | 回显输入 message 参数 |
| `CountingTool` | 计数调用次数（通过 `Arc<Mutex<usize>>`） |
| `FailingTool` | 始终返回失败的 `ToolResult` |
| `RecordingTool` | 记录所有参数用于断言 |

```rust
// CountingTool / RecordingTool 的工厂函数模式
let (counting_tool, count) = CountingTool::new();
// 测试后
assert_eq!(*count.lock().unwrap(), 2);

let (recording_tool, calls) = RecordingTool::new("recorder");
// 测试后
assert_eq!(calls.lock().unwrap()[0]["input"].as_str().unwrap(), "test_value");
```

**Channel Mock（`tests/support/mock_channel.rs`）：**

```rust
pub struct TestChannel {
    sent_messages: Arc<Mutex<Vec<SendMessage>>>,
    typing_events: Arc<Mutex<Vec<TypingEvent>>>,
}
// 实现完整 Channel trait，支持 start_typing/stop_typing 事件录制
```

**Memory Mock：**
- 在测试中通过 `MemoryConfig { backend: "none".into(), ... }` 获得无持久化内存
- 系统测试用 `MemoryConfig { backend: "sqlite".into(), ... }` + `tempfile::TempDir` 获得真实 SQLite

## 测试工厂函数

所有工厂函数在 `tests/support/helpers.rs`：

```rust
// ChatResponse 工厂
pub fn text_response(text: &str) -> ChatResponse { ... }
pub fn tool_response(calls: Vec<ToolCall>) -> ChatResponse { ... }

// Agent 构建工厂（按需选择 dispatcher 和 memory）
pub fn build_agent(provider, tools) -> Agent          // NativeToolDispatcher + NoopMemory
pub fn build_agent_xml(provider, tools) -> Agent      // XmlToolDispatcher + NoopMemory
pub fn build_recording_agent(provider, tools, loader) // 支持自定义 MemoryLoader
pub fn build_agent_with_sqlite_memory(provider, tools, path) // 真实 SQLite

// 静态 MemoryLoader（用于测试 memory enrichment）
pub struct StaticMemoryLoader { context: String }
```

## 覆盖率

**要求：** 无 CI 强制覆盖率门槛（`Cargo.toml` 中未配置 `cargo-tarpaulin`）

**查看覆盖率：**
```bash
# 未内置，可手动运行
cargo tarpaulin --features huanxing --out Html
```

## 测试类型

**组件测试（`tests/component/`）：**
- 范围：单个模块的公共 API 边界
- 依赖：仅使用真实库函数，无 mock
- 特点：测试配置 schema 默认值/TOML 序列化、常量值、签名验证算法

**集成测试（`tests/integration/`）：**
- 范围：跨模块边界，使用 mock 替代外部服务
- 依赖：`MockProvider`、`RecordingProvider`、`EchoTool` 等 mock
- 特点：验证 Agent 编排循环、内存持久化、消息历史保真度

**系统测试（`tests/system/`）：**
- 范围：全组件接线，使用真实 SQLite 内存
- 依赖：`MockProvider`（仅替换 LLM 网络调用）
- 特点：验证跨层数据流，tempfile 隔离每个测试

**Live 测试（`tests/test_live.rs`）：**
- 标注 `#[ignore]`，仅通过 `cargo test -- --ignored` 运行
- 需要真实 API 凭证

**单元测试（`src/**` 内）：**
- 在 `#[cfg(test)] mod tests { ... }` 块中
- 直接测试私有函数
- 大型模块（如 `src/agent/tests.rs`）独立为文件

## 常用断言模式

**异步测试：**
```rust
#[tokio::test]
async fn test_name() {
    let result = some_async_fn().await.unwrap();
    assert_eq!(result, expected);
}
```

**错误测试：**
```rust
let result: Result<Config, _> = toml::from_str("port = -1\n");
assert!(result.is_err(), "negative port should fail for u16");
```

**序列化往返（TOML）：**
```rust
let toml_str = toml::to_string(&config).expect("config should serialize");
let parsed: Config = toml::from_str(&toml_str).expect("should deserialize back");
assert_eq!(parsed.field, original.field);
```

**精确断言说明：** 所有 `assert!` 和 `assert_eq!` 均携带错误说明字符串，如：
```rust
assert_eq!(count, 1, "Tool should be called exactly once");
assert!(gw.require_pairing, "pairing should be required by default");
```

**pattern matching 断言：**
```rust
assert!(matches!(&history[0], ConversationMessage::Chat(c) if c.role == "system"));
```

## Trace Fixture 系统

`tests/support/trace.rs` 提供 `LlmTrace` fixture 结构，支持从 JSON/YAML 文件加载预录制的 LLM 对话用于回放测试：

```rust
// TraceLlmProvider：从 LlmTrace fixture 回放
pub struct TraceLlmProvider { steps: Mutex<Vec<TraceResponse>>, ... }

// assertions.rs：声明式期望验证
verify_expects(&expects, &final_response, &tools_called, label);
```

---

*测试分析：2026-03-21*
