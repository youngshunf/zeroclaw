use std::fs::{self, File};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use serde_json::Value;
use toml::Table;
use tauri::command;

/// 获取 Marketplace API Base URL
fn read_marketplace_api_base() -> String {
    let config_path = dirs::home_dir()
        .unwrap_or_default()
        .join(".huanxing")
        .join("config.toml");

    if let Ok(content) = fs::read_to_string(&config_path) {
        if let Ok(table) = content.parse::<Table>() {
            if let Some(huanxing) = table.get("huanxing").and_then(|v| v.as_table()) {
                if let Some(url) = huanxing.get("api_base_url").and_then(|v| v.as_str()) {
                    let url = url.trim().trim_end_matches('/');
                    if !url.is_empty() {
                        return url.to_string();
                    }
                }
            }
        }
    }
    // Default fallback
    "http://127.0.0.1:8000".to_string()
}

/// 缓存目录路径
fn get_cache_dir() -> PathBuf {
    let dir = dirs::home_dir()
        .unwrap_or_default()
        .join(".huanxing")
        .join("cache");
    let _ = fs::create_dir_all(&dir);
    dir
}

/// 异步静默同步市场数据
pub async fn sync_marketplace_data() {
    let api_base = read_marketplace_api_base();
    let cache_dir = get_cache_dir();

    // 1. Sync Apps
    let apps_url = format!("{}/api/v1/marketplace/client/apps", api_base);
    if let Ok(res) = reqwest::get(&apps_url).await {
        if let Ok(json) = res.json::<Value>().await {
            if let Some(data) = json.get("data") {
                let path = cache_dir.join("market_apps.json");
                let _ = fs::write(&path, serde_json::to_string(data).unwrap_or_default());
            }
        }
    }

    // 2. Sync Skills
    let skills_url = format!("{}/api/v1/marketplace/client/skills", api_base);
    if let Ok(res) = reqwest::get(&skills_url).await {
        if let Ok(json) = res.json::<Value>().await {
            if let Some(data) = json.get("data") {
                let path = cache_dir.join("market_skills.json");
                let _ = fs::write(&path, serde_json::to_string(data).unwrap_or_default());
            }
        }
    }
    
    eprintln!("[huanxing-desktop] Marketplace cache synchronized.");
}

#[command]
pub fn get_market_apps() -> Result<Value, String> {
    let path = get_cache_dir().join("market_apps.json");
    if let Ok(content) = fs::read_to_string(path) {
        serde_json::from_str(&content).map_err(|e| e.to_string())
    } else {
        // Return empty items structure if not synced yet
        Ok(serde_json::json!({ "items": [], "total": 0 }))
    }
}

#[command]
pub fn get_market_skills() -> Result<Value, String> {
    let path = get_cache_dir().join("market_skills.json");
    if let Ok(content) = fs::read_to_string(path) {
        serde_json::from_str(&content).map_err(|e| e.to_string())
    } else {
        Ok(serde_json::json!({ "items": [], "total": 0 }))
    }
}

/// 辅助：解压工具
fn unzip_buffer(buf: &[u8], target_dir: &Path) -> Result<(), String> {
    let cursor = Cursor::new(buf);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| format!("Zip 解析失败: {}", e))?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
        let outpath = match file.enclosed_name() {
            Some(path) => target_dir.join(path),
            None => continue,
        };

        if file.name().ends_with('/') {
            fs::create_dir_all(&outpath).map_err(|e| e.to_string())?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(p).map_err(|e| e.to_string())?;
                }
            }
            let mut outfile = File::create(&outpath).map_err(|e| e.to_string())?;
            std::io::copy(&mut file, &mut outfile).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[command]
pub async fn download_and_install_agent(
    app_id: String,
    agent_name: String,
    display_name: String,
    package_url: String,
) -> Result<(), String> {
    eprintln!("[huanxing-desktop] Downloading Agent from: {}", package_url);
    
    // 1. Download
    let response = reqwest::get(&package_url)
        .await
        .map_err(|e| format!("下载失败: {}", e))?;
        
    if !response.status().is_success() {
        return Err(format!("下载响应错误: {}", response.status()));
    }
    
    let bytes = response.bytes().await.map_err(|e| e.to_string())?;
    
    // 2. target path
    let target_dir = dirs::home_dir()
        .unwrap_or_default()
        .join(".huanxing")
        .join("agents")
        .join(&agent_name);
        
    if target_dir.exists() {
        return Err(format!("Agent 目录已存在: {}", agent_name));
    }
    
    // 3. Extract
    unzip_buffer(&bytes, &target_dir)?;
    
    // 4. Override config.toml
    let config_path = target_dir.join("config.toml");
    if config_path.exists() {
        if let Ok(content) = fs::read_to_string(&config_path) {
            let mut doc = content.parse::<toml_edit::DocumentMut>().map_err(|e| e.to_string())?;
            if let Some(agent) = doc.get_mut("agent").and_then(|i| i.as_table_mut()) {
                agent.insert("name", toml_edit::value(agent_name.clone()));
                agent.insert("display_name", toml_edit::value(display_name.clone()));
            }
            fs::write(&config_path, doc.to_string()).ok();
        }
    }
    
    Ok(())
}

#[command]
pub async fn download_and_install_skill(
    agent_name: String,
    skill_id: String,
    package_url: String,
) -> Result<(), String> {
    eprintln!("[huanxing-desktop] Downloading Skill {} for Agent {}", skill_id, agent_name);
    
    // 1. Download
    let response = reqwest::get(&package_url)
        .await
        .map_err(|e| format!("下载失败: {}", e))?;
        
    let bytes = response.bytes().await.map_err(|e| e.to_string())?;
    
    // 2. target path
    let target_dir = dirs::home_dir()
        .unwrap_or_default()
        .join(".huanxing")
        .join("agents")
        .join(&agent_name)
        .join("skills")
        .join(&skill_id);
        
    if target_dir.exists() {
        let _ = fs::remove_dir_all(&target_dir); // clean old
    }
    fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;
    
    // 3. Extract
    unzip_buffer(&bytes, &target_dir)?;
    
    // 4. Update the agent's config.toml skills list
    let config_path = dirs::home_dir()
        .unwrap_or_default()
        .join(".huanxing")
        .join("agents")
        .join(&agent_name)
        .join("config.toml");
        
    if config_path.exists() {
        if let Ok(content) = fs::read_to_string(&config_path) {
            if let Ok(mut doc) = content.parse::<toml_edit::DocumentMut>() {
                let plugins = doc.entry("plugins").or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
                if let Some(plugins_table) = plugins.as_table_mut() {
                    let skills = plugins_table.entry("skills").or_insert(toml_edit::Item::Value(toml_edit::Value::Array(toml_edit::Array::new())));
                    
                    if let Some(arr) = skills.as_array_mut() {
                        let mut exists = false;
                        for v in arr.iter() {
                            if let Some(s) = v.as_str() {
                                if s == skill_id {
                                    exists = true;
                                    break;
                                }
                            }
                        }
                        if !exists {
                            arr.push(skill_id.clone());
                            fs::write(&config_path, doc.to_string()).ok();
                        }
                    }
                }
            }
        }
    }
    
    Ok(())
}
