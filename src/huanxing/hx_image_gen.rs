use crate::security::SecurityPolicy;
use crate::security::policy::ToolOperation;
use crate::tools::traits::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

/// HuanXing specific image generation tool calling a custom OpenAI-compatible gateway.
///
/// Supports an array of models for fallback. If a model fails to generate an image,
/// it will attempt the next one in the priority list.
pub struct HxImageGenTool {
    security: Arc<SecurityPolicy>,
    workspace_dir: PathBuf,
    models: Vec<String>,
    api_url: String,
    api_key: String,
}

impl HxImageGenTool {
    pub fn new(
        security: Arc<SecurityPolicy>,
        workspace_dir: PathBuf,
        models: Vec<String>,
        api_url: String,
        api_key: String,
    ) -> Self {
        Self {
            security,
            workspace_dir,
            models,
            api_url,
            api_key,
        }
    }

    /// Build a reusable HTTP client with reasonable timeouts.
    fn http_client() -> reqwest::Client {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .unwrap_or_default()
    }

    /// Read api_key from a tenant's workspace config.toml at runtime.
    async fn resolve_tenant_api_key(workspace_dir: &std::path::Path) -> Option<String> {
        let _ = crate::huanxing::config::promote_legacy_agent_config_from_workspace(workspace_dir);
        let canonical_path =
            crate::huanxing::config::agent_config_path_from_workspace(workspace_dir);
        let legacy_path = workspace_dir.join("config.toml");
        let config_path = if canonical_path.exists() {
            canonical_path
        } else {
            legacy_path
        };
        let content = tokio::fs::read_to_string(&config_path).await.ok()?;
        // Simple partial deserialize to extract api_key
        #[derive(serde::Deserialize)]
        struct Partial {
            api_key: Option<String>,
        }
        let parsed: Partial = toml::from_str(&content).ok()?;
        parsed.api_key.filter(|k| !k.is_empty())
    }

