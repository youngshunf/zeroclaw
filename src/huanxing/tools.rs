//! HuanXing registration, tenant management and admin tools.
//!
//! Organized by priority:
//! - **P0**: lookup_sender, register_user, invalidate_cache (core flow)
//! - **P1**: get_user, send_sms, verify_sms, check_quota, get_subscription,
//!           usage_stats, local_find_user, local_bind_channel (business)
//! - **P2**: document tools (Phase 3 — separate module)
//! - **P3**: HASN social tools (Phase 4 — separate module)

use super::registry::RegistryLoader;
use crate::huanxing::api_client::ApiClient;
use crate::huanxing::db::TenantDb;
use crate::huanxing::router::TenantRouter;
use crate::huanxing::templates::{TemplateEngine, UserInfo, WorkspaceVariant};
use crate::tools::traits::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

/// Normalize channel type aliases.
fn normalize_channel(raw: &str) -> &str {
    match raw {
        "onebot" => "napcat",
        "qq" => "qqbot",
        "feishu" => "lark",
        other => other,
    }
}

// ═══════════════════════════════════════════════════════
// P0 — Core Flow
// ═══════════════════════════════════════════════════════

// ── hx_lookup_sender ─────────────────────────────────

/// Look up a sender's registration status by channel + sender_id.
pub struct HxLookupSender {
    db: TenantDb,
}

impl HxLookupSender {
    pub fn new(db: TenantDb) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Tool for HxLookupSender {
    fn name(&self) -> &str {
        "hx_lookup_sender"
    }

    fn description(&self) -> &str {
        "查询发送者是否已注册。输入 channel_type 和 sender_id，返回用户信息或未注册状态。"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "channel_type": {
                    "type": "string",
                    "description": "渠道类型: napcat / lark / qq"
                },
                "sender_id": {
                    "type": "string",
                    "description": "发送者在渠道中的 ID"
                }
            },
            "required": ["channel_type", "sender_id"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let channel_type = normalize_channel(args["channel_type"].as_str().unwrap_or_default());
        let sender_id = args["sender_id"].as_str().unwrap_or_default();

        if channel_type.is_empty() || sender_id.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("channel_type and sender_id are required".to_string()),
            });
        }

        match self.db.find_by_channel(channel_type, sender_id).await {
            Ok(Some(record)) => Ok(ToolResult {
                success: true,
                output: json!({
                    "registered": true,
                    "user_id": record.user_id,
                    "agent_id": record.agent_id,
                    "nickname": record.nickname,
                    "template": record.template,
                    "plan": record.plan,
                    "star_name": record.star_name,
                })
                .to_string(),
                error: None,
            }),
            Ok(None) => Ok(ToolResult {
                success: true,
                output: json!({
                    "registered": false,
                    "channel_type": channel_type,
                    "sender_id": sender_id,
                })
                .to_string(),
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Database error: {e}")),
            }),
        }
    }
}

// ── hx_register_user ─────────────────────────────────

/// Register a new user: consume verified credentials, create DB record + full workspace + channel binding + LLM config.
pub struct HxRegisterUser {
    db: TenantDb,
    api: ApiClient,
    agents_dir: PathBuf,
    common_skills_dir: PathBuf,
    template_engine: TemplateEngine,
    default_template: String,
    default_provider: Option<String>,
    llm_base_url: Option<String>,
    server_id: String,
    router: Arc<TenantRouter>,
}

impl HxRegisterUser {
    pub fn new(
        db: TenantDb,
        api: ApiClient,
        agents_dir: PathBuf,
        common_skills_dir: PathBuf,
        templates_dir: PathBuf,
        default_template: String,
        default_provider: Option<String>,
        llm_base_url: Option<String>,
        server_id: String,
        router: Arc<TenantRouter>,
    ) -> Self {
        Self {
            db,
            api,
            agents_dir,
            common_skills_dir,
            template_engine: TemplateEngine::new(templates_dir),
            default_template,
            default_provider,
            llm_base_url,
            server_id,
            router,
        }
    }

    /// Create with hub registry support for skill installation from marketplace.
    pub fn with_registry(
        db: TenantDb,
        api: ApiClient,
        agents_dir: PathBuf,
        common_skills_dir: PathBuf,
        templates_dir: PathBuf,
        default_template: String,
        default_provider: Option<String>,
        llm_base_url: Option<String>,
        server_id: String,
        router: Arc<TenantRouter>,
        registry: Arc<RegistryLoader>,
    ) -> Self {
        Self {
            db,
            api,
            agents_dir,
            common_skills_dir,
            template_engine: TemplateEngine::with_registry(templates_dir, registry),
            default_template,
            default_provider,
            llm_base_url,
            server_id,
            router,
        }
    }
}

#[async_trait]
impl Tool for HxRegisterUser {
    fn name(&self) -> &str {
        "hx_register_user"
    }

