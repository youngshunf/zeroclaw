//! Phase 05-05 / 05-06 — HASN cutover 冒烟测试。
//!
//! 本测试验证三个 cutover 不变式（**不连真实中央服务器**）：
//!
//! 1. **peer_id 迁移正确性**（05-05）：模拟历史污染场景（sessions.peer_id 写成
//!    Owner 的 `h_*`），调 `HasnChatDb::run_migration_phase_05_05_peer_id` 后应把
//!    peer_id 修正为对端 Agent 的 `a_*`；再次调用（幂等）应早退并返回 0 行。
//! 2. **legacy connector 未被误触发**（05-05）：读 `hasn_connector::test_harness`
//!    计数器，确认从进程启动到测试结束 `connect` / `handle_ws_frame` 都为 0。
//! 3. **outbound LocalLoopback 跨 crate 合同**（05-06 Gap #3）：zeroclaw 仓通过
//!    hasn-node 的 public API 调 `HasnConnector::send_message`，同 owner 两本地
//!    实体应**不经 ws.send_frame** 直接走 MessageRouter::dispatch + event broadcast；
//!    返回 Ok 且 `/ws/hasn-events` 订阅方收到 `HasnEvent::Message { local_loopback }`。
//!
//! 退化说明（Plan Rule 3 — 解除 blocking issue）：原 05-06 Task 3 建议的全量启动
//! 路径（`initialize_embedded_huanxing_node` + `register_huanxing_native_spawner`）
//! 依赖 zeroclaw tenant/workspace 基础设施 helper（`seed_test_tenant_and_agent` /
//! `open_tenant_chat_db` / `make_test_config`）——它们在当前 codebase 不存在；且
//! `initialize_embedded_huanxing_node` 会触碰进程级 `OnceLock<HasnConnector>`，
//! 与 hasn-node 其它测试或未来 smoke 串跑会污染全局状态。改用 hasn-node 的
//! public API 直接构建 `HasnConnector::new` + `Node::new`（不走 OnceLock），聚焦
//! outbound LocalLoopback 的**跨 crate 合同**（API 可达性 + 语义一致性）；完整
//! Spawner 驱动路径已由 hasn-node 单元测试覆盖（Task 1 的
//! `build_local_loopback_payload_dispatches_without_errors` + Task 2 的
//! `outbound_same_owner_skips_ws`），无重复覆盖缺口。

use zeroclaw_huanxing::hasn_bridge::chat_db::{ChatMessageRecord, HasnChatDb};
use zeroclaw_huanxing::hasn_bridge::connector::test_harness as legacy_harness;

#[tokio::test]
async fn owner_to_own_agent_roundtrip_is_local_only() {
    // 在临时目录里建一个 per-tenant hasn_chat.db
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("data").join("hasn_chat.db");
    let chat_db = HasnChatDb::open(&db_path).expect("open hasn_chat.db");

    // ── 模拟历史污染：Owner → 自家 Agent 会话 peer_id 被错写为 Owner 自己 ──
    let owner_hasn = "h_owner_cutover";
    let agent_hasn = "a_agent_cutover";
    let conv_id = "c_cutover_loop";

    // 先插一条 messages（receiver_id = Agent），让迁移 SELECT 能命中 a_*
    let msg = ChatMessageRecord {
        id: 0,
        message_id: "msg_cutover_1".to_string(),
        conversation_id: conv_id.to_string(),
        sender_id: owner_hasn.to_string(),
        receiver_id: agent_hasn.to_string(), // ← peer 候选
        content_type: "text".to_string(),
        content: r#"{"text":"hi from cutover"}"#.to_string(),
        status: "delivered".to_string(),
        is_outgoing: false,
        created_at: "2026-04-20 10:00:00".to_string(),
    };
    chat_db.insert_message(&msg).await.expect("insert_message");

    // 模拟老逻辑下的错误 peer：直接 upsert_session 传 Owner 作为 peer
    chat_db
        .upsert_session(conv_id, "p2p", owner_hasn)
        .await
        .expect("seed bad session peer_id");

    // 迁移前 peer_id 应为 owner_hasn（污染）
    let sessions_before = chat_db.get_sessions().await.expect("get_sessions before");
    let peer_before = sessions_before
        .iter()
        .find(|s| s.conversation_id == conv_id)
        .map(|s| s.peer_id.clone())
        .expect("seed session exists");
    assert_eq!(peer_before, owner_hasn, "迁移前 peer_id 应是污染的 h_*");

    // ── 执行迁移：第一次应修正 1 行 ──
    let n1 = chat_db
        .run_migration_phase_05_05_peer_id()
        .await
        .expect("first migration ok");
    assert_eq!(n1, 1, "第一次 migration 应修正 1 行 sessions.peer_id");

    let sessions_after = chat_db.get_sessions().await.expect("get_sessions after");
    let peer_after = sessions_after
        .iter()
        .find(|s| s.conversation_id == conv_id)
        .map(|s| s.peer_id.clone())
        .expect("session still exists");
    assert_eq!(
        peer_after, agent_hasn,
        "迁移后 peer_id 应修正为对端 Agent 的 a_*"
    );

    // ── 幂等性：再调一次应返回 0 行 ──
    let n2 = chat_db
        .run_migration_phase_05_05_peer_id()
        .await
        .expect("second migration ok");
    assert_eq!(n2, 0, "第二次 migration 应幂等早退（sync_state 已标记 done）");

    // ── legacy connector cutover 断言：从进程启动到这里，legacy WS 入口从未被触发 ──
    assert!(
        !legacy_harness::was_connect_called(),
        "Phase 05-05 cutover: legacy HasnConnector::connect 不应被调用 (calls={})",
        legacy_harness::connect_call_count()
    );
    assert!(
        !legacy_harness::was_handle_ws_frame_called(),
        "Phase 05-05 cutover: legacy handle_ws_frame 不应被调用 (calls={})",
        legacy_harness::handle_ws_frame_call_count()
    );
}

