use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Manager};

#[cfg(not(mobile))]
use crate::sidecar::manager::SidecarManager;

/// 获取配置目录（桌面端从 SidecarManager，移动端从 home_dir）
fn get_config_dir(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    #[cfg(not(mobile))]
    {
        let manager = app.state::<Arc<SidecarManager>>();
        Ok(manager.config_dir().clone())
    }
    #[cfg(mobile)]
    {
        let _ = app;
        Ok(dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".huanxing"))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentRecord {
    pub agent_id: String,
    pub template: String,
    pub star_name: Option<String>,
    pub hasn_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct QrCodeResponse {
    pub session_key: String,
    pub qrcode_url: String,
    /// The actual QR code string (used for polling status)
    pub qrcode_id: String,
}

#[derive(Debug, Serialize)]
pub struct AuthStatusResponse {
    pub status: String,
    pub bot_token: Option<String>,
    pub ilink_bot_id: Option<String>,
}

#[tauri::command]
pub async fn list_user_agents(app: AppHandle) -> Result<Vec<AgentRecord>, String> {
    let config_dir = get_config_dir(&app)?;
    let db_path = config_dir.join("tenant.db");

    let conn =
        rusqlite::Connection::open(&db_path).map_err(|e| format!("Failed to open DB: {}", e))?;

    let mut stmt = conn.prepare_cached(
        "SELECT agent_id, template, star_name, hasn_id FROM agents WHERE user_id = ?1 ORDER BY created_at"
    ).map_err(|e| e.to_string())?;

    // Desktop user is always default
    let user_id = "desktop_user_id";

    let rows = stmt
        .query_map(rusqlite::params![user_id], |row| {
            Ok(AgentRecord {
                agent_id: row.get(0)?,
                template: row.get(1).unwrap_or_default(),
                star_name: row.get(2).unwrap_or(None),
                hasn_id: row.get(3).unwrap_or(None),
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(rows)
}

#[tauri::command]
pub async fn bind_channel_to_agent(
    app: AppHandle,
    channel_type: String,
    sender_id: String,
    agent_id: String,
) -> Result<(), String> {
    let config_dir = get_config_dir(&app)?;
    let db_path = config_dir.join("tenant.db");

    let conn =
        rusqlite::Connection::open(&db_path).map_err(|e| format!("Failed to open DB: {}", e))?;

    let user_id = "desktop_user_id";

    conn.execute(
        "INSERT OR REPLACE INTO routing (channel_type, sender_id, agent_id, user_id)
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![channel_type, sender_id, agent_id, user_id],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

const FIXED_BASE_URL: &str = "https://ilinkai.weixin.qq.com";

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct WxQRCodeResponse {
    qrcode: String,
    qrcode_img_content: String,
}

#[derive(Debug, Deserialize)]
struct WxStatusResponse {
    status: String,
    bot_token: Option<String>,
    ilink_bot_id: Option<String>,
}

#[tauri::command]
pub async fn generate_weixin_qr() -> Result<QrCodeResponse, String> {
    let client = Client::builder()
        .timeout(Duration::from_millis(8000))
        .build()
        .map_err(|e| e.to_string())?;

    let url = format!("{}/ilink/bot/get_bot_qrcode?bot_type=3", FIXED_BASE_URL);
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;

    let text = resp.text().await.map_err(|e| e.to_string())?;
    tracing::info!("WeChat QR API response: {}", &text[..text.len().min(500)]);
    let qr_response: WxQRCodeResponse = serde_json::from_str(&text).map_err(|e| e.to_string())?;

    // qrcode_img_content is a URL like "https://liteapp.weixin.qq.com/q/..."
    // which is an HTML page, not an image. We need to generate the QR code locally.
    let qr_content = &qr_response.qrcode_img_content;
    tracing::info!("Generating QR code for URL: {}", qr_content);

    let qr_svg = qrcode::QrCode::new(qr_content.as_bytes())
        .map_err(|e| format!("Failed to generate QR code: {e}"))?
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(256, 256)
        .dark_color(qrcode::render::svg::Color("#000000"))
        .light_color(qrcode::render::svg::Color("#ffffff"))
        .build();

    // Encode SVG as a data URI
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(qr_svg.as_bytes());
    let data_uri = format!("data:image/svg+xml;base64,{}", b64);

    Ok(QrCodeResponse {
        session_key: uuid::Uuid::new_v4().to_string(),
        qrcode_url: data_uri,
        qrcode_id: qr_response.qrcode,
    })
}

#[tauri::command]
pub async fn poll_weixin_auth_status(
    _session_key: String,
    qrcode: String,
) -> Result<AuthStatusResponse, String> {
    let client = Client::builder()
        .timeout(Duration::from_millis(35000))
        .build()
        .map_err(|e| e.to_string())?;

    let url = format!(
        "{}/ilink/bot/get_qrcode_status?qrcode={}",
        FIXED_BASE_URL, qrcode
    );
    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            if e.is_timeout() {
                return Ok(AuthStatusResponse {
                    status: "wait".to_string(),
                    bot_token: None,
                    ilink_bot_id: None,
                });
            }
            return Err(e.to_string());
        }
    };

    let text = resp.text().await.map_err(|e| e.to_string())?;
    let status_response: WxStatusResponse =
        serde_json::from_str(&text).map_err(|e| e.to_string())?;

    Ok(AuthStatusResponse {
        status: status_response.status,
        bot_token: status_response.bot_token,
        ilink_bot_id: status_response.ilink_bot_id,
    })
}

/// 将微信扫码获得的凭证持久化到 config.toml 的 `[channels_config.weixin]` 节。
///
/// 写入后需要重启 sidecar 才能使渠道生效。
#[tauri::command]
pub async fn save_weixin_credentials(
    app: AppHandle,
    bot_token: String,
    bot_id: String,
    base_url: Option<String>,
) -> Result<(), String> {
    let config_dir = get_config_dir(&app)?;
    let config_path = config_dir.join("config.toml");

    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config.toml: {}", e))?;

    let mut doc = content
        .parse::<toml_edit::DocumentMut>()
        .map_err(|e| format!("Failed to parse config.toml: {}", e))?;

    // Ensure [channels_config] exists
    if doc.get("channels_config").is_none() {
        doc["channels_config"] = toml_edit::Item::Table(toml_edit::Table::new());
    }

    // Build [channels_config.weixin] table
    let mut weixin_table = toml_edit::Table::new();
    weixin_table.insert("bot_token", toml_edit::value(&bot_token));
    weixin_table.insert("bot_id", toml_edit::value(&bot_id));
    weixin_table.insert(
        "base_url",
        toml_edit::value(
            base_url
                .as_deref()
                .unwrap_or("https://ilinkai.weixin.qq.com"),
        ),
    );

    doc["channels_config"]["weixin"] = toml_edit::Item::Table(weixin_table);

    std::fs::write(&config_path, doc.to_string())
        .map_err(|e| format!("Failed to write config.toml: {}", e))?;

    tracing::info!("Weixin credentials saved to config.toml");

    Ok(())
}