    fn description(&self) -> &str {
        "创建新用户的 Agent。必须先调用 hx_verify_sms 验证手机号。\n\n内部自动完成：消费验证凭证 → 创建工作区 + 安装模板技能 → 配置 LLM → 保存数据库 + 路由 → 同步后端。\n凭证从数据库读取（verify_sms 已存入），无需手动传递。"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "phone": {
                    "type": "string",
                    "description": "手机号（必须先通过 hx_verify_sms 验证）"
                },
                "channel_type": {
                    "type": "string",
                    "description": "消息来源渠道名（napcat / lark / onebot / qq / feishu）"
                },
                "sender_id": {
                    "type": "string",
                    "description": "渠道用户ID（QQ号/飞书open_id）"
                },
                "nickname": {
                    "type": "string",
                    "description": "用户昵称（可选，默认'主人'）"
                },
                "star_name": {
                    "type": "string",
                    "description": "用户给 AI 助手起的名字（可选，默认为'小星'）"
                },
                "template": {
                    "type": "string",
                    "description": "模板: media-creator / side-hustle / finance / office / health / assistant（可选）"
                }
            },
            "required": ["phone", "channel_type", "sender_id"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let phone = args["phone"].as_str().unwrap_or_default();
        let nickname = args["nickname"].as_str().unwrap_or("主人");
        let channel_type = normalize_channel(
            args["channel_type"]
                .as_str()
                .or_else(|| args["channel"].as_str())
                .unwrap_or_default(),
        );
        let sender_id = args["sender_id"]
            .as_str()
            .or_else(|| args["peerId"].as_str())
            .or_else(|| args["peer_id"].as_str())
            .unwrap_or_default();
        let star_name = args["star_name"].as_str().unwrap_or("小星");
        let template = args["template"].as_str().unwrap_or(&self.default_template);

        if phone.is_empty() || channel_type.is_empty() || sender_id.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("phone, channel_type and sender_id are required".to_string()),
            });
        }

        let mut steps: Vec<String> = Vec::new();

        // Step 1: Consume verified credentials from DB
        let credentials = match self.db.consume_verified_credentials(phone).await {
            Ok(Some(creds)) => {
                steps.push(format!("✅ Step1: 凭证验证通过 (userId={})", creds.user_id));
                creds
            }
            Ok(None) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("未找到验证凭证，请先调用 hx_verify_sms 验证手机号".to_string()),
                });
            }
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("读取凭证失败: {e}")),
                });
            }
        };

        let user_id = &credentials.user_id;
        let llm_token = credentials.llm_token.as_deref().unwrap_or_default();
        let access_token = credentials.access_token.as_str();
        let gateway_token = credentials.gateway_token.as_deref().unwrap_or_default();

        // Generate agent ID
        let seq = self.db.get_next_user_seq().await.unwrap_or(1);
        let agent_id = format!("{seq:03}-{phone}-{template}");
        let workspace = self.agents_dir.join(&agent_id);

        // Step 2: Save user to local DB with tokens + channel binding + routing
        match self
            .db
            .save_user_full(
                user_id,
                phone,
                &agent_id,
                Some(nickname),
                template,
                Some(star_name),
                Some(&workspace.to_string_lossy()),
                Some(access_token),
                Some(llm_token),
                Some(gateway_token),
                Some(&self.server_id),
            )
            .await
        {
            Ok(()) => {}
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("保存用户记录失败: {e}")),
                });
            }
        }

        // Bind channel
        if let Err(e) = self
            .db
            .bind_channel(user_id, channel_type, sender_id, None)
            .await
        {
            tracing::warn!("Failed to bind channel: {e}");
        }

        // Add routing
        if let Err(e) = self
            .db
            .add_routing(&agent_id, channel_type, sender_id)
            .await
        {
            tracing::warn!("Failed to add routing: {e}");
        }

        steps.push("✅ Step2: 本地数据保存 + 渠道绑定完成".to_string());

        // Step 2.5: Sync to backend
        match self
            .api
            .agent_post(
                "/api/v1/huanxing/agent/users",
                &json!({
                    "user_id": user_id,
                    "server_id": self.server_id,
                    "agent_id": agent_id,
                    "star_name": star_name,
                    "template": template,
                    "channel_type": channel_type,
                    "channel_peer_id": sender_id,
                }),
            )
            .await
        {
            Ok(_) => steps.push("✅ Step2.5: 后端用户同步完成".to_string()),
            Err(e) => {
                let msg = e.to_string();
                tracing::error!("后端同步失败: {msg}");
                steps.push(format!("⚠️ Step2.5: 后端同步失败 ({msg})，不影响本地使用"));
            }
        }

        // Step 3: Create workspace using TemplateEngine
        let user_info = UserInfo {
            nickname,
            phone,
            star_name,
            user_id,
            agent_id: &agent_id,
            template,
        };

        // LLM config: use llm_token as api_key if available
        let provider = self.default_provider.as_deref();
        let api_key = if llm_token.is_empty() {
            None
        } else {
            Some(llm_token)
        };

        match self
            .template_engine
            .create_workspace(&workspace, &user_info, provider, api_key, WorkspaceVariant::Cloud)
            .await
        {
            Ok(files) => {
                steps.push(format!(
                    "✅ Step3: 工作区创建完成 (模板={template}, 文件={})",
                    files.len()
                ));
            }
            Err(e) => {
                steps.push(format!("⚠️ Step3: 工作区创建失败 ({e})"));
            }
        }

        // Step 3.5: Configure LLM gateway (models.json + auth-profiles.json)
        if !llm_token.is_empty() {
            let provider_for_llm = self
                .default_provider
                .as_deref()
                .unwrap_or("anthropic-custom:https://llm.dcfuture.cn");
            match configure_agent_llm(&workspace, llm_token, provider_for_llm).await {
                Ok(()) => {
                    steps.push(format!(
                        "✅ Step3.5: LLM 网关配置完成 (provider={provider_for_llm})"
                    ));
                }
                Err(e) => {
                    steps.push(format!("⚠️ Step3.5: LLM 配置失败 ({e})"));
                }
            }
        } else {
            steps.push("⚠️ Step3.5: 无 llm_token，跳过 LLM 配置".to_string());
        }

        // Step 5: Invalidate router cache so next message routes correctly
        self.router.invalidate(channel_type, sender_id);
        steps.push("✅ Step5: 路由缓存已清除".to_string());

        Ok(ToolResult {
            success: true,
            output: json!({
                "success": true,
                "message": "🎉 注册完成！",
                "agentId": agent_id,
                "template": template,
                "steps": steps,
            })
            .to_string(),
            error: None,
        })
    }
}

