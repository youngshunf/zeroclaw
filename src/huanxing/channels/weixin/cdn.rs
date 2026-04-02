use aes::cipher::{BlockEncrypt, KeyInit};
use aes::Aes128;
use anyhow::{anyhow, Result};
use hex::encode as hex_encode;
use md5::{Md5, Digest};
use reqwest::Client;

use super::WeixinChannel;
use super::api::get_upload_url;
use super::types::GetUploadUrlReq;

pub struct UploadedFileInfo {
    pub filekey: String,
    pub download_encrypted_query_param: String,
    pub aeskey_hex: String,
    pub file_size: u64,
    pub file_size_ciphertext: u64,
}

pub fn encrypt_aes_ecb(plaintext: &[u8], key: &[u8]) -> Result<Vec<u8>> {
    if key.len() != 16 {
        return Err(anyhow!("AES-128 key must be 16 bytes"));
    }
    let cipher = Aes128::new_from_slice(key).map_err(|e| anyhow!("AES Key init failed: {}", e))?;
    
    // ECB is just block by block standard application
    // Let's manually implement PKCS7 and encrypt blocks.
    // pkcs7 padding always adds 1 to 16 bytes.
    let pad_len = 16 - (plaintext.len() % 16);
    let padded_len = plaintext.len() + pad_len;
    
    let mut ciphertext = vec![0u8; padded_len];
    ciphertext[..plaintext.len()].copy_from_slice(plaintext);
    
    // Apply pkcs7 padding
    for i in plaintext.len()..padded_len {
        ciphertext[i] = pad_len as u8;
    }
    
    // Encrypt each block
    for chunk in ciphertext.chunks_exact_mut(16) {
        let block = aes::cipher::generic_array::GenericArray::from_mut_slice(chunk);
        cipher.encrypt_block(block);
    }
    
    Ok(ciphertext)
}

fn compute_md5(data: &[u8]) -> String {
    let mut hasher = Md5::new();
    hasher.update(data);
    hex_encode(hasher.finalize())
}

fn aes_ecb_padded_size(plaintext_size: usize) -> u64 {
    let pad_len = 16 - (plaintext_size % 16);
    (plaintext_size + pad_len) as u64
}

pub async fn upload_media_to_cdn(
    channel: &WeixinChannel,
    data: &[u8],
    recipient: &str,
    media_type: u8,
) -> Result<UploadedFileInfo> {
    let rawsize = data.len();
    let rawfilemd5 = compute_md5(data);
    let filesize = aes_ecb_padded_size(rawsize);
    
    let filekey_bytes = uuid::Uuid::new_v4().into_bytes();
    let filekey = hex_encode(filekey_bytes);
    
    let aeskey_bytes = uuid::Uuid::new_v4().into_bytes();
    let aeskey_hex = hex_encode(aeskey_bytes);
    
    tracing::debug!("Weixin CDN upload preparing: rawsize={} filesize={} md5={} filekey={}", 
        rawsize, filesize, rawfilemd5, filekey);
        
    let req = GetUploadUrlReq {
        filekey: Some(filekey.clone()),
        media_type: Some(media_type),
        to_user_id: Some(recipient.to_string()),
        rawsize: Some(rawsize as u64),
        rawfilemd5: Some(rawfilemd5),
        filesize: Some(filesize),
        no_need_thumb: Some(true),
        aeskey: Some(aeskey_hex.clone()),
        thumb_rawsize: None,
        thumb_rawfilemd5: None,
        thumb_filesize: None,
        base_info: None,
    };
    
    let upload_resp = get_upload_url(channel, req).await?;
    
    let target_url = upload_resp.upload_full_url
        .unwrap_or_else(|| {
            // Build from upload param if full url missing (fallback)
            // But full URL is usually returned.
            // if we really needed full fallback buildCdnUploadUrl, wait, 
            // openclaw says if missing upload_full_url we use buildCdnUploadUrl. For now expect full url
            String::new()
        });
        
    if target_url.is_empty() {
        return Err(anyhow!("get_upload_url returned no upload_full_url"));
    }
    
    let ciphertext = encrypt_aes_ecb(data, &aeskey_bytes)?;
    
    tracing::debug!("Weixin CDN POST url={} ciphertextSize={}", target_url, ciphertext.len());
    
    let client = Client::new();
    let res = client.post(&target_url)
        .header("Content-Type", "application/octet-stream")
        .body(ciphertext)
        .send()
        .await?;
        
    let status = res.status();
    if !status.is_success() {
        let err_msg = res.headers().get("x-error-message")
            .map(|h| h.to_str().unwrap_or("")).unwrap_or("")
            .to_string();
        tracing::error!("Weixin CDN server error: status={} err={}", status, err_msg);
        return Err(anyhow!("CDN server error: {} {}", status, err_msg));
    }
    
    let download_param = res.headers().get("x-encrypted-param")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());
        
    let download_encrypted_query_param = download_param.ok_or_else(|| anyhow!("Missing x-encrypted-param in CDN response"))?;
    
    tracing::debug!("Weixin CDN upload success!");
    
    Ok(UploadedFileInfo {
        filekey,
        download_encrypted_query_param,
        aeskey_hex,
        file_size: rawsize as u64,
        file_size_ciphertext: filesize,
    })
}