// ════════════════════════════════════════════════════════════════
// Phase 05-06 — outbound LocalLoopback 跨 crate 合同 smoke
// ════════════════════════════════════════════════════════════════

/// Gap #3 根因修复后的跨 crate 合同验证：
///
/// zeroclaw 侧调用 hasn-node 的 `HasnConnector::send_message`（public API）
/// 时，同 owner 两本地实体触发 outbound LocalLoopback：
/// - 不调 `ws.send_frame`（未连接 ws 也不抛 Err）
/// - 通过 `event_tx` 广播 `HasnEvent::Message { local_loopback=true }`
///
/// 锁定 API 可达性（zeroclaw 能调到）+ 语义一致性（行为与 hasn-node 单测对齐）。
#[tokio::test]
async fn owner_to_own_agent_outbound_skips_ws() {
    use hasn_node::config::NodeConfig;
    use hasn_node::connector::{HasnConnector, HasnEvent};
    use hasn_node::node::Node;
    use std::sync::Arc;

    // 1) 构造独立 Node（不触碰 hasn-node 进程级 OnceLock<HasnConnector>）
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut node_config = NodeConfig::default();
    node_config.node.data_dir = Some(tmp.path().to_str().unwrap().to_string());
    let node = Arc::new(Node::new(node_config).expect("Node::new"));

    // 2) seed 同 owner 两本地实体（owner human + agent）
    let owner_id = "owner_zc_outbound";
    let owner_hasn = "h_zc_outbound";
    let agent_hasn = "a_zc_outbound";
    node.db
        .upsert_owner(owner_id, None, Some("ZeroClaw Test Owner"), None)
        .expect("upsert_owner");
    node.db
        .upsert_local_agent(
            "zc::outbound::owner",
            owner_id,
            Some(owner_hasn),
            "owner_entity",
            "Owner Entity",
            "owner_shim",
            Some("human"),
            None,
            None,
            None,
            "{}",
        )
        .expect("upsert owner entity");

    // 3) 注册一个 echo spawner —— 收到 ctx 后发一条 Text + Done
    use async_trait::async_trait;
    use hasn_node::spawner::{AgentSpawner, InboundContext, ReplyChunk};
    use tokio::sync::mpsc;

    struct EchoSpawner;
    #[async_trait]
    impl AgentSpawner for EchoSpawner {
        fn name(&self) -> &str {
            "test_spawner_zc_echo"
        }
        async fn dispatch(
            &self,
            _ctx: InboundContext,
        ) -> anyhow::Result<mpsc::Receiver<ReplyChunk>> {
            let (tx, rx) = mpsc::channel(4);
            let _ = tx.send(ReplyChunk::Text("zc reply".into())).await;
            let _ = tx.send(ReplyChunk::Done).await;
            Ok(rx)
        }
        async fn probe(&self) -> bool {
            true
        }
    }
    node.register_spawner(Arc::new(EchoSpawner))
        .await
        .expect("register spawner");

    node.db
        .upsert_local_agent(
            "zc::outbound::agent",
            owner_id,
            Some(agent_hasn),
            "agent_entity",
            "Agent Entity",
            "test_spawner_zc_echo",
            Some("assistant"),
            None,
            None,
            None,
            "{}",
        )
        .expect("upsert agent entity");

    // 4) 构造 HasnConnector（ws 未连接）并订阅事件
    let connector = HasnConnector::new(node.clone(), node.chat_db.clone());
    let mut rx = connector.subscribe();

    // 5) 调 send_message —— 同 owner 两实体应走 outbound LocalLoopback
    let result = connector
        .send_message(
            agent_hasn,
            serde_json::json!({"text": "你好"}),
            Some(owner_hasn.to_string()),
            Some("zc_loc_1".to_string()),
        )
        .await;
    assert!(
        result.is_ok(),
        "outbound LocalLoopback 应不调 ws.send_frame → 返回 Ok；实际 {:?}",
        result
    );

    // 6) event_tx 广播 HasnEvent::Message { payload.local_loopback=true }
    let payload = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            match rx.recv().await {
                Ok(HasnEvent::Message { payload }) => return Some(payload),
                Ok(_) => continue,
                Err(_) => return None,
            }
        }
    })
    .await
    .expect("5s 未收到 HasnEvent 广播 → outbound LocalLoopback 分支未触发 spawner")
    .expect("broadcast channel 已关闭");

    assert_eq!(
        payload.get("local_loopback").and_then(|v| v.as_bool()),
        Some(true),
        "广播 payload 应带 local_loopback=true; payload={payload}"
    );
    assert_eq!(
        payload.get("from_id").and_then(|v| v.as_str()),
        Some(agent_hasn),
        "广播 from_id 应为回复方 agent"
    );
    assert_eq!(
        payload.get("to_id").and_then(|v| v.as_str()),
        Some(owner_hasn),
        "广播 to_id 应为原发送 owner"
    );

    // 7) legacy 入口依旧不应被触发（回归保护）
    assert!(
        !legacy_harness::was_connect_called(),
        "05-06 smoke 不应触发 legacy connect (calls={})",
        legacy_harness::connect_call_count()
    );
}