// ── LLM config helper ───────────────────────────────

/// Configure per-agent LLM settings by writing config.toml with proper credentials.
/// `default_provider` should be the full provider string (e.g. "anthropic-custom:https://llm.dcfuture.cn").
async fn configure_agent_llm(
    workspace: &std::path::Path,
    llm_token: &str,
    default_provider: &str,
) -> anyhow::Result<()> {
    let config_path = workspace.join("config.toml");
    if config_path.exists() {
        // Read existing config and update api_key
        let content = tokio::fs::read_to_string(&config_path).await?;
        let updated = if content.contains("api_key = ") {
            // Replace existing api_key line
            let re = regex::Regex::new(r#"api_key\s*=\s*"[^"]*""#)?;
            re.replace(&content, &format!(r#"api_key = "{llm_token}""#))
                .to_string()
        } else {
            // Append api_key
            format!("{content}\napi_key = \"{llm_token}\"\n")
        };

        // Also ensure default_provider points to LLM gateway (use full provider string from config)
        let updated = if updated.contains("default_provider = ") {
            let re2 = regex::Regex::new(r#"default_provider\s*=\s*"[^"]*""#)?;
            re2.replace(
                &updated,
                &format!(r#"default_provider = "{default_provider}""#),
            )
            .to_string()
        } else {
            format!("{updated}\ndefault_provider = \"{default_provider}\"\n")
        };

        tokio::fs::write(&config_path, updated).await?;
    }
    Ok(())
}

// ── hx_invalidate_cache ─────────────────────────────

/// Invalidate tenant routing cache (admin tool).
pub struct HxInvalidateCache {
    router: Arc<TenantRouter>,
}

impl HxInvalidateCache {
    pub fn new(router: Arc<TenantRouter>) -> Self {
        Self { router }
    }
}

#[async_trait]
impl Tool for HxInvalidateCache {
    fn name(&self) -> &str {
        "hx_invalidate_cache"
    }

    fn description(&self) -> &str {
        "清除租户路由缓存。在用户信息变更后使用。"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "channel_type": {
                    "type": "string",
                    "description": "渠道类型（可选，不传则清除全部缓存）"
                },
                "sender_id": {
                    "type": "string",
                    "description": "发送者 ID（可选，需配合 channel_type）"
                },
                "user_id": {
                    "type": "string",
                    "description": "用户 ID（可选，清除该用户所有渠道缓存）"
                }
            }
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let channel_type = args["channel_type"].as_str();
        let sender_id = args["sender_id"].as_str();
        let user_id = args["user_id"].as_str();

        if let (Some(ch), Some(sid)) = (channel_type, sender_id) {
            self.router.invalidate(ch, sid);
            Ok(ToolResult {
                success: true,
                output: format!("Cache invalidated for {ch}:{sid}"),
                error: None,
            })
        } else if let Some(uid) = user_id {
            self.router.invalidate_user(uid);
            Ok(ToolResult {
                success: true,
                output: format!("Cache invalidated for user {uid}"),
                error: None,
            })
        } else {
            let size = self.router.cache_size();
            self.router.invalidate_all();
            Ok(ToolResult {
                success: true,
                output: format!("Full cache cleared ({size} entries removed)"),
                error: None,
            })
        }
    }
}

// ═══════════════════════════════════════════════════════
// P1 — Business Tools
// ═══════════════════════════════════════════════════════

// ── hx_send_sms ──────────────────────────────────────

/// Send SMS verification code via backend API.
pub struct HxSendSms {
    api: ApiClient,
}

impl HxSendSms {
    pub fn new(api: ApiClient) -> Self {
        Self { api }
    }
}

#[async_trait]
impl Tool for HxSendSms {
    fn name(&self) -> &str {
        "hx_send_sms"
    }

    fn description(&self) -> &str {
        "发送短信验证码到用户手机号。用于注册或身份验证。"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "phone": {
                    "type": "string",
                    "description": "手机号（11位中国大陆手机号）"
                }
            },
            "required": ["phone"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let phone = args["phone"].as_str().unwrap_or_default();
        if phone.is_empty() || phone.len() != 11 {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("请提供有效的11位手机号".to_string()),
            });
        }

        match self
            .api
            .open_post("/api/v1/auth/send-code", &json!({ "phone": phone }))
            .await
        {
            Ok(resp) => Ok(ToolResult {
                success: true,
                output: json!({
                    "sent": true,
                    "phone": phone,
                    "message": format!("验证码已发送到 {}", mask_phone(phone)),
                    "detail": resp,
                })
                .to_string(),
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("发送验证码失败: {e}")),
            }),
        }
    }
}

// ── hx_verify_sms ────────────────────────────────────

/// Verify SMS code via backend API.
pub struct HxVerifySms {
    api: ApiClient,
    db: TenantDb,
}

impl HxVerifySms {
    pub fn new(api: ApiClient, db: TenantDb) -> Self {
        Self { api, db }
    }
}

#[async_trait]
impl Tool for HxVerifySms {
    fn name(&self) -> &str {
        "hx_verify_sms"
    }

    fn description(&self) -> &str {
        "验证手机号并判断用户状态。验证码验证通过后，返回用户在本服务器的注册状态。\n\n凭证自动存入数据库，不需要手动传递。\n\n返回的 status 字段含义：\n- new: 新用户，可以注册\n- local_same_channel: 本服务器已注册且当前渠道已绑定，无需操作\n- local_other_channel: 本服务器已注册但当前渠道未绑定"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "phone": {
                    "type": "string",
                    "description": "手机号"
                },
                "code": {
                    "type": "string",
                    "description": "6位验证码"
                },
                "channel": {
                    "type": "string",
                    "description": "消息来源渠道名（napcat / lark / onebot / qq / feishu）"
                },
                "peerId": {
                    "type": "string",
                    "description": "渠道用户ID（QQ号/飞书open_id）"
                }
            },
            "required": ["phone", "code", "channel", "peerId"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let phone = args["phone"].as_str().unwrap_or_default();
        let code = args["code"].as_str().unwrap_or_default();
        let channel = normalize_channel(
            args["channel"]
                .as_str()
                .or_else(|| args["channel_type"].as_str())
                .unwrap_or_default(),
        );
        let peer_id = args["peerId"]
            .as_str()
            .or_else(|| args["peer_id"].as_str())
            .or_else(|| args["sender_id"].as_str())
            .unwrap_or_default();

        if phone.is_empty() || code.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("phone and code are required".to_string()),
            });
        }

        // Step 1: Call phone-login API
        let login_resp = match self
            .api
            .open_post(
                "/api/v1/auth/phone-login",
                &json!({ "phone": phone, "code": code }),
            )
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                let msg = e.to_string();
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("验证失败: {msg}")),
                });
            }
        };

        let success = login_resp["code"].as_i64() == Some(200);
        if !success {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!(
                    "验证失败: {}",
                    login_resp["msg"].as_str().unwrap_or("验证码错误或已过期")
                )),
            });
        }

        let data = &login_resp["data"];
        let user_id = data["user"]["uuid"]
            .as_str()
            .or_else(|| data["user"]["id"].as_str())
            .unwrap_or_default()
            .to_string();
        let access_token = data["access_token"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let llm_token = data["llm_token"].as_str().map(|s| s.to_string());
        let gateway_token = data["gateway_token"].as_str().map(|s| s.to_string());
        let is_new_user = data["is_new_user"].as_bool().unwrap_or(false);

        let phone_masked = if phone.len() == 11 {
            format!("{}****{}", &phone[..3], &phone[7..])
        } else {
            phone.to_string()
        };

        // Step 2: Save credentials to DB (consumed later by hx_register_user)
        if let Err(e) = self
            .db
            .save_verified_credentials(&crate::huanxing::db::VerifiedCredentials {
                phone: phone.to_string(),
                user_id: user_id.clone(),
                access_token,
                refresh_token: None,
                llm_token,
                gateway_token,
                is_new_user,
            })
            .await
        {
            tracing::error!("Failed to save verified credentials: {e}");
        }

        // Step 3: Check local DB — does this phone already have an agent?
        if let Ok(Some(local_user)) = self.db.find_by_phone(phone).await {
            if local_user.agent_id.is_empty() {
                // Has user record but no agent — treat as new
            } else {
                // Check if current channel is already bound
                if let Ok(channels) = self.db.get_channels(&local_user.user_id).await {
                    let already_bound = channels
                        .iter()
                        .any(|c| c.channel_type == channel && c.peer_id == peer_id);

                    if already_bound {
                        return Ok(ToolResult {
                            success: true,
                            output: json!({
                                "success": true,
                                "phone_masked": phone_masked,
                                "status": {
                                    "code": "local_same_channel",
                                    "agentId": local_user.agent_id,
                                }
                            })
                            .to_string(),
                            error: None,
                        });
                    } else {
                        return Ok(ToolResult {
                            success: true,
                            output: json!({
                                "success": true,
                                "phone_masked": phone_masked,
                                "status": {
                                    "code": "local_other_channel",
                                    "agentId": local_user.agent_id,
                                }
                            })
                            .to_string(),
                            error: None,
                        });
                    }
                }
            }
        }

        // Step 4: New user
        Ok(ToolResult {
            success: true,
            output: json!({
                "success": true,
                "phone_masked": phone_masked,
                "status": { "code": "new" }
            })
            .to_string(),
            error: None,
        })
    }
}

