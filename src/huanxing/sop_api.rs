use crate::gateway::AppState;
use crate::sop::engine::SopEngine;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Extension, Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

// ── Models ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AgentQuery {
    pub agent: String,
}

#[derive(Debug, Serialize)]
pub struct SopRequirementsDto {
    pub skills: Vec<String>,
    pub optional_skills: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SopStepInfo {
    pub number: u32,
    pub title: String,
    pub requires_confirmation: bool,
    pub suggested_tools: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SopInfo {
    pub name: String,
    pub display_name: Option<String>,
    pub description: String,
    pub version: String,
    pub priority: String,
    pub execution_mode: String,
    pub max_concurrent: u32,
    pub active_runs: usize,
    pub requirements: Option<SopRequirementsDto>,
}

#[derive(Debug, Serialize)]
pub struct SopDetailResponse {
    #[serde(flatten)]
    pub info: SopInfo,
    pub steps: Vec<SopStepInfo>,
    pub triggers: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SopListResponse {
    pub sops: Vec<SopInfo>,
}

// ── Router ────────────────────────────────────────────────────────

pub fn sop_routes() -> Router<AppState> {
    Router::new()
        .route("/api/sop/list", get(list_sops))
        .route("/api/sop/{name}/detail", get(sop_detail))
        .route("/api/sop/{name}/execute", post(execute_sop))
        .route("/api/sop/runs", get(list_runs))
}

// ── Handlers ──────────────────────────────────────────────────────

/// GET /api/sop/list?agent={name}
/// 
/// Lists all available SOPs for the specified agent.
async fn list_sops(
    State(state): State<AppState>,
    Query(query): Query<AgentQuery>,
    Extension(engine): Extension<Arc<Mutex<SopEngine>>>,
) -> impl IntoResponse {
    let workspace = resolve_agent_workspace(&state, &query.agent);
    
    if !workspace.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": format!("agent '{}' not found", query.agent)})),
        )
            .into_response();
    }

    let mut engine_guard = engine.lock().await;
    engine_guard.ensure_loaded(&workspace);

    let active_runs_per_sop = count_active_runs(&*engine_guard);
    
    let mut sops: Vec<SopInfo> = engine_guard
        .sops()
        .iter()
        .map(|sop| {
            let active_runs = active_runs_per_sop.get(&sop.name).copied().unwrap_or(0);
            
            let reqs = sop.requirements.as_ref().map(|r| SopRequirementsDto {
                skills: r.skills.clone(),
                optional_skills: r.optional_skills.clone(),
            });

            SopInfo {
                name: sop.name.clone(),
                display_name: sop.display_name.clone(),
                description: sop.description.clone(),
                version: sop.version.clone(),
                priority: sop.priority.to_string(),
                execution_mode: sop.execution_mode.to_string(),
                max_concurrent: sop.max_concurrent,
                active_runs,
                requirements: reqs,
            }
        })
        .collect();

    sops.sort_by(|a, b| a.name.cmp(&b.name));

    (StatusCode::OK, Json(SopListResponse { sops })).into_response()
}

/// GET /api/sop/{name}/detail?agent={agent_name}
/// 
/// Gets the full details including steps for a specific SOP.
async fn sop_detail(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(query): Query<AgentQuery>,
    Extension(engine): Extension<Arc<Mutex<SopEngine>>>,
) -> impl IntoResponse {
    let workspace = resolve_agent_workspace(&state, &query.agent);
    
    if !workspace.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": format!("agent '{}' not found", query.agent)})),
        )
            .into_response();
    }

    let mut engine_guard = engine.lock().await;
    engine_guard.ensure_loaded(&workspace);

    let op_sop = engine_guard.sops().iter().find(|s| s.name == name).cloned();

    match op_sop {
        Some(sop) => {
            let active_runs = count_active_runs(&*engine_guard)
                .get(&sop.name)
                .copied()
                .unwrap_or(0);

            let reqs = sop.requirements.as_ref().map(|r| SopRequirementsDto {
                skills: r.skills.clone(),
                optional_skills: r.optional_skills.clone(),
            });

            let info = SopInfo {
                name: sop.name.clone(),
                display_name: sop.display_name.clone(),
                description: sop.description.clone(),
                version: sop.version.clone(),
                priority: sop.priority.to_string(),
                execution_mode: sop.execution_mode.to_string(),
                max_concurrent: sop.max_concurrent,
                active_runs,
                requirements: reqs,
            };

            let steps = sop.steps.into_iter().map(|step| SopStepInfo {
                number: step.number,
                title: step.title,
                requires_confirmation: step.requires_confirmation,
                suggested_tools: step.suggested_tools,
            }).collect();
            
            let triggers = sop.triggers.into_iter().map(|t| t.to_string()).collect();

            let response = SopDetailResponse {
                info,
                steps,
                triggers,
            };

            (StatusCode::OK, Json(response)).into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": format!("sop '{}' not found", name)})),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct ExecuteSopRequest {
    pub payload: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ExecuteSopResponse {
    pub session_id: String,
    pub run_id: String,
    pub title: String,
}

/// POST /api/sop/{name}/execute?agent={agent_name}
/// 
/// Starts a new SOP execution and provisions a dedicated session.
async fn execute_sop(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(query): Query<AgentQuery>,
    Extension(engine): Extension<Arc<Mutex<SopEngine>>>,
    Json(req): Json<ExecuteSopRequest>,
) -> impl IntoResponse {
    let workspace = resolve_agent_workspace(&state, &query.agent);
    
    if !workspace.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": format!("agent '{}' not found", query.agent)})),
        )
            .into_response();
    }

    let run_id = {
        let mut engine_guard = engine.lock().await;
        engine_guard.ensure_loaded(&workspace);

        if engine_guard.get_sop(&name).is_none() {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": format!("SOP '{}' not found", name)})),
            )
                .into_response();
        }

        let event = crate::sop::SopEvent {
            source: crate::sop::SopTriggerSource::Manual,
            topic: None,
            payload: req.payload.clone(),
            timestamp: crate::sop::engine::now_iso8601(),
        };

        match engine_guard.start_run(&name, event) {
            Ok(action) => {
                match action {
                    crate::sop::SopRunAction::ExecuteStep { run_id, .. } => run_id,
                    crate::sop::SopRunAction::WaitApproval { run_id, .. } => run_id,
                    crate::sop::SopRunAction::DeterministicStep { run_id, .. } => run_id,
                    crate::sop::SopRunAction::CheckpointWait { run_id, .. } => run_id,
                    crate::sop::SopRunAction::Completed { run_id, .. } => run_id,
                    crate::sop::SopRunAction::Failed { run_id, .. } => run_id,
                }
            }
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": format!("Failed to start SOP run: {}", e)})),
                )
                    .into_response();
            }
        }
    };

    let session_id = format!("sop-run-{}", run_id);
    let title = format!("SOP工作流: {}", name);
    let now = chrono::Utc::now().to_rfc3339();

    let result = tokio::task::block_in_place(|| {
        let sessions_dir = workspace.join("sessions");
        std::fs::create_dir_all(&sessions_dir).ok();
        let db_path = sessions_dir.join("sessions.db");
        let conn = rusqlite::Connection::open(&db_path)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;",
        )?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS desktop_sessions (
                session_id   TEXT PRIMARY KEY,
                agent_id     TEXT NOT NULL,
                title        TEXT NOT NULL DEFAULT '',
                created_at   TEXT NOT NULL,
                updated_at   TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_desktop_sessions_agent ON desktop_sessions(agent_id);",
        )?;
        
        conn.execute(
            "INSERT INTO desktop_sessions (session_id, agent_id, title, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(session_id) DO UPDATE SET updated_at = ?5",
            rusqlite::params![session_id, query.agent, title, now, now],
        )?;

        Ok::<(), rusqlite::Error>(())
    });

    if let Err(e) = result {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to create DB session: {}", e)})),
        )
            .into_response();
    }

    (
        StatusCode::OK,
        Json(ExecuteSopResponse {
            session_id,
            run_id,
            title,
        }),
    )
        .into_response()
}

