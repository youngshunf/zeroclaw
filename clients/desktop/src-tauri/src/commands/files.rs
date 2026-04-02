//! 文件操作命令 — 文件复制到工作区

use base64::{engine::general_purpose::STANDARD, Engine as _};
use rusqlite::Connection;
use std::path::{Path, PathBuf};

/// 复制文件到 Agent 工作区
///
/// 前端将文件读取为 base64 后调用此命令，
/// 在工作区 files/ 目录下创建文件。
#[tauri::command]
pub async fn copy_file_to_workspace(
    base64_data: String,
    dest_path: String,
) -> Result<String, String> {
    let path = PathBuf::from(&dest_path);

    // 确保父目录存在
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("创建目录失败: {e}"))?;
    }

    // 解码 base64 并写入
    let bytes = STANDARD
        .decode(&base64_data)
        .map_err(|e| format!("base64 解码失败: {e}"))?;

    tokio::fs::write(&path, &bytes)
        .await
        .map_err(|e| format!("写入文件失败: {e}"))?;

    eprintln!(
        "[huanxing-desktop] File saved: {} ({} bytes)",
        dest_path,
        bytes.len()
    );

    Ok(dest_path)
}

/// 获取当前 Agent 工作区目录
#[tauri::command]
pub fn get_workspace_dir(
    manager: tauri::State<'_, std::sync::Arc<crate::sidecar::manager::SidecarManager>>,
) -> Result<String, String> {
    if !manager.has_valid_huanxing_config() {
        return Err("HuanXing configuration is invalid or missing".into());
    }

    let config_dir = manager.config_dir();
    let workspace = resolve_default_agent_workspace(&config_dir)?;

    // 确保目录存在
    std::fs::create_dir_all(&workspace).map_err(|e| format!("创建工作区目录失败: {e}"))?;

    Ok(workspace.to_string_lossy().to_string())
}

/// 获取唤星配置目录
#[tauri::command]
pub fn get_config_dir() -> Result<String, String> {
    let home = dirs::home_dir().ok_or("无法获取 home 目录")?;
    let config_dir = home.join(".huanxing");
    Ok(config_dir.to_string_lossy().to_string())
}

fn resolve_default_agent_workspace(config_dir: &Path) -> Result<PathBuf, String> {
    let tenant_dir = resolve_first_tenant_dir(config_dir)?;
    Ok(config_dir
        .join("users")
        .join(tenant_dir)
        .join("agents")
        .join("default")
        .join("workspace"))
}

fn resolve_first_tenant_dir(config_dir: &Path) -> Result<String, String> {
    let db_path = config_dir.join("data").join("users.db");
    let conn = Connection::open(&db_path)
        .map_err(|e| format!("打开 users.db 失败 ({}): {e}", db_path.display()))?;

    conn.query_row(
        "SELECT tenant_dir
         FROM users
         WHERE tenant_dir IS NOT NULL
           AND TRIM(tenant_dir) != ''
         ORDER BY datetime(COALESCE(created_at, '1970-01-01T00:00:00Z')) ASC, rowid ASC
         LIMIT 1",
        [],
        |row| row.get::<_, String>(0),
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            "未在 users.db 中找到可用 tenant_dir，无法定位默认 Agent 工作区".to_string()
        }
        other => format!("读取 tenant_dir 失败: {other}"),
    })
}

#[cfg(test)]
mod tests {
    use super::resolve_default_agent_workspace;
    use rusqlite::Connection;
    use tempfile::tempdir;

    fn seed_users_db(config_dir: &std::path::Path, tenant_dir: &str) {
        let data_dir = config_dir.join("data");
        std::fs::create_dir_all(&data_dir).unwrap();
        let db_path = data_dir.join("users.db");
        let conn = Connection::open(db_path).unwrap();
        conn.execute(
            "CREATE TABLE users (
                tenant_dir TEXT,
                created_at TEXT
            )",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO users (tenant_dir, created_at) VALUES (?1, ?2)",
            rusqlite::params![tenant_dir, "2026-04-02T00:00:00Z"],
        )
        .unwrap();
    }

    #[test]
    fn resolve_default_agent_workspace_uses_users_db_tenant() {
        let temp = tempdir().unwrap();
        let config_dir = temp.path().join(".huanxing");
        seed_users_db(&config_dir, "001-tenant-a");

        let workspace = resolve_default_agent_workspace(&config_dir).unwrap();

        assert_eq!(
            workspace,
            config_dir
                .join("users")
                .join("001-tenant-a")
                .join("agents")
                .join("default")
                .join("workspace")
        );
    }

    #[test]
    fn resolve_default_agent_workspace_errors_when_no_tenant_exists() {
        let temp = tempdir().unwrap();
        let config_dir = temp.path().join(".huanxing");
        std::fs::create_dir_all(config_dir.join("data")).unwrap();

        let err = resolve_default_agent_workspace(&config_dir).unwrap_err();

        assert!(err.contains("tenant"));
        assert!(!config_dir.join("agents").exists());
    }
}