// ── hx_get_user ──────────────────────────────────────

/// Get full user info from local DB.
pub struct HxGetUser {
    db: TenantDb,
}

impl HxGetUser {
    pub fn new(db: TenantDb) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Tool for HxGetUser {
    fn name(&self) -> &str {
        "hx_get_user"
    }

    fn description(&self) -> &str {
        "获取用户完整信息（本地数据库），可通过 user_id、phone 或 agent_id 查询。"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "user_id": { "type": "string", "description": "用户 ID" },
                "phone": { "type": "string", "description": "手机号" },
                "agent_id": { "type": "string", "description": "Agent ID" }
            }
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let record = if let Some(uid) = args["user_id"].as_str() {
            self.db.get_user(uid).await?
        } else if let Some(phone) = args["phone"].as_str() {
            self.db.find_by_phone(phone).await?
        } else if let Some(aid) = args["agent_id"].as_str() {
            self.db.find_by_agent_id(aid).await?
        } else {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("请提供 user_id、phone 或 agent_id".to_string()),
            });
        };

        match record {
            Some(r) => {
                let channels = self.db.get_channels(&r.user_id).await.unwrap_or_default();
                Ok(ToolResult {
                    success: true,
                    output: json!({
                        "user_id": r.user_id,
                        "agent_id": r.agent_id,
                        "phone": r.phone,
                        "nickname": r.nickname,
                        "star_name": r.star_name,
                        "template": r.template,
                        "plan": r.plan,
                        "plan_expires": r.plan_expires,
                        "status": r.status,
                        "created_at": r.created_at,
                        "last_active": r.last_active,
                        "channels": channels.iter().map(|c| json!({
                            "type": c.channel_type,
                            "peer_id": c.peer_id,
                            "peer_name": c.peer_name,
                            "bound_at": c.bound_at,
                        })).collect::<Vec<_>>(),
                    })
                    .to_string(),
                    error: None,
                })
            }
            None => Ok(ToolResult {
                success: true,
                output: json!({ "found": false }).to_string(),
                error: None,
            }),
        }
    }
}

