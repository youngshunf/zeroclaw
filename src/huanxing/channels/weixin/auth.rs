use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;

const FIXED_BASE_URL: &str = "https://ilinkai.weixin.qq.com";
const GET_QRCODE_TIMEOUT_MS: u64 = 8_000;
const QR_LONG_POLL_TIMEOUT_MS: u64 = 35_000;

#[derive(Debug, Deserialize)]
pub struct QRCodeResponse {
    pub qrcode: String,
    pub qrcode_img_content: String,
}

#[derive(Debug, Deserialize)]
pub struct StatusResponse {
    pub status: String,
    pub bot_token: Option<String>,
    pub ilink_bot_id: Option<String>,
    pub baseurl: Option<String>,
    pub ilink_user_id: Option<String>,
    pub redirect_host: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ActiveLogin {
    pub session_key: String,
    pub qrcode: String,
    pub qrcode_url: String,
    pub started_at: std::time::Instant,
    pub current_api_base_url: String,
}

pub async fn start_weixin_login_qr(bot_type: &str) -> Result<ActiveLogin> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_millis(GET_QRCODE_TIMEOUT_MS))
        .build()?;

    let url = format!("{FIXED_BASE_URL}/ilink/bot/get_bot_qrcode?bot_type={bot_type}");
    let resp = client.get(&url).send().await?.error_for_status()?;
    let text = resp.text().await?;
    let qr_response: QRCodeResponse = serde_json::from_str(&text).context("failed to parse QR response")?;

    Ok(ActiveLogin {
        session_key: uuid::Uuid::new_v4().to_string(),
        qrcode: qr_response.qrcode,
        qrcode_url: qr_response.qrcode_img_content,
        started_at: std::time::Instant::now(),
        current_api_base_url: FIXED_BASE_URL.to_string(),
    })
}

pub async fn poll_qr_status(base_url: &str, qrcode: &str) -> Result<StatusResponse> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_millis(QR_LONG_POLL_TIMEOUT_MS))
        .build()?;

    let url = format!("{base_url}/ilink/bot/get_qrcode_status?qrcode={qrcode}");
    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            if e.is_timeout() {
                return Ok(StatusResponse {
                    status: "wait".to_string(),
                    bot_token: None,
                    ilink_bot_id: None,
                    baseurl: None,
                    ilink_user_id: None,
                    redirect_host: None,
                });
            }
            return Err(e.into());
        }
    };

    let text = resp.text().await?;
    let status_response: StatusResponse = serde_json::from_str(&text).context("failed to parse status poll")?;
    Ok(status_response)
}
