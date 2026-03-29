//! 文件操作命令 — 文件复制到工作区

use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::path::PathBuf;

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
pub fn get_workspace_dir() -> Result<String, String> {
    let home = dirs::home_dir().ok_or("无法获取 home 目录")?;
    let workspace = home.join(".huanxing").join("agents").join("default");

    // 确保目录存在
    std::fs::create_dir_all(&workspace)
        .map_err(|e| format!("创建工作区目录失败: {e}"))?;

    Ok(workspace.to_string_lossy().to_string())
}

/// 获取唤星配置目录
#[tauri::command]
pub fn get_config_dir() -> Result<String, String> {
    let home = dirs::home_dir().ok_or("无法获取 home 目录")?;
    let config_dir = home.join(".huanxing");
    Ok(config_dir.to_string_lossy().to_string())
}