// ── hx_check_quota ───────────────────────────────────

/// Check user quota via backend API.
pub struct HxCheckQuota {
    api: ApiClient,
}

impl HxCheckQuota {
    pub fn new(api: ApiClient) -> Self {
        Self { api }
    }
}

#[async_trait]
impl Tool for HxCheckQuota {
    fn name(&self) -> &str {
        "hx_check_quota"
    }

    fn description(&self) -> &str {
        "检查用户配额是否充足。"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "phone": {
                    "type": "string",
                    "description": "用户手机号"
                }
            },
            "required": ["phone"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let phone = args["phone"].as_str().unwrap_or_default();
        if phone.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("phone is required".to_string()),
            });
        }

        match self
            .api
            .agent_get("/api/v1/agent/check-quota", &[("phone", phone)])
            .await
        {
            Ok(resp) => Ok(ToolResult {
                success: true,
                output: resp.to_string(),
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("查询配额失败: {e}")),
            }),
        }
    }
}

// ── hx_get_subscription ─────────────────────────────

/// Get subscription status via backend API.
pub struct HxGetSubscription {
    api: ApiClient,
}

impl HxGetSubscription {
    pub fn new(api: ApiClient) -> Self {
        Self { api }
    }
}

#[async_trait]
impl Tool for HxGetSubscription {
    fn name(&self) -> &str {
        "hx_get_subscription"
    }