#[derive(Debug, Deserialize)]
pub struct RunsQuery {
    pub agent: String,
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RunsListResponse {
    pub runs: Vec<crate::sop::SopRun>,
}

/// GET /api/sop/runs?agent={name}&status=completed
async fn list_runs(
    State(state): State<AppState>,
    Query(query): Query<RunsQuery>,
    Extension(engine): Extension<Arc<Mutex<SopEngine>>>,
) -> impl IntoResponse {
    let workspace = resolve_agent_workspace(&state, &query.agent);
    
    if !workspace.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": format!("agent '{}' not found", query.agent)})),
        )
            .into_response();
    }

    let mut engine_guard = engine.lock().await;
    engine_guard.ensure_loaded(&workspace);

    let status_filter = query.status.as_deref();

    let mut result_runs = Vec::new();

    if status_filter.is_none() || status_filter == Some("active") {
        for run in engine_guard.active_runs().values() {
            result_runs.push(run.clone());
        }
    }

    if status_filter.is_none() || status_filter == Some("completed") || status_filter == Some("failed") || status_filter == Some("cancelled") {
        for run in engine_guard.finished_runs(None) {
            let status_str = match run.status {
                crate::sop::SopRunStatus::Completed => "completed",
                crate::sop::SopRunStatus::Failed => "failed",
                crate::sop::SopRunStatus::Cancelled => "cancelled",
                _ => "active", // shouldn't be here
            };
            if let Some(f) = status_filter {
                if status_str != f { continue; }
            }
            result_runs.push(run.clone());
        }
    }

    result_runs.sort_by(|a, b| b.started_at.cmp(&a.started_at));

    (StatusCode::OK, Json(RunsListResponse { runs: result_runs })).into_response()
}

// ── Helpers ───────────────────────────────────────────────────────

fn count_active_runs(engine: &SopEngine) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for run in engine.active_runs().values() {
        *counts.entry(run.sop_name.clone()).or_insert(0) += 1;
    }
    counts
}

fn resolve_agent_workspace(state: &AppState, agent_name: &str) -> std::path::PathBuf {
    let config = state.config.lock().clone();
    let agents_dir = config
        .huanxing
        .resolve_agents_dir(config.config_path.parent().unwrap_or(&config.workspace_dir));
    agents_dir.join(agent_name)
}
