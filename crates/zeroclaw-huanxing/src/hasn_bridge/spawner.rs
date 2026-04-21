//! HuanxingNativeSpawner — 唤星原生 Agent 进程内桥接层
//!
//! Phase 5 将 Phase 2 的 trait 骨架升级为真实的 embedded bridge：
//! - 继续复用 zeroclaw-huanxing 既有 tenant/session/runtime 逻辑
//! - 通过 hasn-node 的 generic `ReplyChunk` 合同把结果回交给路由层
//! - 不在 spawner 内直接写 WS 帧，也不额外引入 HTTP / IPC 回环
//!
//! # Phase 05-05 — HASN Cutover Decision Table
//!
//! Phase 05-01/02 在 hasn-node 侧建好了 router + SpawnerRegistry + HuanxingNativeSpawner，
//! 但「入口（WS connect）+ 出口（/send）+ 入站 dispatch」仍留在 legacy
//! `zeroclaw_huanxing::hasn_connector/hasn_router`。本 Plan（05-05）把这三块迁到 hasn-node。
//!
//! | 能力                        | Before (legacy 独占)                                        | After (hasn-node 独占)                                                 | 备注 |
//! |-----------------------------|-------------------------------------------------------------|-------------------------------------------------------------------------|------|
//! | HASN WS 长连接              | `zeroclaw_huanxing::hasn_connector::global_connector().connect` | `hasn_node::connector::global_connector_opt().connect_with_retry`   | main.rs 启动段 cutover |
//! | 出站 send_message           | legacy `HasnConnector::send_message`                        | hasn-node `HasnConnector::send_message`                                 | `hasn_api::hasn_send` 薄转发 |
//! | 入站 `hasn.message.received`| legacy `MessageRouter::dispatch` → `HasnAgentBridge::inject_and_stream` | hasn-node `router::dispatch` → `SpawnerRegistry` → `HuanxingNativeSpawner` | 05-01 已实装，本 Plan 只做「通电」 |
//! | Owner ↔ 自家 Agent          | 走 `ws.send_frame` → 服务端 2006 拦截                       | hasn-node router 本地闭环 → 回复经 Spawner 写 chat_db + broadcast `HasnEvent::Message` | 05-05 Task 3 新增 |
//! | `/ws/hasn-events` broadcast | legacy `hasn_connector.subscribe()`                         | hasn-node `connector.subscribe()`                                       | 字段形状保持一致 |
//! | chat_db 落盘路径            | `~/.huanxing/users/{tenant_dir}/data/hasn_chat.db`          | `~/.huanxing/users/{tenant_dir}/data/hasn_chat.db`                      | **不变** — 继续写 legacy 路径；hasn-node 的 `~/.hasn/hasn_db.sqlite` 在本 Plan 中不涉及 |
//! | local_entities (Owner + Agent 驻留) | legacy HashSet (in-memory)                          | hasn-node `local_agents` 表 (SQLite)                                    | 驻留注册在 hasn-node 的 `add_owner_ack` / `add_agent_ack` 里写表 |
//! | Agent 注册映射 (a_xxx → workspace) | legacy `TenantDb::find_by_hasn_id`                   | `HuanxingNativeSpawner::chat_db_for_hasn_id` 内复用 legacy `TenantDb`  | **不变** — huanxing-native 是桥接层，仍依赖 zeroclaw-huanxing 的 TenantDb |
//! | PROVISION_AGENT             | legacy `handle_ws_frame` 真实创建 agent + workspace         | hasn-node 目前仅 log（未实装 `AgentProvisioner`）                       | **本 Plan 不迁移** — 保留 legacy 这条非热路径（将来 Phase 6+） |
//! | hasn_sync.rs（联系人 Pull） | legacy `global_connector()`                                 | legacy 保留                                                             | **不变** — 非 WS 热路径，独立 HTTP 拉取 |
//!
//! ## 硬约束
//! - 桌面端 `clients/desktop/src/lib/hasn-api.ts` 零改动（Phase 2 Success Criteria 2）
//! - legacy `hasn_connector::global_connector` symbol 保留给 `hasn_sync.rs` 等非热路径使用（Pull 云端联系人列表），但 WS frame 入站处理走 hasn-node
//! - legacy `handle_ws_frame` / `MessageRouter::dispatch` / `HasnAgentBridge::inject_and_stream` 进入 deprecated 状态：保留符号 + `debug_assert!(false, "Phase 05-05: legacy path deprecated")`，确保 dual-WS 不共存
//!
//! ## peer_id 修正
//! - legacy `hasn_router::MessageRouter::dispatch`（`hasn_router.rs:140`）upsert_session 的 peer 参数误用 `&message.from_id`（= Owner 自己），应为 `target_id`（= 对端 Agent 的 `a_xxx`）
//! - Task 3 在 `HasnAgentBridge::dispatch_to_reply_chunks`（以及共享的 session upsert helper）把 peer 显式写为 `ctx.agent_hasn_id`
//! - Task 5 附幂等迁移 `HasnChatDb::run_migration_phase_05_05_peer_id` 修正历史污染

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use hasn_node::node::Node;
use hasn_node::spawner::{AgentSpawner, InboundContext, ReplyChunk};
use tokio::sync::{RwLock, mpsc};
use zeroclaw_config::schema::Config;
use zeroclaw_infra::session_backend::SessionBackend;