    fn description(&self) -> &str {
        "查询用户订阅状态和套餐信息。"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "phone": {
                    "type": "string",
                    "description": "用户手机号"
                }
            },
            "required": ["phone"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let phone = args["phone"].as_str().unwrap_or_default();
        if phone.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("phone is required".to_string()),
            });
        }

        match self
            .api
            .agent_get("/api/v1/agent/subscription", &[("phone", phone)])
            .await
        {
            Ok(resp) => Ok(ToolResult {
                success: true,
                output: resp.to_string(),
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("查询订阅失败: {e}")),
            }),
        }
    }
}

// ── hx_usage_stats ───────────────────────────────────

/// Get usage statistics via backend API.
pub struct HxUsageStats {
    api: ApiClient,
}

impl HxUsageStats {
    pub fn new(api: ApiClient) -> Self {
        Self { api }
    }
}

#[async_trait]
impl Tool for HxUsageStats {
    fn name(&self) -> &str {
        "hx_usage_stats"
    }

    fn description(&self) -> &str {
        "查询用户使用量统计。"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "phone": {
                    "type": "string",
                    "description": "用户手机号"
                }
            },
            "required": ["phone"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let phone = args["phone"].as_str().unwrap_or_default();
        if phone.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("phone is required".to_string()),
            });
        }

        match self
            .api
            .agent_get("/api/v1/agent/usage-stats", &[("phone", phone)])
            .await
        {
            Ok(resp) => Ok(ToolResult {
                success: true,
                output: resp.to_string(),
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("查询用量失败: {e}")),
            }),
        }
    }
}

// ── hx_local_find_user ───────────────────────────────

/// Find user by phone, channel binding, or user_id in local DB.
pub struct HxLocalFindUser {
    db: TenantDb,
}

impl HxLocalFindUser {
    pub fn new(db: TenantDb) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Tool for HxLocalFindUser {
    fn name(&self) -> &str {
        "hx_local_find_user"
    }

    fn description(&self) -> &str {
        "在本地数据库中按手机号、渠道 ID 或用户 ID 查找用户。"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "phone": { "type": "string", "description": "手机号" },
                "channel_type": { "type": "string", "description": "渠道类型" },
                "sender_id": { "type": "string", "description": "渠道中的发送者 ID" },
                "user_id": { "type": "string", "description": "用户 ID" }
            }
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let record = if let Some(phone) = args["phone"].as_str() {
            self.db.find_by_phone(phone).await?
        } else if let (Some(ch), Some(sid)) =
            (args["channel_type"].as_str(), args["sender_id"].as_str())
        {
            let ch = normalize_channel(ch);
            self.db.find_by_channel(ch, sid).await?
        } else if let Some(uid) = args["user_id"].as_str() {
            self.db.get_user(uid).await?
        } else {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("请提供 phone、channel_type+sender_id 或 user_id".to_string()),
            });
        };

        match record {
            Some(r) => Ok(ToolResult {
                success: true,
                output: json!({
                    "found": true,
                    "user_id": r.user_id,
                    "agent_id": r.agent_id,
                    "phone": r.phone,
                    "nickname": r.nickname,
                    "star_name": r.star_name,
                    "template": r.template,
                    "plan": r.plan,
                    "status": r.status,
                    "created_at": r.created_at,
                })
                .to_string(),
                error: None,
            }),
            None => Ok(ToolResult {
                success: true,
                output: json!({ "found": false }).to_string(),
                error: None,
            }),
        }
    }
}

// ── hx_local_bind_channel ────────────────────────────

/// Bind a new channel to an existing user.
pub struct HxLocalBindChannel {
    db: TenantDb,
    router: Arc<TenantRouter>,
}

impl HxLocalBindChannel {
    pub fn new(db: TenantDb, router: Arc<TenantRouter>) -> Self {
        Self { db, router }
    }
}

#[async_trait]
impl Tool for HxLocalBindChannel {
    fn name(&self) -> &str {
        "hx_local_bind_channel"
    }

