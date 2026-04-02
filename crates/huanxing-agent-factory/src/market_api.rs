use anyhow::Result;
use reqwest::Client;
use std::io::Cursor;
use std::path::Path;

pub async fn get_download_info(
    api_base: &str,
    resource_type: &str,
    resource_id: &str,
) -> Result<serde_json::Value> {
    let url = format!(
        "{}/api/v1/marketplace/client/download/{}/{}/latest",
        api_base, resource_type, resource_id
    );
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let resp = client.get(&url).send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("Market API error: {}", resp.status());
    }

    let json = resp.json::<serde_json::Value>().await?;

    // API 响应格式: { "code": 0, "data": { "package_url": "..." } }
    // 提取 .data 返回，与 desktop marketplace.rs 的 get_download_info 保持一致
    json.get("data")
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("API 响应缺少 data 字段"))
}

pub async fn download_bytes(url: &str) -> Result<Vec<u8>> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("Download failed: {}", resp.status());
    }
    let body = resp.bytes().await?;
    Ok(body.to_vec())
}

pub fn unzip_buffer(buf: &[u8], dest_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dest_dir)?;
    let cursor = Cursor::new(buf);
    let mut archive = zip::ZipArchive::new(cursor)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = match file.enclosed_name() {
            Some(path) => dest_dir.join(path),
            None => continue,
        };

        if (&*file.name()).ends_with('/') {
            std::fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    std::fs::create_dir_all(p)?;
                }
            }
            let mut outfile = std::fs::File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;

            // UNIX check for permissions
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = file.unix_mode() {
                    let _ =
                        std::fs::set_permissions(&outpath, std::fs::Permissions::from_mode(mode));
                }
            }
        }
    }
    Ok(())
}
