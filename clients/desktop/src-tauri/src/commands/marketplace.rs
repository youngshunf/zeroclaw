use std::fs::{self, File};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use serde_json::Value;
use toml::Table;
use tauri::{command, Emitter, AppHandle};

#[derive(Clone, serde::Serialize)]
struct ProgressPayload {
    message: String,
}

fn emit_progress(app: &AppHandle, msg: &str) {
    let _ = app.emit("agent-install-progress", ProgressPayload { message: msg.to_string() });
}

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
    "http://127.0.0.1:8020".to_string()
}

/// 读取全局配置中的 LLM 设置（用于替换模板占位符）
#[allow(dead_code)]
fn read_global_llm_config() -> (String, f64) {
    let config_path = dirs::home_dir()
        .unwrap_or_default()
        .join(".huanxing")
        .join("config.toml");

    let mut model = "MiniMax-M2.7".to_string();
    let mut temperature = 0.7;

    if let Ok(content) = fs::read_to_string(&config_path) {
        if let Ok(table) = content.parse::<Table>() {
            if let Some(m) = table.get("default_model").and_then(|v| v.as_str()) {
                model = m.to_string();
            }
            if let Some(t) = table.get("default_temperature").and_then(|v| v.as_float()) {
                temperature = t;
            }
        }
    }
    (model, temperature)
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
    
    // 3. Sync SOPs
    let sops_url = format!("{}/api/v1/marketplace/client/sops", api_base);
    if let Ok(res) = reqwest::get(&sops_url).await {
        if let Ok(json) = res.json::<Value>().await {
            if let Some(data) = json.get("data") {
                let path = cache_dir.join("market_sops.json");
                let _ = fs::write(&path, serde_json::to_string(data).unwrap_or_default());
            }
        }
    }
    
    // 4. Sync and download common-skills from market
    let common_skills_url = format!("{}/api/v1/marketplace/client/common-skills", api_base);
    let skills_dir = dirs::home_dir().unwrap_or_default().join(".huanxing").join("skills");
    let _ = fs::create_dir_all(&skills_dir);
    
    if let Ok(res) = reqwest::get(&common_skills_url).await {
        if let Ok(json) = res.json::<Value>().await {
            if let Some(data) = json.get("data") {
                if let Some(skills_arr) = data.get("skills").and_then(|v| v.as_array()) {
                    for skill_val in skills_arr {
                        if let Some(skill_id) = skill_val.as_str() {
                            let target_skill_dir = skills_dir.join(skill_id);
                            if !target_skill_dir.exists() {
                                // Skill is not installed locally, let's download it.
                                eprintln!("[huanxing-desktop] Downloading common skill: {}", skill_id);
                                if let Ok(info) = get_download_info(&api_base, "skill", skill_id).await {
                                    if let Some(pkg_url) = info.get("package_url").and_then(|v| v.as_str()) {
                                        if let Ok(bytes) = download_bytes(pkg_url).await {
                                            if unzip_buffer(&bytes, &target_skill_dir).is_ok() {
                                                eprintln!("[huanxing-desktop] Successfully installed common skill: {}", skill_id);
                                            } else {
                                                let _ = fs::remove_dir_all(&target_skill_dir);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    eprintln!("[huanxing-desktop] Marketplace cache synchronized (apps + skills + sops + common-skills).");
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

#[command]
pub fn get_market_sops() -> Result<Value, String> {
    let path = get_cache_dir().join("market_sops.json");
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

/// 下载文件到字节数组
async fn download_bytes(url: &str) -> Result<Vec<u8>, String> {
    let response = reqwest::get(url)
        .await
        .map_err(|e| format!("下载失败: {}", e))?;
    if !response.status().is_success() {
        return Err(format!("下载响应错误: {}", response.status()));
    }
    let bytes = response.bytes().await.map_err(|e| e.to_string())?;
    Ok(bytes.to_vec())
}

/// 从市场 API 获取最新版本下载信息
async fn get_download_info(api_base: &str, item_type: &str, item_id: &str) -> Result<Value, String> {
    let url = format!("{}/api/v1/marketplace/client/download/{}/{}/latest", api_base, item_type, item_id);
    let response = reqwest::get(&url)
        .await
        .map_err(|e| format!("获取下载信息失败: {}", e))?;
    let json: Value = response.json().await.map_err(|e| e.to_string())?;
    
    let code = json.get("code").and_then(|c| c.as_i64());
    if code != Some(0) && code != Some(200) {
        return Err(format!("获取下载信息失败: {}", json.get("msg").and_then(|m| m.as_str()).unwrap_or("未知错误")));
    }
    
    json.get("data").cloned().ok_or_else(|| "响应缺少 data 字段".to_string())
}

#[command]
pub async fn download_and_install_agent(
    app: tauri::AppHandle,
    _app_id: String,
    agent_name: String,
    display_name: String,
    package_url: String,
) -> Result<(), String> {
    emit_progress(&app, "初始化安装环境...");
    eprintln!("[huanxing-desktop] Installing Agent '{}' (template: {})", agent_name, _app_id);
    
    let api_base = read_marketplace_api_base();
    
    // ── Step 1: 市场下载包获取 ──
    let final_url = if package_url.is_empty() || package_url.contains(":8000") {
        emit_progress(&app, "正在获取下载地址...");
        let info = get_download_info(&api_base, "app", &_app_id).await
            .map_err(|e| format!("无法获取下载地址: {}", e))?;
        info.get("package_url").and_then(|v| v.as_str()).unwrap_or_default().to_string()
    } else {
        package_url
    };
    
    if final_url.is_empty() {
        emit_progress(&app, "Error: 无效的下载地址");
        return Err("无法解析有效的 package_url".to_string());
    }

    // ── Step 2: 调用 AgentFactory 安装体系 ──
    let config_dir = dirs::home_dir().unwrap_or_default().join(".huanxing");
    let factory = huanxing_agent_factory::AgentFactory::new(config_dir, Some(api_base));

    let params = huanxing_agent_factory::CreateAgentParams {
        tenant_id: "default".to_string(), // 桌面端始终在 default 租户下
        template_id: _app_id.clone(),
        agent_name: agent_name.clone(),
        display_name: display_name.clone(),
        is_desktop: true,                 // 触发 Layer2: _base_desktop 特殊覆盖
        user_nickname: display_name.clone(),
        provider: None,
        api_key: None,
        hasn_id: None,
    };

    struct TauriProgress {
        pub app: tauri::AppHandle,
    }
    
    impl huanxing_agent_factory::ProgressSink for TauriProgress {
        fn on_progress(&self, step: &str, detail: &str) {
            emit_progress(&self.app, &format!("{} - {}", step, detail));
        }
        fn on_error(&self, step: &str, error: &str) {
            emit_progress(&self.app, &format!("⚠️错误: {} ({})", step, error));
        }
    }

    match factory.install_from_market(&params, &final_url, &TauriProgress { app: app.clone() }).await {
        Ok(_) => {
            eprintln!("[huanxing-desktop] Agent '{}' installed successfully", agent_name);
            emit_progress(&app, "Agent 赋能安装全部完成！");
            Ok(())
        }
        Err(e) => {
            let msg = format!("Agent 安装失败: {}", e);
            eprintln!("[huanxing-desktop] {}", msg);
            Err(msg)
        }
    }
}

#[command]
pub async fn download_and_install_skill(
    app: tauri::AppHandle,
    agent_name: String,
    skill_id: String,
    package_url: String,
) -> Result<(), String> {
    emit_progress(&app, &format!("准备获取技能 '{}' ...", skill_id));
    eprintln!("[huanxing-desktop] Downloading Skill {} for Agent {}", skill_id, agent_name);
    
    let api_base = read_marketplace_api_base();
    
    // Resolve package URL
    let final_url = if package_url.is_empty() || package_url.contains(":8000") {
        let info = get_download_info(&api_base, "skill", &skill_id).await
            .map_err(|e| format!("无法获取 Skill 下载地址: {}", e))?;
        info.get("package_url").and_then(|v| v.as_str()).unwrap_or_default().to_string()
    } else {
        package_url
    };
    if final_url.is_empty() { 
        emit_progress(&app, "Error: 无法解析有效的 package_url");
        return Err("无法解析有效的 package_url".to_string()); 
    }

    emit_progress(&app, "正在下载技能包...");
    // 1. Download
    let response = reqwest::get(&final_url)
        .await
        .map_err(|e| format!("下载失败: {}", e))?;
        
    let bytes = response.bytes().await.map_err(|e| e.to_string())?;
    
    // 2. target path
    let target_dir = dirs::home_dir()
        .unwrap_or_default()
        .join(".huanxing")
        .join("users")
        .join("default")
        .join("agents")
        .join(&agent_name)
        .join("skills")
        .join(&skill_id);
        
    if target_dir.exists() {
        let _ = fs::remove_dir_all(&target_dir); // clean old
    }
    fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;
    
    // 3. Extract
    emit_progress(&app, "正在安装和解压...");
    unzip_buffer(&bytes, &target_dir)?;
    
    // 4. Update the agent's config.toml skills list
    emit_progress(&app, "更新 Agent 配置依赖...");
    let config_path = dirs::home_dir()
        .unwrap_or_default()
        .join(".huanxing")
        .join("users")
        .join("default")
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
    
    emit_progress(&app, "✅ 技能安装成功！");
    Ok(())
}

#[command]
pub async fn download_and_install_sop(
    app: tauri::AppHandle,
    agent_name: String,
    sop_id: String,
    package_url: String,
) -> Result<(), String> {
    emit_progress(&app, &format!("准备获取 SOP 工作流 '{}' ...", sop_id));
    
    let api_base = read_marketplace_api_base();
    
    // Resolve package URL
    let final_url = if package_url.is_empty() || package_url.contains(":8000") {
        let info = get_download_info(&api_base, "sop", &sop_id).await
            .map_err(|e| format!("无法获取 SOP 下载地址: {}", e))?;
        info.get("package_url").and_then(|v| v.as_str()).unwrap_or_default().to_string()
    } else {
        package_url
    };
    if final_url.is_empty() { 
        emit_progress(&app, "Error: 无法解析有效的 package_url");
        return Err("无法解析有效的 package_url".to_string()); 
    }

    emit_progress(&app, "正在下载 SOP 工作流包...");
    let bytes = download_bytes(&final_url).await?;
    
    emit_progress(&app, "正在初始化安装目录...");
    
    let target_dir = dirs::home_dir()
        .unwrap_or_default()
        .join(".huanxing")
        .join("users")
        .join("default")
        .join("agents")
        .join(&agent_name)
        .join("sops")
        .join(&sop_id);
    
    if target_dir.exists() {
        let _ = fs::remove_dir_all(&target_dir);
    }
    fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;
    
    emit_progress(&app, "正在解压资产...");
    unzip_buffer(&bytes, &target_dir)?;
    
    // 解析 SOP.md 中引用的技能，检查是否已安装
    emit_progress(&app, "解析并确认能力依赖 (Requirements)...");
    let sop_md_path = target_dir.join("SOP.md");
    if sop_md_path.exists() {
        if let Ok(md_content) = fs::read_to_string(&sop_md_path) {
            let api_base = read_marketplace_api_base();
            let skills_dir = dirs::home_dir()
                .unwrap_or_default()
                .join(".huanxing")
                .join("users")
                .join("default")
                .join("agents")
                .join(&agent_name)
                .join("skills");
            
            // 提取 `- tools: xxx, yyy` 行中的技能
            for line in md_content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("- tools:") {
                    let tools_part = trimmed.trim_start_matches("- tools:").trim();
                    for tool in tools_part.split(',') {
                        let tool = tool.trim();
                        if tool.is_empty() { continue; }
                        // 跳过内置工具
                        if tool.starts_with("memory_") || tool.starts_with("web_") || 
                           tool.starts_with("hx_") || tool == "shell" || tool == "file_read" || 
                           tool == "file_write" || tool == "delegate" {
                            continue;
                        }
                        // 检查是否已安装
                        if !skills_dir.join(tool).exists() {
                            emit_progress(&app, &format!("缺少能力依赖 '{}'，开始自动安装...", tool));
                            eprintln!("[huanxing-desktop]   SOP 依赖技能 '{}' 未安装，尝试自动安装...", tool);
                            match get_download_info(&api_base, "skill", tool).await {
                                Ok(info) => {
                                    if let Some(pkg_url) = info.get("package_url").and_then(|v| v.as_str()) {
                                        match download_bytes(pkg_url).await {
                                            Ok(skill_bytes) => {
                                                let skill_dir = skills_dir.join(tool);
                                                let _ = fs::create_dir_all(&skill_dir);
                                                if let Err(e) = unzip_buffer(&skill_bytes, &skill_dir) {
                                                    emit_progress(&app, &format!("Error: 依赖 '{}' 安装失败: {}", tool, e));
                                                    eprintln!("[huanxing-desktop]   ⚠ 技能 '{}' 安装失败: {}", tool, e);
                                                } else {
                                                    emit_progress(&app, &format!("✓ 依赖 '{}' 安装完备", tool));
                                                    eprintln!("[huanxing-desktop]   ✓ 技能 '{}' 自动安装成功", tool);
                                                }
                                            }
                                            Err(e) => eprintln!("[huanxing-desktop]   ⚠ 技能 '{}' 下载失败: {}", tool, e),
                                        }
                                    }
                                }
                                Err(_) => eprintln!("[huanxing-desktop]   [i] 技能 '{}' 可能是内置工具，跳过", tool),
                            }
                        }
                    }
                }
            }
        }
    }
    
    eprintln!("[huanxing-desktop] SOP '{}' installed for Agent '{}'", sop_id, agent_name);
    emit_progress(&app, "✅ SOP 工作流安装成功！");
    Ok(())
}