    fn description(&self) -> &str {
        "为已注册用户绑定新渠道（QQ号、飞书ID等），绑定后该渠道的消息将路由到用户的专属 AI 助手。"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "user_id": { "type": "string", "description": "用户 ID" },
                "phone": { "type": "string", "description": "手机号（用于查找用户）" },
                "channel_type": { "type": "string", "description": "渠道类型: napcat / lark / qq" },
                "peer_id": { "type": "string", "description": "渠道中的用户 ID" },
                "peer_name": { "type": "string", "description": "渠道中的用户名（可选）" }
            },
            "required": ["channel_type", "peer_id"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let channel_type = normalize_channel(args["channel_type"].as_str().unwrap_or_default());
        let peer_id = args["peer_id"].as_str().unwrap_or_default();
        let peer_name = args["peer_name"].as_str();

        if channel_type.is_empty() || peer_id.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("channel_type and peer_id are required".to_string()),
            });
        }

        // Find user by user_id or phone
        let user = if let Some(uid) = args["user_id"].as_str() {
            self.db.get_user(uid).await?
        } else if let Some(phone) = args["phone"].as_str() {
            self.db.find_by_phone(phone).await?
        } else {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("请提供 user_id 或 phone".to_string()),
            });
        };

        let user = match user {
            Some(u) => u,
            None => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("用户不存在".to_string()),
                });
            }
        };

        match self
            .db
            .bind_channel(&user.user_id, channel_type, peer_id, peer_name)
            .await
        {
            Ok(()) => {
                // Invalidate cache for the new channel binding
                self.router.invalidate(channel_type, peer_id);
                Ok(ToolResult {
                    success: true,
                    output: json!({
                        "bound": true,
                        "user_id": user.user_id,
                        "agent_id": user.agent_id,
                        "channel_type": channel_type,
                        "peer_id": peer_id,
                    })
                    .to_string(),
                    error: None,
                })
            }
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("绑定渠道失败: {e}")),
            }),
        }
    }
}

// ── hx_local_list_users ─────────────────────────────

/// List users from local DB.
pub struct HxLocalListUsers {
    db: TenantDb,
}

impl HxLocalListUsers {
    pub fn new(db: TenantDb) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Tool for HxLocalListUsers {
    fn name(&self) -> &str {
        "hx_local_list_users"
    }

    fn description(&self) -> &str {
        "列出本地注册用户（仅 Guardian 可用）。支持按状态、模板、套餐筛选。"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "status": { "type": "string", "description": "筛选状态: active / disabled" },
                "template": { "type": "string", "description": "筛选模板" },
                "plan": { "type": "string", "description": "筛选套餐" },
                "limit": { "type": "number", "description": "每页数量（默认50）" },
                "offset": { "type": "number", "description": "偏移量" }
            }
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        use crate::huanxing::db::UserFilter;
        let filter = UserFilter {
            status: args["status"].as_str().map(String::from),
            template: args["template"].as_str().map(String::from),
            plan: args["plan"].as_str().map(String::from),
            limit: args["limit"].as_u64().map(|v| v as u32),
            offset: args["offset"].as_u64().map(|v| v as u32),
        };

        match self.db.list_users(&filter).await {
            Ok((users, total)) => Ok(ToolResult {
                success: true,
                output: json!({
                    "total": total,
                    "count": users.len(),
                    "users": users.iter().map(|u| json!({
                        "user_id": u.user_id,
                        "agent_id": u.agent_id,
                        "phone": u.phone,
                        "nickname": u.nickname,
                        "template": u.template,
                        "plan": u.plan,
                        "status": u.status,
                        "created_at": u.created_at,
                        "last_active": u.last_active,
                    })).collect::<Vec<_>>(),
                })
                .to_string(),
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("查询用户列表失败: {e}")),
            }),
        }
    }
}

// ── hx_local_update_user ─────────────────────────────

/// Update user fields in local DB.
pub struct HxLocalUpdateUser {
    db: TenantDb,
    router: Arc<TenantRouter>,
}

impl HxLocalUpdateUser {
    pub fn new(db: TenantDb, router: Arc<TenantRouter>) -> Self {
        Self { db, router }
    }
}

#[async_trait]
impl Tool for HxLocalUpdateUser {
    fn name(&self) -> &str {
        "hx_local_update_user"
    }

    fn description(&self) -> &str {
        "更新用户信息（仅 Guardian 可用）。可更新昵称、星名、套餐、状态。"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "user_id": { "type": "string", "description": "用户 ID" },
                "nickname": { "type": "string", "description": "新昵称" },
                "star_name": { "type": "string", "description": "新 AI 助手名" },
                "plan": { "type": "string", "description": "新套餐" },
                "plan_expires": { "type": "string", "description": "套餐到期时间" },
                "status": { "type": "string", "description": "新状态: active / disabled" }
            },
            "required": ["user_id"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let user_id = args["user_id"].as_str().unwrap_or_default();
        if user_id.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("user_id is required".to_string()),
            });
        }

        match self
            .db
            .update_user(
                user_id,
                args["nickname"].as_str(),
                args["star_name"].as_str(),
                args["plan"].as_str(),
                args["plan_expires"].as_str(),
                args["status"].as_str(),
            )
            .await
        {
            Ok(updated) => {
                if updated {
                    self.router.invalidate_user(user_id);
                }
                Ok(ToolResult {
                    success: true,
                    output: json!({
                        "updated": updated,
                        "user_id": user_id,
                    })
                    .to_string(),
                    error: None,
                })
            }
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("更新失败: {e}")),
            }),
        }
    }
}

// ── hx_local_stats ───────────────────────────────────

