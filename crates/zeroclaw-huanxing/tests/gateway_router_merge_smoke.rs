//! Phase 05-04b gap-closure smoke test.
//!
//! 固化 Plan B3 不变式：`huanxing_routes()` 的所有 `.route()` path 必须与上游
//! `zeroclaw-gateway` 已注册的 `/api/sessions*` 命名空间严格不相交，否则
//! `huanxing-zeroclaw/src/main.rs:1682` 启动时 `router.merge(...)` 会 panic
//! （`Overlapping method route. Handler for 'X /api/sessions...' already exists`），
//! 导致 sidecar 进程 `tokio-rt-worker` 崩溃，桌面端所有 `/api/*` 请求 500。
//!
//! 本测试模拟上游 gateway 已注册的 3 条冲突 (method, path)，再 merge
//! `huanxing_routes()`。只要有任何 path 字面量重新落回 `/api/sessions*`
//! 命名空间，该测试会在编译后运行时 panic，CI 立刻红。
//!
//! 该测试 **不构造 `AppState` 实例**，只在类型层面验证 axum `Router::merge` 的
//! path/method 冲突检测；因此不需要 runtime 依赖（DB / Provider / Memory 等）。

use axum::{
    Router,
    extract::State,
    routing::{delete, get, post, put},
};
use zeroclaw_gateway::AppState;
use zeroclaw_huanxing::gateway_routes::huanxing_routes;

async fn stub(State(_): State<AppState>) -> &'static str {
    "ok"
}

/// 上游 `zeroclaw-gateway/src/lib.rs:1024-1030` 已注册的 session routes。
fn upstream_session_routes() -> Router<AppState> {
    Router::new()
        .route("/api/sessions", get(stub))
        .route("/api/sessions/running", get(stub))
        .route("/api/sessions/{id}", put(stub).delete(stub))
        .route("/api/sessions/{id}/state", get(stub))
}

#[test]
fn gateway_starts_without_router_merge_panic() {
    // 若 huanxing_routes() 中重新引入任何 `/api/sessions*` (method, path)，
    // 下面这一行会 panic —— 正是 Phase 05-04 bug 的直接复现。
    let _merged = upstream_session_routes().merge(huanxing_routes());
}

#[test]
fn huanxing_sessions_namespace_registers_cleanly() {
    // 独立构建 huanxing_routes()，验证自身无内部冲突（POST+GET 共享同一 path
    // 是合法的，axum 区分 method；但若同 method 重复会 panic）。
    let _r = huanxing_routes();
}

/// Negative-control：手工复现 Phase 05-04 bug，证明该测试能捕获冲突。
///
/// 预期：本测试 **必须 panic**（`#[should_panic]`）；若不 panic，说明 axum
/// 的 merge 语义已变，需重新评估本文件两个正向测试的有效性。
#[test]
#[should_panic(expected = "Overlapping method route")]
fn sanity_router_merge_panics_on_conflict() {
    let a: Router<AppState> = Router::new().route("/api/sessions", get(stub));
    let b: Router<AppState> = Router::new().route("/api/sessions", get(stub));
    let _ = a.merge(b);
}

/// 另一个 negative-control：同 path 上重复 `DELETE` method（Phase 05-04
/// 实际 panic 的精确形状）。
#[test]
#[should_panic(expected = "Overlapping method route")]
fn sanity_router_merge_panics_on_delete_conflict() {
    let a: Router<AppState> = Router::new().route("/api/sessions/{id}", delete(stub));
    let b: Router<AppState> = Router::new().route("/api/sessions/{id}", post(stub).delete(stub));
    let _ = a.merge(b);
}
