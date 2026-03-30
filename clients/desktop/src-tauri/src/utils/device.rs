//! 系统设备指纹与运行环境特征提取

pub fn get_device_fingerprint() -> String {
    // macOS: ioreg -rd1 -c IOPlatformExpertDevice | grep IOPlatformUUID
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("ioreg")
            .args(["-rd1", "-c", "IOPlatformExpertDevice"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.contains("IOPlatformUUID") {
                    if let Some(uuid) = line.split('"').nth(3) {
                        return uuid.to_string();
                    }
                }
            }
        }
    }

    // Linux: /etc/machine-id
    #[cfg(target_os = "linux")]
    {
        if let Ok(mid) = std::fs::read_to_string("/etc/machine-id") {
            let trimmed = mid.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }

    // Fallback: hostname hash
    let hostname = hostname::get()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    hostname.hash(&mut hasher);
    format!("h_{:016x}", hasher.finish())
}

pub fn build_device_info() -> serde_json::Value {
    let fingerprint = get_device_fingerprint();
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let hostname = hostname::get()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    serde_json::json!({
        "device_fingerprint": fingerprint,
        "os": os,
        "arch": arch,
        "hostname": hostname,
    })
}