use crate::hasn_bridge::agent_bridge::HasnAgentBridge;
use crate::hasn_bridge::chat_db::HasnChatDb;
use crate::hasn_bridge::connector::HasnAgentSession;

#[derive(Clone)]
struct HuanxingNativeRuntime {
    config: Config,
    session_backend: Option<Arc<dyn SessionBackend>>,
}

fn runtime_slot() -> &'static std::sync::Mutex<Option<HuanxingNativeRuntime>> {
    static SLOT: std::sync::OnceLock<std::sync::Mutex<Option<HuanxingNativeRuntime>>> =
        std::sync::OnceLock::new();
    SLOT.get_or_init(|| std::sync::Mutex::new(None))
}

fn embedded_node_slot() -> &'static std::sync::Mutex<Option<Arc<Node>>> {
    static SLOT: std::sync::OnceLock<std::sync::Mutex<Option<Arc<Node>>>> =
        std::sync::OnceLock::new();
    SLOT.get_or_init(|| std::sync::Mutex::new(None))
}

fn current_runtime() -> anyhow::Result<HuanxingNativeRuntime> {
    runtime_slot()
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| anyhow::anyhow!("huanxing native runtime not configured"))
}

pub fn configure_huanxing_native_runtime(
    config: Config,
    session_backend: Option<Arc<dyn SessionBackend>>,
) {
    *runtime_slot().lock().unwrap() = Some(HuanxingNativeRuntime {
        config,
        session_backend,
    });
}

pub async fn initialize_embedded_huanxing_node(config: &Config) -> anyhow::Result<Arc<Node>> {
    if let Some(node) = embedded_node_slot().lock().unwrap().clone() {
        return Ok(node);
    }

    let config_path = config.config_path.to_string_lossy().to_string();
    let mut node_config = hasn_node::config::NodeConfig::load(&config_path)?;

    // 从 `[huanxing] hasn_base_url`（或兜底 api_base_url）自动推导 HASN WS URL，
    // 注入 hasn-node NodeConfig。用户安装桌面端后由 onboarding 写入 `hasn_base_url`，
    // 无需手动配置 hasn-node 独立字段。
    // 推导规则：
    //   http://host[:port]   → ws://host[:port]/api/v1/hasn/ws/node
    //   https://host[:port]  → wss://host[:port]/api/v1/hasn/ws/node
    //   ws://* / wss://*     → 原样使用
    if node_config.hasn.central_url.is_none()
        && node_config.connection.server_url.is_none()
        && node_config.server.url.is_none()
    {
        let derived = derive_hasn_ws_url(config.huanxing.hasn_url());
        tracing::info!(
            base_url = config.huanxing.hasn_url(),
            ws_url = %derived,
            "[HASN] 从 [huanxing] hasn_base_url 推导 WS URL，覆盖 hasn-node 默认地址"
        );
        node_config.hasn.central_url = Some(derived);
    }
    if node_config.hasn.api_key.is_none()
        && node_config.connection.owner_api_key.is_none()
        && node_config.server.api_key.is_none()
    {
        if let Some(key) = config.huanxing.hasn.api_key.as_deref() {
            let trimmed = key.trim();
            if !trimmed.is_empty() {
                node_config.hasn.api_key = Some(trimmed.to_string());
            }
        }
    }

    let http_addr = format!("{}:{}", node_config.http.host, node_config.http.port);
    let node = Arc::new(Node::new(node_config)?);

    // 注册内置 spawner（ClaudeCode / Webhook）——桌面端 HTTP API
    // `/api/v1/hasn/node/agents` 等依赖这些 spawner 的存在。
    if let Err(err) = node.register_builtin_spawners().await {
        tracing::warn!(error = %err, "注册 hasn-node 内置 spawner 失败（非阻塞）");
    }

    if let Err(err) = node.load_spawners_from_db() {
        tracing::warn!(error = %err, "加载 hasn-node spawner 配置失败（非阻塞）");
    }

    // Phase 05-05 Task 2 — 初始化 hasn-node 全局 connector。后续 WS 建连、
    // send_message、add_owner、add_agent、入站 dispatch 全部走 hasn-node，
    // 不再走 legacy `zeroclaw_huanxing::hasn_connector::global_connector()`。
    // `init_global_connector` 幂等：二次调用返回 false，不影响已有 connector。
    let did_init = hasn_node::connector::init_global_connector(node.clone(), node.chat_db.clone());
    if did_init {
        tracing::info!("[HASN] hasn-node 全局 connector 初始化成功");
    } else {
        tracing::debug!("[HASN] hasn-node 全局 connector 已初始化过，跳过（幂等）");
    }

    // 计划 14.4 —— embedded 模式下 hasn-node 必须在 127.0.0.1:42618 监听，
    // 前端 `HASN_NODE_BASE` 直连此端口。仅 spawn 一次（进程级 flag 幂等）。
    spawn_embedded_http_server(node.clone(), http_addr);

    *embedded_node_slot().lock().unwrap() = Some(node.clone());
    Ok(node)
}