    /// Core generation logic: try models sequentially until success.
    async fn generate(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        // ── Parse parameters ───────────────────────────────────────
        let prompt = match args.get("prompt").and_then(|v| v.as_str()) {
            Some(p) if !p.trim().is_empty() => p.trim().to_string(),
            _ => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("Missing required parameter: 'prompt'".into()),
                });
            }
        };

        let filename = args
            .get("filename")
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("generated_image");

        // Sanitize filename — strip path components to prevent traversal.
        let safe_name = PathBuf::from(filename).file_name().map_or_else(
            || "generated_image".to_string(),
            |n| n.to_string_lossy().to_string(),
        );

        let size = args
            .get("size")
            .and_then(|v| v.as_str())
            .unwrap_or("1024x1024");

        // Allow model override from args, else use configured models
        let models_to_try = if let Some(model_arg) = args
            .get("model")
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
        {
            vec![model_arg.to_string()]
        } else {
            self.models.clone()
        };

        if models_to_try.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("No models configured for hx_image_gen".into()),
            });
        }

        // ── Resolve api_key dynamically from tenant workspace ─────
        // Priority: tenant workspace config.toml api_key > global fallback api_key
        let active_security =
            crate::tools::get_active_security().unwrap_or_else(|| self.security.clone());
        let api_key = Self::resolve_tenant_api_key(&active_security.workspace_dir)
            .await
            .or_else(|| {
                if !self.api_key.is_empty() {
                    Some(self.api_key.clone())
                } else {
                    None
                }
            });

        let api_key = match api_key {
            Some(k) if !k.is_empty() => k,
            _ => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("API Key is required for hx_image_gen. Please ensure your workspace config.toml contains an api_key.".into()),
                });
            }
        };

        tracing::info!(
            "hx_image_gen: using api_key from workspace {} (key prefix: {}...)",
            active_security.workspace_dir.display(),
            &api_key[..api_key.len().min(12)]
        );

        let client = Self::http_client();
        let url = &self.api_url;

        let mut last_error = String::new();

        // Maximum retries for rate-limited (429) requests on the same model
        const MAX_RATE_LIMIT_RETRIES: u32 = 2;
        // Base backoff delay in seconds for 429 retries
        const RATE_LIMIT_BACKOFF_BASE_SECS: u64 = 3;

        // ── Loop over models for fallback ───────────────────────────
        for model in &models_to_try {
            tracing::info!("hx_image_gen: attempting generation with model '{}'", model);

            let body = json!({
                "model": model,
                "prompt": prompt,
                "size": size,
                "n": 1
            });

            // Inner retry loop for rate-limit (429) errors on the same model
            let mut attempt = 0u32;
            let resp_json: serde_json::Value = loop {
                attempt += 1;

                let resp_result = client
                    .post(url)
                    .header("Authorization", format!("Bearer {}", api_key))
                    .header("Content-Type", "application/json")
                    .json(&body)
                    .send()
                    .await;

                match resp_result {
                    Ok(resp) => {
                        let status = resp.status();
                        if status.is_success() {
                            // Parse JSON and break out of retry loop
                            match resp.json::<serde_json::Value>().await {
                                Ok(json) => break json,
                                Err(e) => {
                                    let err_msg = format!(
                                        "Failed to parse JSON response for model {model}: {e}"
                                    );
                                    tracing::warn!("hx_image_gen: {}", err_msg);
                                    last_error = err_msg;
                                    break serde_json::Value::Null; // will be caught below
                                }
                            }
                        }

                        let body_text = resp.text().await.unwrap_or_default();

                        // 429 = rate limit → retry with backoff on the SAME model
                        if status.as_u16() == 429 && attempt <= MAX_RATE_LIMIT_RETRIES {
                            let delay = RATE_LIMIT_BACKOFF_BASE_SECS * attempt as u64;
                            tracing::warn!(
                                "hx_image_gen: rate limited (429) on model {model}, retry {attempt}/{MAX_RATE_LIMIT_RETRIES} after {delay}s"
                            );
                            tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                            continue; // retry same model
                        }

                        // Non-retryable error or retries exhausted → fall through to next model
                        let err_msg =
                            format!("API error ({status}) with model {model}: {body_text}");
                        tracing::warn!("hx_image_gen failed: {}", err_msg);
                        last_error = err_msg;
                        break serde_json::Value::Null;
                    }
                    Err(e) => {
                        let err_msg = format!("Request failed with model {model}: {e}");
                        tracing::warn!("hx_image_gen failed: {}", err_msg);
                        last_error = err_msg;
                        break serde_json::Value::Null;
                    }
                }
            };

            // If we broke out with Null, skip to next model
            if resp_json.is_null() {
                continue;
            }

            // ── Successful response — extract image ─────────────────
            let image_url = resp_json.pointer("/data/0/url").and_then(|v| v.as_str());
            let b64_json = resp_json
                .pointer("/data/0/b64_json")
                .and_then(|v| v.as_str());

            let bytes = if let Some(u) = image_url {
                // ── Download image URL ─────────────────────────────────
                let img_resp = match client.get(u).send().await {
                    Ok(r) => r,
                    Err(e) => {
                        let err_msg = format!(
                            "Failed to download image from {} (model {}): {}",
                            u, model, e
                        );
                        tracing::warn!("hx_image_gen failed: {}", err_msg);
                        last_error = err_msg;
                        continue;
                    }
                };

                if !img_resp.status().is_success() {
                    let err_msg = format!(
                        "Failed to download image from {} (status {})",
                        u,
                        img_resp.status()
                    );
                    tracing::warn!("hx_image_gen failed: {}", err_msg);
                    last_error = err_msg;
                    continue;
                }

                match img_resp.bytes().await {
                    Ok(b) => b.to_vec(),
                    Err(e) => {
                        let err_msg =
                            format!("Failed to read image bytes (model {}): {}", model, e);
                        tracing::warn!("hx_image_gen failed: {}", err_msg);
                        last_error = err_msg;
                        continue;
                    }
                }
            } else if let Some(b64) = b64_json {
                use base64::{Engine as _, engine::general_purpose};
                match general_purpose::STANDARD.decode(b64) {
                    Ok(decoded) => decoded,
                    Err(e) => {
                        let err_msg =
                            format!("Failed to decode base64 image (model {}): {}", model, e);
                        tracing::warn!("hx_image_gen failed: {}", err_msg);
                        last_error = err_msg;
                        continue;
                    }
                }
            } else {
                let err_msg = format!(
                    "No image URL or b64_json in API response for model {}",
                    model
                );
                tracing::warn!("hx_image_gen failed: {}", err_msg);
                last_error = err_msg;
                continue;
            };

            // ── Save to disk ───────────────────────────────────────────
            let images_dir = active_security.workspace_dir.join("images");

            if let Err(e) = tokio::fs::create_dir_all(&images_dir).await {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Failed to create images directory: {}", e)),
                });
            }

            let output_path = images_dir.join(format!("{safe_name}.png"));
            if let Err(e) = tokio::fs::write(&output_path, &bytes).await {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Failed to write image file: {}", e)),
                });
            }

            let size_kb = bytes.len() / 1024;

            return Ok(ToolResult {
                success: true,
                output: format!(
                    "Image generated successfully.\n\
                     File: {}\n\
                     Size: {} KB\n\
                     Model: {}\n\
                     Prompt: {}\n\n\
                     VERY IMPORTANT: To display this image to the user, you MUST reply with exactly:\n\
                     ![{}]({})\n\
                     Do NOT use a relative path. You MUST use the exact absolute path shown above.",
                    output_path.display(),
                    size_kb,
                    model,
                    prompt,
                    safe_name,
                    output_path.display(),
                ),
                error: None,
            });
        }

        // ── All models failed ───────────────────────────────────────────
        Ok(ToolResult {
            success: false,
            output: String::new(),
            error: Some(format!(
                "All hx_image_gen models failed. Last error: {}",
                last_error
            )),
        })
    }
}

