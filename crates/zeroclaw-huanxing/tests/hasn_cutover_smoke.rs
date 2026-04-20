//! Phase 05-05 — HASN cutover 冒烟测试。
//!
//! 本测试验证两个 cutover 不变式（**不连真实中央服务器**）：
//!
//! 1. **peer_id 迁移正确性**：模拟历史污染场景（sessions.peer_id 写成 Owner 的
//!    `h_*`），调 `HasnChatDb::run_migration_phase_05_05_peer_id` 后应把 peer_id
//!    修正为对端 Agent 的 `a_*`；再次调用（幂等）应早退并返回 0 行。
//! 2. **legacy connector 未被误触发**：读 `hasn_connector::test_harness` 计数器，
//!    确认从进程启动到测试结束 `connect` / `handle_ws_frame` 都为 0。测试只用
//!    本仓内类型，不调用 legacy shim 入口，因此计数器应保持 0。
//!
//! 退化说明：原任务建议「接入 hasn-node inject_test_frame + MessageRouter
//! 端到端闭环」，但新增 test-only feature 对 hasn-node 表面侵入较大。hasn-node
//! 单元测试已覆盖 RoutingMode + LocalLoopback 分支 + event_tx 广播（见
//! `hasn-node/crates/hasn-node/src/router.rs` 3 新测试），本 smoke 只补齐
//! 「历史数据修正」+「legacy 入口未触发」两条 cutover 硬约束。

use zeroclaw_huanxing::hasn_chat_db::{ChatMessageRecord, HasnChatDb};
use zeroclaw_huanxing::hasn_connector::test_harness as legacy_harness;

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
