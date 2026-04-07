use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct CronJob {
    pub id: String,
    pub expression: String,
    pub name: Option<String>,
    pub prompt: Option<String>,
    pub enabled: bool,
    pub next_run: String,
    pub last_run: Option<String>,
    pub last_status: Option<String>,
    pub last_output: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CronRun {
    pub id: i64,
    pub job_id: String,
    pub started_at: String,
    pub finished_at: String,
    pub status: String,
    pub output: Option<String>,
    pub duration_ms: Option<i64>,
}

fn get_db_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".huanxing/guardian/workspace/cron/jobs.db")
}

#[tauri::command]
pub fn list_cron_jobs() -> Result<Vec<CronJob>, String> {
    let db_path = get_db_path();
    if !db_path.exists() {
        return Ok(vec![]);
    }

    let conn = rusqlite::Connection::open(&db_path).map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, expression, name, prompt, enabled, next_run, last_run, last_status, last_output FROM cron_jobs ORDER BY next_run ASC")
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            let enabled_int: i32 = row.get(4)?;
            Ok(CronJob {
                id: row.get(0)?,
                expression: row.get(1)?,
                name: row.get(2)?,
                prompt: row.get(3)?,
                enabled: enabled_int == 1,
                next_run: row.get(5)?,
                last_run: row.get(6)?,
                last_status: row.get(7)?,
                last_output: row.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut jobs = Vec::new();
    for row in rows {
        jobs.push(row.map_err(|e| e.to_string())?);
    }
    Ok(jobs)
}

#[tauri::command]
pub fn delete_cron_job(id: String) -> Result<(), String> {
    let db_path = get_db_path();
    let conn = rusqlite::Connection::open(&db_path).map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM cron_jobs WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn toggle_cron_job(id: String, enabled: bool) -> Result<(), String> {
    let db_path = get_db_path();
    let conn = rusqlite::Connection::open(&db_path).map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE cron_jobs SET enabled = ?1 WHERE id = ?2",
        params![if enabled { 1 } else { 0 }, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn add_cron_job(name: Option<String>, expression: String, prompt: String) -> Result<String, String> {
    let db_path = get_db_path();
    
    // Ensure parent directory exists
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    
    let conn = rusqlite::Connection::open(&db_path).map_err(|e| e.to_string())?;

    // Create table if it doesn't exist
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         CREATE TABLE IF NOT EXISTS cron_jobs (
            id               TEXT PRIMARY KEY,
            expression       TEXT NOT NULL,
            command          TEXT NOT NULL,
            schedule         TEXT,
            job_type         TEXT NOT NULL DEFAULT 'shell',
            prompt           TEXT,
            name             TEXT,
            session_target   TEXT NOT NULL DEFAULT 'isolated',
            model            TEXT,
            enabled          INTEGER NOT NULL DEFAULT 1,
            delivery         TEXT,
            delete_after_run INTEGER NOT NULL DEFAULT 0,
            allowed_tools    TEXT,
            created_at       TEXT NOT NULL,
            next_run         TEXT NOT NULL,
            last_run         TEXT,
            last_status      TEXT,
            last_output      TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_cron_jobs_next_run ON cron_jobs(next_run);"
    )
    .map_err(|e| e.to_string())?;

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    
    // Calculate a dummy next run based on now (sidecar scheduler will recalculate accurate one)
    let next_run = now.clone();

    // The backend uses job_type = 'agent' for agent jobs.
    conn.execute(
        "INSERT INTO cron_jobs (
            id, expression, command, job_type, prompt, name, session_target,
            enabled, delete_after_run, created_at, next_run
         ) VALUES (?1, ?2, '', 'agent', ?3, ?4, 'isolated', 1, 0, ?5, ?6)",
        params![id, expression, prompt, name, now, next_run],
    )
    .map_err(|e| e.to_string())?;

    Ok(id)
}

#[tauri::command]
pub fn get_cron_runs(job_id: String) -> Result<Vec<CronRun>, String> {
    let db_path = get_db_path();
    if !db_path.exists() {
        return Ok(vec![]);
    }

    let conn = rusqlite::Connection::open(&db_path).map_err(|e| e.to_string())?;
    // Only fetch table if it has been created to avoid errors on new instances
    let mut stmt = match conn.prepare("SELECT id, job_id, started_at, finished_at, status, output, duration_ms FROM cron_runs WHERE job_id = ?1 ORDER BY started_at DESC LIMIT 50") {
        Ok(s) => s,
        Err(_) => return Ok(vec![]) // table might not exist yet
    };

    let rows = stmt
        .query_map(params![job_id], |row| {
            Ok(CronRun {
                id: row.get(0)?,
                job_id: row.get(1)?,
                started_at: row.get(2)?,
                finished_at: row.get(3)?,
                status: row.get(4)?,
                output: row.get(5)?,
                duration_ms: row.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut runs = Vec::new();
    for row in rows {
        runs.push(row.map_err(|e| e.to_string())?);
    }
    Ok(runs)
}