fn embedded_http_started() -> &'static std::sync::atomic::AtomicBool {
    static FLAG: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    &FLAG
}

/// 把 `hasn_base_url`（HTTP[S] 基址）转换为 HASN WS URL。
///
/// 规则：
/// - `http://host[:port][/path]` → `ws://host[:port]/api/v1/hasn/ws/node`
/// - `https://host[:port][/path]` → `wss://host[:port]/api/v1/hasn/ws/node`
/// - 已是 `ws://` / `wss://` 的原样保留
/// - 解析失败或空 → 沿用 hasn-node 的默认常量（由调用方兜底）
pub(crate) fn derive_hasn_ws_url(base: &str) -> String {
    const WS_PATH: &str = "/api/v1/hasn/ws/node";
    let trimmed = base.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return format!("wss://hasn.huanxing.dcfuture.cn{WS_PATH}");
    }
    if let Some(rest) = trimmed.strip_prefix("https://") {
        let host = rest.split('/').next().unwrap_or(rest);
        return format!("wss://{host}{WS_PATH}");
    }
    if let Some(rest) = trimmed.strip_prefix("http://") {
        let host = rest.split('/').next().unwrap_or(rest);
        return format!("ws://{host}{WS_PATH}");
    }
    if trimmed.starts_with("ws://") || trimmed.starts_with("wss://") {
        return trimmed.to_string();
    }
    // 无 scheme 兜底：按 https 处理
    format!("wss://{trimmed}{WS_PATH}")
}

#[cfg(test)]
mod url_tests {
    use super::derive_hasn_ws_url;

    #[test]
    fn http_to_ws() {
        assert_eq!(
            derive_hasn_ws_url("http://127.0.0.1:8020"),
            "ws://127.0.0.1:8020/api/v1/hasn/ws/node"
        );
    }

    #[test]
    fn https_to_wss() {
        assert_eq!(
            derive_hasn_ws_url("https://api.huanxing.dcfuture.cn"),
            "wss://api.huanxing.dcfuture.cn/api/v1/hasn/ws/node"
        );
    }

    #[test]
    fn strip_trailing_slash_and_path() {
        assert_eq!(
            derive_hasn_ws_url("http://127.0.0.1:8020/api/"),
            "ws://127.0.0.1:8020/api/v1/hasn/ws/node"
        );
    }

    #[test]
    fn ws_passthrough() {
        assert_eq!(
            derive_hasn_ws_url("ws://localhost:9000/custom/path"),
            "ws://localhost:9000/custom/path"
        );
    }

    #[test]
    fn empty_fallback() {
        assert_eq!(
            derive_hasn_ws_url(""),
            "wss://hasn.huanxing.dcfuture.cn/api/v1/hasn/ws/node"
        );
    }
}

fn spawn_embedded_http_server(node: Arc<Node>, addr: String) {
    use std::sync::atomic::Ordering;
    if embedded_http_started()
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        tracing::debug!(
            "[HASN] embedded HTTP server 已启动过（addr={}），跳过",
            addr
        );
        return;
    }

    tokio::spawn(async move {
        let api_state = hasn_node::api::ApiState {
            node: node.clone(),
            chat_db: node.chat_db.clone(),
        };
        let router = hasn_node::http::build_router(api_state);
        let listener = match tokio::net::TcpListener::bind(&addr).await {
            Ok(l) => l,
            Err(err) => {
                embedded_http_started().store(false, std::sync::atomic::Ordering::SeqCst);
                tracing::error!(
                    error = %err,
                    addr = %addr,
                    "[HASN] embedded hasn-node HTTP bind 失败"
                );
                return;
            }
        };
        tracing::info!("[HASN] embedded hasn-node HTTP 启动于 {}", addr);
        if let Err(err) = axum::serve(listener, router).await {
            tracing::error!(error = %err, "[HASN] embedded hasn-node HTTP 异常退出");
        }
    });
}