#[async_trait]
impl Tool for HxImageGenTool {
    fn name(&self) -> &str {
        "hx_image_gen"
    }

    fn description(&self) -> &str {
        "使用外部网关生成图像。支持传递提示词生成图片，自动降级模型直至成功。返回保存到工作区的图片路径。"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "required": ["prompt"],
            "properties": {
                "prompt": {
                    "type": "string",
                    "description": "描述要生成图片的文本提示词。"
                },
                "filename": {
                    "type": "string",
                    "description": "保存的文件名（不包含扩展名），默认是 'generated_image'。图片将被保存到工作区的 images 目录下。"
                },
                "size": {
                    "type": "string",
                    "description": "图片尺寸，如 '1024x1024' (默认)。"
                },
                "model": {
                    "type": "string",
                    "description": "可选的模型名称。如果提供，将覆盖配置中默认的模型及降级顺序。"
                }
            }
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        if let Err(error) = self
            .security
            .enforce_tool_operation(ToolOperation::Act, "hx_image_gen")
        {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(error),
            });
        }

        self.generate(args).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::{AutonomyLevel, SecurityPolicy};

    fn test_security() -> Arc<SecurityPolicy> {
        Arc::new(SecurityPolicy {
            autonomy: AutonomyLevel::Full,
            workspace_dir: std::env::temp_dir(),
            ..SecurityPolicy::default()
        })
    }

    fn test_tool() -> HxImageGenTool {
        HxImageGenTool::new(
            test_security(),
            std::env::temp_dir(),
            vec!["dall-e-3".into()],
            "https://api.openai.com/v1/images/generations".into(),
            "dummy_key".into(),
        )
    }

    #[test]
    fn tool_name() {
        let tool = test_tool();
        assert_eq!(tool.name(), "hx_image_gen");
    }

    #[tokio::test]
    async fn missing_prompt_returns_error() {
        let tool = test_tool();
        let result = tool.execute(json!({})).await.unwrap();
        assert!(!result.success);
        assert!(result.error.as_deref().unwrap().contains("prompt"));
    }

    #[tokio::test]
    async fn missing_api_key_returns_error() {
        let tool = HxImageGenTool::new(
            test_security(),
            std::env::temp_dir(),
            vec!["dall-e-3".into()],
            "https://api.openai.com/v1/images/generations".into(),
            "".into(),
        );
        let result = tool.execute(json!({"prompt": "test"})).await.unwrap();
        assert!(!result.success);
        assert!(
            result
                .error
                .as_deref()
                .unwrap()
                .contains("API Key is required")
        );
    }

    #[tokio::test]
    async fn resolve_tenant_api_key_prefers_wrapper_config() {
        let temp = tempfile::tempdir().unwrap();
        let wrapper = temp.path().join("agents").join("default");
        let workspace = wrapper.join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();

        tokio::fs::write(wrapper.join("config.toml"), "api_key = \"wrapper-key\"\n")
            .await
            .unwrap();
        tokio::fs::write(
            workspace.join("config.toml"),
            "api_key = \"legacy-workspace-key\"\n",
        )
        .await
        .unwrap();

        let api_key = HxImageGenTool::resolve_tenant_api_key(&workspace).await;

        assert_eq!(api_key.as_deref(), Some("wrapper-key"));
    }
}