/// Local database statistics (admin tool).
pub struct HxLocalStats {
    db: TenantDb,
}

impl HxLocalStats {
    pub fn new(db: TenantDb) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Tool for HxLocalStats {
    fn name(&self) -> &str {
        "hx_local_stats"
    }

    fn description(&self) -> &str {
        "获取本地数据库统计信息（用户数、活跃用户、模板分布、套餐分布）。仅 Guardian 可用。"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({ "type": "object", "properties": {} })
    }

    async fn execute(&self, _args: serde_json::Value) -> anyhow::Result<ToolResult> {
        match self.db.get_stats().await {
            Ok(stats) => Ok(ToolResult {
                success: true,
                output: json!({
                    "total_users": stats.total_users,
                    "active_users": stats.active_users,
                    "disabled_users": stats.disabled_users,
                    "total_channels": stats.total_channels,
                    "templates": stats.templates.iter()
                        .map(|(k, v)| json!({"name": k, "count": v}))
                        .collect::<Vec<_>>(),
                    "plans": stats.plans.iter()
                        .map(|(k, v)| json!({"name": k, "count": v}))
                        .collect::<Vec<_>>(),
                })
                .to_string(),
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("获取统计失败: {e}")),
            }),
        }
    }
}

// ═══════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════

/// Mask phone number for display: 138****8367
fn mask_phone(phone: &str) -> String {
    if phone.len() >= 7 {
        format!("{}****{}", &phone[..3], &phone[phone.len() - 4..])
    } else {
        phone.to_string()
    }
}

// ═══════════════════════════════════════════════════════
// hx_tts — Text-to-Speech voice message
// ═══════════════════════════════════════════════════════

/// Agent tool to synthesize text-to-speech and return audio data
/// for the channel layer to send as a voice message.
pub struct HxTts {
    tts_config: crate::config::TtsConfig,
    workspace_dir: std::path::PathBuf,
}

impl HxTts {
    pub fn new(tts_config: crate::config::TtsConfig, workspace_dir: std::path::PathBuf) -> Self {
        Self { tts_config, workspace_dir }
    }
}

#[async_trait]
impl Tool for HxTts {
    fn name(&self) -> &str {
        "hx_tts"
    }

    fn description(&self) -> &str {
        "将文字转换为语音消息。调用成功后会返回一个 [VOICE:...] 标记。\n\
         重要：你必须将返回的这个 [VOICE:...] 标记原样最终输出给用户，频道底层才能将其渲染并发送为真实的语音气泡！\n\
         应用场景：主动语音播报、语音提醒、有代入感的语音回复等。\n\
         可选音色（如果不指定默认使用系统配置）：\n\
         - Chelsie (温柔女声) \n\
         - Kai (凯：阳光男声，耳朵的一场SPA)\n\
         - Moon (月白：率性帅气男声)\n\
         - Maia (四月：知性温柔女声)\n\
         - Nofish (不吃鱼：不会翘舌音的设计师男声)\n\
         - Bella (萌宝：喝酒不打醉拳的小萝莉)"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "要转换为语音的文字内容（最大4096字符）"
                },
                "voice": {
                    "type": "string",
                    "description": "音色选择（可选，默认使用配置的默认音色）"
                }
            },
            "required": ["text"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        tracing::info!("hx_tts: execute called with args: {args}");

        let text = args
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();

        if text.is_empty() {
            tracing::warn!("hx_tts: text is empty, returning error");
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("text 不能为空".into()),
            });
        }

        if text.len() > 4096 {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("text 超过最大长度限制（4096字符）".into()),
            });
        }

        let voice = args
            .get("voice")
            .and_then(|v| v.as_str())
            .unwrap_or(&self.tts_config.default_voice);

        // Synthesize audio using huanxing voice module
        match super::voice::synthesize_with_voice(&self.tts_config, text, voice).await {
            Ok(audio_bytes) => {
                // Write to tenant workspace tts_cache dir
                let tts_dir = self.workspace_dir.join("tts_cache");
                let _ = std::fs::create_dir_all(&tts_dir);
                let ext = &self.tts_config.default_format;
                let filename = format!("{}.{ext}", uuid::Uuid::new_v4());
                let file_path = tts_dir.join(&filename);

                if let Err(e) = std::fs::write(&file_path, &audio_bytes) {
                    return Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("写入音频文件失败: {e}")),
                    });
                }

                tracing::info!(
                    "hx_tts: synthesized {} bytes, saved to {}",
                    audio_bytes.len(),
                    file_path.display()
                );

                // Return VOICE marker with file:// prefix for NapCat
                Ok(ToolResult {
                    success: true,
                    output: format!("[VOICE:file://{}]", file_path.display()),
                    error: None,
                })
            }
            Err(e) => {
                tracing::error!("hx_tts: 语音合成失败: {e:?}");
                Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("语音合成失败: {e}")),
                })
            }
        }
    }
}