/// 唤星原生 Spawner —— 复用既有桥接逻辑的薄 trait 壳
pub struct HuanxingNativeSpawner {
    config: Config,
    session_backend: Option<Arc<dyn SessionBackend>>,
    /// hasn-node 全局 ChatStorage（M2.5 双写目标，M3 后单写）
    hasn_chat: Option<Arc<hasn_node::chat_db::ChatStorage>>,
    sessions: Arc<RwLock<HashMap<String, Arc<HasnAgentSession>>>>,
}

impl HuanxingNativeSpawner {
    pub fn new(
        config: Config,
        session_backend: Option<Arc<dyn SessionBackend>>,
        hasn_chat: Option<Arc<hasn_node::chat_db::ChatStorage>>,
    ) -> Self {
        Self {
            config,
            session_backend,
            hasn_chat,
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn config_dir(&self) -> std::path::PathBuf {
        self.config
            .config_path
            .parent()
            .unwrap_or(&self.config.workspace_dir)
            .to_path_buf()
    }

    fn fallback_chat_db_path(&self) -> std::path::PathBuf {
        self.config_dir().join("data").join("hasn_chat.db")
    }

    async fn chat_db_for_hasn_id(&self, hasn_id: &str) -> anyhow::Result<HasnChatDb> {
        let config_dir = self.config_dir();
        let db_path = self.config.huanxing.resolve_db_path(&config_dir);
        let chat_db_path = match crate::db::TenantDb::open(&db_path) {
            Ok(db) => match db.find_by_hasn_id(hasn_id).await {
                Ok(Some(record)) => self
                    .config
                    .huanxing
                    .resolve_tenant_root(&config_dir, record.tenant_dir.as_deref())
                    .join("data")
                    .join("hasn_chat.db"),
                _ => self.fallback_chat_db_path(),
            },
            Err(_) => self.fallback_chat_db_path(),
        };
        HasnChatDb::open(&chat_db_path)
    }

    fn bridge(&self, chat_db: HasnChatDb) -> HasnAgentBridge {
        HasnAgentBridge::new(
            self.config.clone(),
            self.session_backend.clone(),
            chat_db,
            self.hasn_chat.clone(),
            self.sessions.clone(),
        )
    }
}

impl Default for HuanxingNativeSpawner {
    fn default() -> Self {
        let runtime = current_runtime().expect("huanxing native runtime must be configured");
        Self::new(runtime.config, runtime.session_backend, None)
    }
}

pub async fn register_huanxing_native_spawner(node: Arc<Node>) -> anyhow::Result<()> {
    let runtime = current_runtime()?;
    let spawner = Arc::new(HuanxingNativeSpawner::new(
        runtime.config,
        runtime.session_backend,
        Some(node.chat_db.clone()),
    ));
    node.register_spawner(spawner).await
}

#[async_trait]
impl AgentSpawner for HuanxingNativeSpawner {
    fn name(&self) -> &str {
        "huanxing_native"
    }

    async fn dispatch(&self, ctx: InboundContext) -> anyhow::Result<mpsc::Receiver<ReplyChunk>> {
        let chat_db = self.chat_db_for_hasn_id(&ctx.agent_hasn_id).await?;
        self.bridge(chat_db).dispatch_to_reply_chunks(&ctx).await
    }

    async fn probe(&self) -> bool {
        let config_dir = self.config_dir();
        self.config.huanxing.enabled
            && config_dir.exists()
            && self
                .config
                .huanxing
                .resolve_db_path(&config_dir)
                .parent()
                .map(std::path::Path::exists)
                .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;
    use std::time::Duration;

    use axum::{
        Router,
        extract::{
            State,
            ws::{Message, WebSocketUpgrade},
        },
        response::IntoResponse,
        routing::get,
    };
    use futures_util::{SinkExt, StreamExt};
    use serde_json::json;
    use tokio::sync::{Mutex, mpsc as unbounded_mpsc};

    use hasn_node::config::NodeConfig;
    use hasn_node::connector::HasnConnector;
    use hasn_node::spawner::SpawnerRegistry;
    use zeroclaw_infra::session_backend::SessionBackend;
    use zeroclaw_providers::ChatMessage;

    #[derive(Default)]
    struct MockHasnState {
        outbound_frames: Arc<Mutex<Vec<serde_json::Value>>>,
        inbound_tx: Arc<Mutex<Option<unbounded_mpsc::UnboundedSender<String>>>>,
    }

    fn test_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| std::sync::Mutex::new(())).lock().unwrap()
    }

    fn test_config(config_dir: &std::path::Path) -> Config {
        let mut config = Config::default();
        config.huanxing.enabled = true;
        config.config_path = config_dir.join("config.toml");
        config.workspace_dir = config_dir.join("workspace");
        config.knowledge.enabled = false;
        config
    }

    fn write_node_config(config: &Config, data_dir: &std::path::Path) {
        std::fs::create_dir_all(data_dir).unwrap();
        std::fs::write(
            &config.config_path,
            format!(
                "[node]\ndata_dir = \"{}\"\n\n[connection]\nowner_api_key = \"hasn_ok_test_owner\"\n",
                data_dir.display()
            ),
        )
        .unwrap();
    }

    async fn create_workspace_tree(
        config_dir: &std::path::Path,
        tenant_dir: &str,
        agent_id: &str,
    ) -> (std::path::PathBuf, std::path::PathBuf) {
        let owner_dir = config_dir.join("users").join(tenant_dir).join("workspace");
        let agent_wrapper = config_dir
            .join("users")
            .join(tenant_dir)
            .join("agents")
            .join(agent_id);
        let agent_workspace = agent_wrapper.join("workspace");
        tokio::fs::create_dir_all(&owner_dir).await.unwrap();
        tokio::fs::create_dir_all(&agent_workspace).await.unwrap();
        tokio::fs::write(owner_dir.join("USER.md"), "# User\n")
            .await
            .unwrap();
        tokio::fs::write(agent_workspace.join("SOUL.md"), "# Soul\n")
            .await
            .unwrap();
        tokio::fs::write(
            agent_wrapper.join("config.toml"),
            format!(
                "display_name = \"{}\"\n[agent]\nhasn_id = \"pending\"\n",
                agent_id
            ),
        )
        .await
        .unwrap();
        (owner_dir, agent_workspace)
    }

    async fn seed_tenant(
        config_dir: &std::path::Path,
        tenant_dir: &str,
        agent_id: &str,
        owner_hasn_id: &str,
        agent_hasn_id: &str,
    ) {
        let db_path = config_dir.join("data").join("users.db");
        let db = crate::db::TenantDb::open(&db_path).unwrap();
        db.save_user_full(
            "user-1",
            "13800000000",
            agent_id,
            Some("Tester"),
            "assistant",
            Some("Star"),
            None,
            Some(tenant_dir),
            Some(owner_hasn_id),
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
        assert!(db.update_agent_hasn_id(agent_id, agent_hasn_id).await.unwrap());
    }

    fn build_node_config(data_dir: &std::path::Path) -> NodeConfig {
        let mut config = NodeConfig::default();
        config.node.data_dir = Some(data_dir.display().to_string());
        config.connection.owner_api_key = Some("hasn_ok_test_owner".to_string());
        config
    }

    fn owner_setup(node: &Arc<Node>, owner_id: &str) {
        node.db
            .upsert_owner(owner_id, Some(owner_id), Some("Demo Owner"), Some("hasn_ok_test"))
            .unwrap();
    }

    fn inbound_context(agent_hasn_id: &str, conversation_id: &str, from_id: &str) -> InboundContext {
        InboundContext {
            owner_id: "h_owner_demo".to_string(),
            agent_hasn_id: agent_hasn_id.to_string(),
            conversation_id: conversation_id.to_string(),
            from_hasn_id: from_id.to_string(),
            user_message: "hello from hasn".to_string(),
            local_agent: hasn_core::model::local::DispatchLocalAgent {
                local_key: format!("huanxing_native::{agent_hasn_id}"),
                owner_id: "h_owner_demo".to_string(),
                hasn_id: Some(agent_hasn_id.to_string()),
                agent_name: "default".to_string(),
                display_name: "Default Agent".to_string(),
                source_type: "huanxing_native".to_string(),
                role: Some("assistant".to_string()),
                workspace_path: None,
                native_project_path: None,
                system_prompt_path: None,
                metadata_json: serde_json::json!({"tenant_dir": "001-13800000000"}),
            },
            raw: serde_json::json!({
                "id": 1,
                "conversation_id": conversation_id,
                "from_id": from_id,
                "to_id": agent_hasn_id,
                "content": { "text": "hello from hasn" },
                "content_type": 1,
                "created_time": "2026-04-19T00:00:00Z",
                "self_sent": false
            }),
        }
    }

    async fn drain_chunks(mut rx: mpsc::Receiver<ReplyChunk>) -> Vec<ReplyChunk> {
        let mut chunks = Vec::new();
        while let Ok(Some(chunk)) = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await {
            let done = matches!(chunk, ReplyChunk::Done);
            chunks.push(chunk);
            if done {
                break;
            }
        }
        chunks
    }

    struct MemoryBackend {
        inner: Arc<std::sync::Mutex<HashMap<String, Vec<ChatMessage>>>>,
    }

    impl Default for MemoryBackend {
        fn default() -> Self {
            Self {
                inner: Arc::new(std::sync::Mutex::new(HashMap::new())),
            }
        }
    }

    impl SessionBackend for MemoryBackend {
        fn load(&self, session_key: &str) -> Vec<ChatMessage> {
            self.inner
                .lock()
                .unwrap()
                .get(session_key)
                .cloned()
                .unwrap_or_default()
        }

        fn append(&self, session_key: &str, message: &ChatMessage) -> std::io::Result<()> {
            self.inner
                .lock()
                .unwrap()
                .entry(session_key.to_string())
                .or_default()
                .push(message.clone());
            Ok(())
        }

        fn remove_last(&self, session_key: &str) -> std::io::Result<bool> {
            let removed = self
                .inner
                .lock()
                .unwrap()
                .get_mut(session_key)
                .and_then(Vec::pop)
                .is_some();
            Ok(removed)
        }

        fn list_sessions(&self) -> Vec<String> {
            self.inner.lock().unwrap().keys().cloned().collect()
        }
    }

    async fn spawn_mock_hasn_server() -> (String, Arc<MockHasnState>) {
        let state = Arc::new(MockHasnState::default());

        async fn ws_handler(
            State(state): State<Arc<MockHasnState>>,
            ws: WebSocketUpgrade,
        ) -> impl IntoResponse {
            ws.on_upgrade(move |socket| async move {
                let (mut sender, mut receiver) = socket.split();
                let (server_tx, mut server_rx) = unbounded_mpsc::unbounded_channel::<String>();
                *state.inbound_tx.lock().await = Some(server_tx);

                let sender_task = tokio::spawn(async move {
                    while let Some(text) = server_rx.recv().await {
                        if sender.send(Message::Text(text.into())).await.is_err() {
                            break;
                        }
                    }
                });

                let connected = json!({
                    "hasn": "hasn/2.0",
                    "method": "hasn.connected",
                    "params": {
                        "node_id": "n_test_runtime",
                        "node_type": "desktop",
                        "server_time": "2026-04-19T00:00:00Z",
                        "owner_id": "h_owner_demo",
                        "owner_count": 1,
                        "agent_count": 0
                    }
                });
                let _ = state_send_frame(&state, connected).await;

                while let Some(Ok(Message::Text(text))) = receiver.next().await {
                    let frame: serde_json::Value = serde_json::from_str(&text).unwrap();
                    state.outbound_frames.lock().await.push(frame);
                }

                sender_task.abort();
            })
        }

        let router = Router::new()
            .route("/api/v1/hasn/ws/node", get(ws_handler))
            .with_state(state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });

        (format!("ws://{addr}/api/v1/hasn/ws/node"), state)
    }

    async fn wait_for<F, Fut>(timeout: Duration, mut predicate: F)
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = bool>,
    {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if predicate().await {
                return;
            }
            assert!(tokio::time::Instant::now() < deadline, "condition timed out");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    async fn state_send_frame(
        state: &Arc<MockHasnState>,
        frame: serde_json::Value,
    ) -> anyhow::Result<()> {
        wait_for(Duration::from_secs(3), || {
            let state = state.clone();
            async move { state.inbound_tx.lock().await.is_some() }
        })
        .await;

        let sender = state
            .inbound_tx
            .lock()
            .await
            .clone()
            .expect("inbound sender should exist");
        sender.send(frame.to_string())?;
        Ok(())
    }

    async fn push_inbound_message(
        state: &Arc<MockHasnState>,
        message_id: i64,
        conversation_id: &str,
        to_id: &str,
        text: &str,
        from_owner_id: Option<&str>,
        to_owner_id: Option<&str>,
    ) {
        let frame = json!({
            "hasn": "hasn/2.0",
            "method": "hasn.message.received",
            "params": {
                "to_id": to_id,
                "message": {
                    "id": message_id,
                    "conversation_id": conversation_id,
                    "from_id": "u_sender",
                    "from_type": 1,
                    "to_id": to_id,
                    "content": { "text": text },
                    "content_type": 1,
                    "created_time": "2026-04-19T00:00:00Z",
                    "self_sent": false,
                    "from_owner_id": from_owner_id,
                    "to_owner_id": to_owner_id
                }
            }
        });
        state_send_frame(state, frame).await.unwrap();
    }

    async fn connect_test_connector(node: Arc<Node>, ws_url: &str) -> Arc<HasnConnector> {
        let connector = Arc::new(HasnConnector::new(node.clone(), node.chat_db.clone()));
        let params = node.build_v21_connect_params(Some(ws_url), None).unwrap();
        connector
            .connect(&params.url, params.auth_headers)
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(250)).await;
        connector
    }

    async fn wait_for_add_agent(state: &Arc<MockHasnState>, hasn_id: &str) {
        wait_for(Duration::from_secs(3), || {
            let state = state.clone();
            let hasn_id = hasn_id.to_string();
            async move {
                state.outbound_frames.lock().await.iter().any(|frame| {
                    frame["method"] == "hasn.node.add_agent"
                        && frame["params"]["agent_id"] == hasn_id
                })
            }
        })
        .await;
    }

    fn collect_sent_texts(frames: &[serde_json::Value], msg_type: &str) -> Vec<String> {
        frames
            .iter()
            .filter(|frame| {
                frame["method"] == "hasn.message.send" && frame["params"]["type"] == msg_type
            })
            .filter_map(|frame| {
                frame["params"]["content"]["text"]
                    .as_str()
                    .map(ToOwned::to_owned)
            })
            .collect()
    }

    #[tokio::test]
    async fn test_register_huanxing_native_spawner() {
        let _guard = test_lock();
        let temp = tempfile::tempdir().unwrap();
        let config = test_config(temp.path());
        write_node_config(&config, &temp.path().join("hasn-node"));
        configure_huanxing_native_runtime(config.clone(), None);

        let node = Arc::new(Node::new(build_node_config(&temp.path().join("hasn-node"))).unwrap());
        register_huanxing_native_spawner(node.clone()).await.unwrap();

        assert!(node.spawner_registry.read().await.get("huanxing_native").is_some());
    }

    #[tokio::test]
    async fn silent_drop_stays_policy_only() {
        let temp = tempfile::tempdir().unwrap();
        let config = test_config(temp.path());
        write_node_config(&config, &temp.path().join("hasn-node"));
        let tenant_dir = "001-13800000000";
        let agent_id = "default";
        let agent_hasn_id = "a_huanxing_agent_1";
        create_workspace_tree(temp.path(), tenant_dir, agent_id).await;
        seed_tenant(temp.path(), tenant_dir, agent_id, "h_owner_demo", agent_hasn_id).await;

        let chat_db = HasnChatDb::open(
            &temp
                .path()
                .join("users")
                .join(tenant_dir)
                .join("data")
                .join("hasn_chat.db"),
        )
        .unwrap();
        chat_db
            .upsert_contact(&crate::hasn_bridge::chat_db::ContactRecord {
                hasn_id: "u_blocked".to_string(),
                nickname: Some("Blocked".to_string()),
                avatar_url: None,
                contact_type: "human".to_string(),
                relation_type: "social".to_string(),
                trust_level: 0,
                status: "blocked".to_string(),
                created_at: "2026-04-19 00:00:00".to_string(),
            })
            .await
            .unwrap();

        let spawner = HuanxingNativeSpawner::new(config, None, None);
        let rx = spawner
            .dispatch(inbound_context(agent_hasn_id, "c_silent", "u_blocked"))
            .await
            .unwrap();
        let chunks = drain_chunks(rx).await;

        assert!(chunks.is_empty());
        assert_eq!(spawner.sessions.read().await.len(), 0);
    }

    #[tokio::test]
    async fn tenant_lookup_failure_surfaces_error_chunk() {
        let temp = tempfile::tempdir().unwrap();
        let config = test_config(temp.path());
        write_node_config(&config, &temp.path().join("hasn-node"));

        let spawner = HuanxingNativeSpawner::new(config, None, None);
        let mut rx = spawner
            .dispatch(inbound_context("a_missing_agent", "c_missing", "u_sender"))
            .await
            .unwrap();

        let chunk = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .unwrap()
            .expect("error chunk expected");
        assert!(matches!(
            chunk,
            ReplyChunk::Error(ref err) if err == "huanxing dispatch failed: tenant lookup"
        ));
    }

    #[tokio::test]
    async fn dispatch_reuses_session_for_same_conversation() {
        let temp = tempfile::tempdir().unwrap();
        let config = test_config(temp.path());
        write_node_config(&config, &temp.path().join("hasn-node"));
        let tenant_dir = "001-13800000000";
        let agent_id = "default";
        let agent_hasn_id = "a_huanxing_agent_1";
        create_workspace_tree(temp.path(), tenant_dir, agent_id).await;
        seed_tenant(temp.path(), tenant_dir, agent_id, "h_owner_demo", agent_hasn_id).await;

        let backend: Arc<dyn SessionBackend> = Arc::new(MemoryBackend::default());
        let spawner = HuanxingNativeSpawner::new(config, Some(backend), None);

        let rx1 = spawner
            .dispatch(inbound_context(agent_hasn_id, "c_reuse", "u_sender"))
            .await
            .unwrap();
        wait_for(Duration::from_secs(3), || {
            let sessions = spawner.sessions.clone();
            async move { sessions.read().await.contains_key("c_reuse") }
        })
        .await;
        let first_session = spawner
            .sessions
            .read()
            .await
            .get("c_reuse")
            .cloned()
            .expect("first session should exist");
        let _ = drain_chunks(rx1).await;

        let rx2 = spawner
            .dispatch(inbound_context(agent_hasn_id, "c_reuse", "u_sender"))
            .await
            .unwrap();
        wait_for(Duration::from_secs(3), || {
            let sessions = spawner.sessions.clone();
            async move { sessions.read().await.len() == 1 }
        })
        .await;
        let second_session = spawner
            .sessions
            .read()
            .await
            .get("c_reuse")
            .cloned()
            .expect("second session should exist");
        let _ = drain_chunks(rx2).await;

        assert!(Arc::ptr_eq(&first_session, &second_session));
        assert_eq!(spawner.sessions.read().await.len(), 1);
    }

    #[tokio::test]
    async fn onboarded_huanxing_row_dispatches_to_huanxing_native_spawner() {
        let _guard = test_lock();
        let temp = tempfile::tempdir().unwrap();
        let (ws_url, state) = spawn_mock_hasn_server().await;
        let config = test_config(temp.path());
        let hasn_data_dir = temp.path().join("hasn-node");
        write_node_config(&config, &hasn_data_dir);
        let tenant_dir = "001-13800000000";
        let agent_id = "default";
        let agent_hasn_id = "a_huanxing_agent_1";
        create_workspace_tree(temp.path(), tenant_dir, agent_id).await;
        seed_tenant(temp.path(), tenant_dir, agent_id, "h_owner_demo", agent_hasn_id).await;
        configure_huanxing_native_runtime(config.clone(), None);

        let mut node_config = build_node_config(&hasn_data_dir);
        node_config.connection.server_url = Some(ws_url.clone());
        let node = Arc::new(Node::new(node_config).unwrap());
        owner_setup(&node, "h_owner_demo");
        crate::api_agents::upsert_huanxing_native_local_agent(&config, agent_id, agent_hasn_id)
            .await
            .unwrap();
        register_huanxing_native_spawner(node.clone()).await.unwrap();

        let local_agent = node
            .db
            .find_active_local_agent_by_hasn_id(agent_hasn_id)
            .unwrap()
            .expect("mirrored local agent row should exist");
        assert_eq!(local_agent.source_type, "huanxing_native");

        let _connector = connect_test_connector(node.clone(), &ws_url).await;
        wait_for_add_agent(&state, agent_hasn_id).await;

        push_inbound_message(
            &state,
            1,
            "c_onboarded",
            agent_hasn_id,
            "hello huanxing",
            Some("h_wrong_from"),
            Some("h_wrong_to"),
        )
        .await;

        wait_for(Duration::from_secs(3), || {
            let state = state.clone();
            async move {
                collect_sent_texts(&state.outbound_frames.lock().await, "error")
                    .iter()
                    .any(|text| {
                        text.starts_with("huanxing dispatch failed:")
                            || text == "busy, retry later"
                    })
            }
        })
        .await;

        let frames = state.outbound_frames.lock().await.clone();
        let error_texts = collect_sent_texts(&frames, "error");
        assert!(error_texts.iter().any(|text| {
            text.starts_with("huanxing dispatch failed:") || text == "busy, retry later"
        }));
        assert!(!error_texts.iter().any(|text| {
            text.contains("spawner 'huanxing_native' not registered")
        }));
    }

    #[test]
    fn spawner_name_is_stable() {
        let mut registry = SpawnerRegistry::new();
        let spawner = Arc::new(HuanxingNativeSpawner::new(Config::default(), None, None));
        registry.register(spawner);
        assert!(registry.get("huanxing_native").is_some());
        assert_eq!(registry.list_names(), vec!["huanxing_native"]);
    }
}
