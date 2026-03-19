//! Secret management tools for user agents.
//!
//! Users can set, list, and delete API keys and other secrets
//! through their Agent's conversation interface. Secrets are stored
//! in the workspace `.env` file and injected as environment variables
//! when skill tools execute.

use crate::tools::traits::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Get the effective workspace dir for the current tenant.
fn tenant_workspace(fallback: &Path) -> PathBuf {
    fallback.to_path_buf()
}

/// Read all key-value pairs from a .env file.
fn read_dotenv(path: &Path) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return map,
    };
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim().to_string();
            let mut value = value.trim().to_string();
            // Remove surrounding quotes
            if (value.starts_with('"') && value.ends_with('"'))
                || (value.starts_with('\'') && value.ends_with('\''))
            {
                if value.len() >= 2 {
                    value = value[1..value.len() - 1].to_string();
                }
            }
            if !key.is_empty() {
                map.insert(key, value);
            }
        }
    }
    map
}

/// Write key-value pairs back to a .env file, preserving comments.
fn write_dotenv(path: &Path, secrets: &BTreeMap<String, String>) -> std::io::Result<()> {
    // Read existing file to preserve comments
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let mut comments = Vec::new();
    for line in existing.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            comments.push(line.to_string());
        }
    }

    let mut output = String::new();

    // Write header if no comments exist
    if comments.is_empty() {
        output.push_str("# Workspace secrets (.env)\n");
        output.push_str("# Managed by hx_set_secret / hx_delete_secret\n");
        output.push_str("# DO NOT commit this file to version control\n");
        output.push('\n');
    } else {
        for c in &comments {
            output.push_str(c);
            output.push('\n');
        }
        if !output.ends_with("\n\n") {
            output.push('\n');
        }
    }

    // Write secrets sorted by key
    for (key, value) in secrets {
        // Quote values containing spaces or special chars
        if value.contains(' ') || value.contains('#') || value.contains('\'') {
            output.push_str(&format!("{}=\"{}\"\n", key, value));
        } else {
            output.push_str(&format!("{}={}\n", key, value));
        }
    }

    std::fs::write(path, output)
}

/// Mask a secret value for display (show first 4 + last 2 chars).
fn mask_value(value: &str) -> String {
    if value.len() <= 8 {
        return "****".to_string();
    }
    let prefix = &value[..4];
    let suffix = &value[value.len() - 2..];
    format!("{}...{}", prefix, suffix)
}

// ═══════════════════════════════════════════════════════
// hx_set_secret
// ═══════════════════════════════════════════════════════

pub struct HxSetSecret {
    pub workspace_dir: PathBuf,
}

#[async_trait]
impl Tool for HxSetSecret {
    fn name(&self) -> &str {
        "hx_set_secret"
    }

    fn description(&self) -> &str {
        "设置用户密钥（API Key 等）。存储在 workspace .env 文件中，技能执行时自动注入为环境变量。"
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "key": {
                    "type": "string",
                    "description": "密钥名称（环境变量名），如 TAVILY_API_KEY"
                },
                "value": {
                    "type": "string",
                    "description": "密钥值"
                }
            },
            "required": ["key", "value"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let key = args["key"].as_str().unwrap_or("").trim().to_uppercase();
        let value = args["value"].as_str().unwrap_or("").trim().to_string();

        if key.is_empty() {
            return Ok(ToolResult {
                output: "❌ 请提供密钥名称 (key)".into(),
                success: false,
                error: None,
            });
        }
        if value.is_empty() {
            return Ok(ToolResult {
                output: "❌ 请提供密钥值 (value)".into(),
                success: false,
                error: None,
            });
        }

        // Validate key format (only alphanumeric + underscore)
        if !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Ok(ToolResult {
                output: "❌ 密钥名称只能包含字母、数字和下划线".into(),
                success: false,
                error: None,
            });
        }

        let ws = tenant_workspace(&self.workspace_dir);
        let env_path = ws.join(".env");

        let mut secrets = read_dotenv(&env_path);
        let is_update = secrets.contains_key(&key);
        secrets.insert(key.clone(), value.clone());

        match write_dotenv(&env_path, &secrets) {
            Ok(()) => {
                let masked = mask_value(&value);
                let action = if is_update { "更新" } else { "设置" };
                tracing::info!(key = %key, workspace = %ws.display(), "Secret set");
                Ok(ToolResult {
                    output: format!(
                        "✅ 已{}密钥 `{}`\n值: `{}`\n\n技能执行时将自动注入为环境变量。",
                        action, key, masked
                    ),
                    success: true,
                    error: None,
                })
            }
            Err(e) => Ok(ToolResult {
                output: format!("❌ 保存失败: {e}"),
                success: false,
                error: Some(e.to_string()),
            }),
        }
    }
}

// ═══════════════════════════════════════════════════════
// hx_list_secrets
// ═══════════════════════════════════════════════════════

pub struct HxListSecrets {
    pub workspace_dir: PathBuf,
}

#[async_trait]
impl Tool for HxListSecrets {
    fn name(&self) -> &str {
        "hx_list_secrets"
    }

    fn description(&self) -> &str {
        "列出用户已配置的密钥（不显示值，只显示名称和脱敏状态）"
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _args: Value) -> anyhow::Result<ToolResult> {
        let ws = tenant_workspace(&self.workspace_dir);
        let env_path = ws.join(".env");

        let secrets = read_dotenv(&env_path);

        if secrets.is_empty() {
            return Ok(ToolResult {
                output: "📋 暂无配置任何密钥。\n\n使用 `hx_set_secret` 可以添加 API Key。".into(),
                success: true,
                error: None,
            });
        }

        let mut output = format!("📋 已配置 {} 个密钥：\n\n", secrets.len());
        for (key, value) in &secrets {
            output.push_str(&format!(
                "  • `{}` = `{}`\n",
                key,
                mask_value(value)
            ));
        }
        output.push_str("\n使用 `hx_set_secret` 更新，`hx_delete_secret` 删除。");

        Ok(ToolResult {
            output,
            success: true,
            error: None,
        })
    }
}

// ═══════════════════════════════════════════════════════
// hx_delete_secret
// ═══════════════════════════════════════════════════════

pub struct HxDeleteSecret {
    pub workspace_dir: PathBuf,
}

#[async_trait]
impl Tool for HxDeleteSecret {
    fn name(&self) -> &str {
        "hx_delete_secret"
    }

    fn description(&self) -> &str {
        "删除用户的一个密钥"
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "key": {
                    "type": "string",
                    "description": "要删除的密钥名称"
                }
            },
            "required": ["key"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let key = args["key"].as_str().unwrap_or("").trim().to_uppercase();

        if key.is_empty() {
            return Ok(ToolResult {
                output: "❌ 请提供要删除的密钥名称 (key)".into(),
                success: false,
                error: None,
            });
        }

        let ws = tenant_workspace(&self.workspace_dir);
        let env_path = ws.join(".env");

        let mut secrets = read_dotenv(&env_path);
        if secrets.remove(&key).is_none() {
            return Ok(ToolResult {
                output: format!("❌ 密钥 `{}` 不存在", key),
                success: false,
                error: None,
            });
        }

        match write_dotenv(&env_path, &secrets) {
            Ok(()) => {
                tracing::info!(key = %key, workspace = %ws.display(), "Secret deleted");
                Ok(ToolResult {
                    output: format!("✅ 已删除密钥 `{}`", key),
                    success: true,
                    error: None,
                })
            }
            Err(e) => Ok(ToolResult {
                output: format!("❌ 保存失败: {e}"),
                success: false,
                error: Some(e.to_string()),
            }),
        }
    }
}
