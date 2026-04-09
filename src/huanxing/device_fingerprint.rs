//! 设备指纹生成模块
//!
//! 在 ZeroClaw 进程启动时生成稳定的设备指纹，并将其写回 config.toml。
//!
//! # 指纹派生策略
//!
//! 1. **主要来源**：hostname + 操作系统 + 操作系统版本
//!    - `hostname + OS + OS_VERSION` → SHA-256 → 前 32 字符 hex
//!    - 这在同一台机器上（无论重装/多用户/重启）永远相同
//!
//! 2. **node_id 派生**：`n_` + SHA-256(fingerprint)[:16]
//!    - 与后端 `register_node` 的派生逻辑完全一致
//!
//! 3. **node_name**：`{OS名称} {OS版本}` (e.g. "macOS 14.4.1", "Windows 11", "Ubuntu 24.04")
//!
//! 4. **写回机制**：只有当 config.toml 中 `node_id` 为 None/空时才写入，
//!    避免覆盖用户手动配置的值。

use sha2::{Digest, Sha256};
use std::sync::OnceLock;
use std::path::Path;

// ── 全局单例 ──────────────────────────────────────────────
/// 全局设备指纹（进程内单例，启动时由 bootstrap 写入，之后只读）
static GLOBAL_FINGERPRINT: OnceLock<DeviceFingerprint> = OnceLock::new();

/// 获取全局设备指纹（需要在 `set_global_fingerprint` 之后调用）。
pub fn get_global_fingerprint() -> Option<&'static DeviceFingerprint> {
    GLOBAL_FINGERPRINT.get()
}

/// 写入全局指纹（只能调用一次，bootstrap 阶段调用）。
pub fn set_global_fingerprint(fp: DeviceFingerprint) {
    let _ = GLOBAL_FINGERPRINT.set(fp);
}

/// 设备指纹结果
#[derive(Debug, Clone)]
pub struct DeviceFingerprint {
    /// 原始设备指纹字符串（32字符 hex），存入后端 device_fingerprint 字段
    pub fingerprint: String,
    /// 从 fingerprint 派生的 node_id（`n_` + fingerprint[:16]），与后端逻辑一致
    pub node_id: String,
    /// 设备名称，格式：`{OS} {版本}` (e.g. "macOS 14.4.1")
    pub node_name: String,
    /// 设备平台（lowercase，e.g. "macos", "windows", "linux"）
    pub device_platform: String,
}

/// 生成设备指纹。
///
/// 基于 hostname + OS + 版本号 进行 SHA-256 哈希，保证同一物理设备永远返回相同值。
pub fn generate_device_fingerprint() -> DeviceFingerprint {
    let host = hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "unknown-host".to_string());

    let os_name = std::env::consts::OS; // "macos" / "windows" / "linux"
    let os_version = get_os_version();

    // 拼接原料并哈希
    let raw = format!("{host}|{os_name}|{os_version}");
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    let hash = hasher.finalize();
    let fingerprint = format!("{:x}", hash)[..32].to_string();

    // node_id = n_ + sha256(fingerprint)[:16]（与后端 register_node 逻辑完全一致）
    let mut id_hasher = Sha256::new();
    id_hasher.update(fingerprint.as_bytes());
    let id_hash = id_hasher.finalize();
    let node_id = format!("n_{}", &format!("{:x}", id_hash)[..16]);

    // node_name: 友好的 OS 描述
    let (display_os, platform) = platform_display(os_name, &os_version);
    let node_name = format!("{display_os} {os_version}").trim().to_string();

    tracing::debug!(
        "[DeviceFingerprint] host={host} os={os_name} version={os_version} \
         fingerprint={fingerprint} node_id={node_id}"
    );

    DeviceFingerprint {
        fingerprint,
        node_id,
        node_name,
        device_platform: platform,
    }
}

/// 将 node_id 写回全局 config.toml（仅当原来为空时）。
///
/// 使用基于行的文本替换，避免对整个 TOML 文件做结构化解析写入
/// （防止丢失注释和格式）。
pub fn persist_node_id_to_config(config_path: &Path, node_id: &str) -> std::io::Result<bool> {
    let content = std::fs::read_to_string(config_path)?;

    // 检查 [huanxing] section 中是否已有 node_id
    if has_node_id_set(&content) {
        tracing::debug!(
            "[DeviceFingerprint] node_id 已存在于 config.toml，跳过写入"
        );
        return Ok(false);
    }

    // 在 [huanxing] section 下插入 node_id
    let updated = inject_node_id(&content, node_id);
    std::fs::write(config_path, &updated)?;

    tracing::info!(
        "[DeviceFingerprint] node_id={node_id} 已写入 config.toml: {}",
        config_path.display()
    );
    Ok(true)
}

/// 检查 content 中 [huanxing] section 是否已有非空 node_id
fn has_node_id_set(content: &str) -> bool {
    let mut in_huanxing = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            // 进入新 section
            in_huanxing = trimmed == "[huanxing]";
            continue;
        }
        if in_huanxing {
            // node_id = "..." 或 server_id = "..."（别名）
            if trimmed.starts_with("node_id") || trimmed.starts_with("server_id") {
                // 提取等号后面的值
                if let Some(val_part) = trimmed.splitn(2, '=').nth(1) {
                    let val = val_part.trim().trim_matches('"').trim();
                    if !val.is_empty() {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// 在 [huanxing] section 第一行后插入 node_id（如果该 section 已存在），
/// 否则在文件末尾追加。
fn inject_node_id(content: &str, node_id: &str) -> String {
    let mut result = String::with_capacity(content.len() + 64);
    let mut injected = false;

    for line in content.lines() {
        result.push_str(line);
        result.push('\n');

        // 在 [huanxing] 这一行之后立即注入
        if !injected && line.trim() == "[huanxing]" {
            result.push_str(&format!("node_id = \"{node_id}\"\n"));
            injected = true;
        }
    }

    if !injected {
        // [huanxing] section 不存在，追加到末尾
        result.push_str(&format!("\n[huanxing]\nnode_id = \"{node_id}\"\n"));
    }

    result
}

/// 确保 node_id 已在 config 和 config.toml 文件中都存在。
///
/// 调用方式（在 HuanXingConfig 初始化后立即调用）：
/// ```ignore
/// let fp = ensure_node_id(&mut config.huanxing, &config.config_path);
/// ```
///
/// 返回当前有效的 DeviceFingerprint（无论是新生成还是已有的）。
pub fn ensure_node_id(
    node_id_slot: &mut Option<String>,
    config_path: &Path,
) -> DeviceFingerprint {
    let fp = generate_device_fingerprint();

    if node_id_slot.as_deref().map(|s| s.is_empty()).unwrap_or(true) {
        // 内存中也没有，写入内存 + 文件
        *node_id_slot = Some(fp.node_id.clone());

        // 写回文件（尽力，失败不 panic）
        if let Err(e) = persist_node_id_to_config(config_path, &fp.node_id) {
            tracing::warn!("[DeviceFingerprint] 写入 config.toml 失败: {e}");
        }
    } else {
        tracing::debug!(
            "[DeviceFingerprint] config 中已有 node_id={}, 保留不覆盖",
            node_id_slot.as_deref().unwrap_or("")
        );
    }

    fp
}

// ── 平台信息 ──────────────────────────────────────────────

/// 获取操作系统版本字符串
fn get_os_version() -> String {
    #[cfg(target_os = "macos")]
    {
        // sw_vers -productVersion → "14.4.1"
        if let Ok(output) = std::process::Command::new("sw_vers")
            .arg("-productVersion")
            .output()
        {
            let v = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !v.is_empty() {
                return v;
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        // registry or ver command
        if let Ok(output) = std::process::Command::new("cmd")
            .args(["/C", "ver"])
            .output()
        {
            let v = String::from_utf8_lossy(&output.stdout).trim().to_string();
            // e.g. "Microsoft Windows [Version 10.0.22631.4751]"
            if let Some(start) = v.find("Version ") {
                let ver_part = &v[start + 8..];
                if let Some(end) = ver_part.find(']') {
                    return ver_part[..end].to_string();
                }
            }
            return v;
        }
    }
    #[cfg(target_os = "linux")]
    {
        // /etc/os-release → PRETTY_NAME
        if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
            for line in content.lines() {
                if let Some(val) = line.strip_prefix("VERSION_ID=") {
                    return val.trim().trim_matches('"').to_string();
                }
            }
        }
    }
    "unknown".to_string()
}

/// 返回 (显示名称, 平台标识)
fn platform_display(os: &str, _version: &str) -> (String, String) {
    match os {
        "macos" => ("macOS".to_string(), "macos".to_string()),
        "windows" => ("Windows".to_string(), "windows".to_string()),
        "linux" => ("Linux".to_string(), "linux".to_string()),
        "ios" => ("iOS".to_string(), "ios".to_string()),
        "android" => ("Android".to_string(), "android".to_string()),
        _ => (os.to_string(), os.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fingerprint_is_deterministic() {
        let fp1 = generate_device_fingerprint();
        let fp2 = generate_device_fingerprint();
        assert_eq!(fp1.fingerprint, fp2.fingerprint, "指纹必须在同一机器上保持稳定");
        assert_eq!(fp1.node_id, fp2.node_id, "node_id 必须确定性派生");
        assert!(fp1.node_id.starts_with("n_"), "node_id 必须以 n_ 开头");
        assert_eq!(fp1.node_id.len(), 2 + 16, "node_id 应为 n_ + 16字符");
        assert!(!fp1.node_name.is_empty(), "node_name 不应为空");
    }

    #[test]
    fn test_has_node_id_set() {
        let with_id = "[huanxing]\nnode_id = \"n_abc123\"\n";
        assert!(has_node_id_set(with_id));

        let without_id = "[huanxing]\nenabled = true\n";
        assert!(!has_node_id_set(without_id));

        let empty_id = "[huanxing]\nnode_id = \"\"\n";
        assert!(!has_node_id_set(empty_id));
    }

    #[test]
    fn test_inject_node_id() {
        let content = "[huanxing]\nenabled = true\n";
        let result = inject_node_id(content, "n_test123456");
        assert!(result.contains("node_id = \"n_test123456\""));
        // node_id 应该在 [huanxing] 之后立即出现
        let lines: Vec<&str> = result.lines().collect();
        let hi = lines.iter().position(|l| *l == "[huanxing]").unwrap();
        assert_eq!(lines[hi + 1], "node_id = \"n_test123456\"");
    }
}
